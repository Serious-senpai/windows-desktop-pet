use std::ffi::c_void;
use std::io::Error;
use std::{mem, ptr, slice};

use anyhow::Context;
use windows_sys::Win32::Foundation::{HWND, POINT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, RGBQUAD, ReleaseDC, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};

use crate::config::DEFAULT_BLENDFUNCTION;

pub struct Renderer {
    pixels: *mut c_void,
    bitmap: HBITMAP,
    memory_dc: HDC,
    screen_dc: HDC,
    size: SIZE,
    area: usize,
}

impl Renderer {
    pub fn new(width: i32, height: i32) -> anyhow::Result<Self> {
        if width <= 0 || height <= 0 {
            anyhow::bail!("width ({width} and height ({height}) must be positive");
        }

        let mut result = Self {
            pixels: ptr::null_mut(),
            bitmap: ptr::null_mut(),
            memory_dc: ptr::null_mut(),
            screen_dc: ptr::null_mut(),
            size: SIZE {
                cx: width,
                cy: height,
            },
            area: usize::try_from(width)
                .and_then(|w| usize::try_from(height).map(|h| w.strict_mul(h)))?,
        };

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>().try_into()?,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default()],
        };
        result.bitmap = unsafe {
            CreateDIBSection(
                ptr::null_mut(),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut result.pixels,
                ptr::null_mut(),
                0,
            )
        };
        if result.bitmap.is_null() {
            return Err(Error::last_os_error()).context("CreateDIBSection error");
        }

        result.memory_dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
        if result.memory_dc.is_null() {
            return Err(Error::last_os_error()).context("CreateCompatibleDC error");
        }

        let old_bitmap = unsafe { SelectObject(result.memory_dc, result.bitmap) };
        if old_bitmap.is_null() {
            anyhow::bail!("SelectObject error");
        }

        result.screen_dc = unsafe { GetDC(ptr::null_mut()) };
        if result.screen_dc.is_null() {
            anyhow::bail!("GetDC error");
        }

        Ok(result)
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        unsafe { slice::from_raw_parts_mut(self.pixels.cast(), self.area) }
    }

    pub fn update(&self, window: HWND, position: &POINT) -> anyhow::Result<()> {
        const POINT_ZERO: POINT = POINT { x: 0, y: 0 };
        let result = unsafe {
            UpdateLayeredWindow(
                window,
                self.screen_dc,
                position,
                &self.size,
                self.memory_dc,
                &POINT_ZERO,
                0,
                &DEFAULT_BLENDFUNCTION,
                ULW_ALPHA,
            )
        };

        if result == 0 {
            return Err(Error::last_os_error()).context("UpdateLayeredWindow error");
        }

        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if !self.screen_dc.is_null() {
            unsafe {
                ReleaseDC(ptr::null_mut(), self.screen_dc);
            }
        }
        if !self.memory_dc.is_null() {
            unsafe {
                DeleteDC(self.memory_dc);
            }
        }
        if !self.bitmap.is_null() {
            unsafe {
                DeleteObject(self.bitmap);
            }
        }
    }
}
