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
    _pixels: *mut c_void,
    _bitmap: HBITMAP,
    _memory_dc: HDC,
    _screen_dc: HDC,
    _size: SIZE,
    _area: usize,
}

impl Renderer {
    pub fn new(width: i32, height: i32) -> anyhow::Result<Self> {
        if width <= 0 || height <= 0 {
            anyhow::bail!("width ({width} and height ({height}) must be positive");
        }

        let mut result = Self {
            _pixels: ptr::null_mut(),
            _bitmap: ptr::null_mut(),
            _memory_dc: ptr::null_mut(),
            _screen_dc: ptr::null_mut(),
            _size: SIZE {
                cx: width,
                cy: height,
            },
            _area: usize::try_from(width)
                .and_then(|w| usize::try_from(height).map(|h| w.strict_mul(h)))
                .context("usize::try_from failure")?,
        };

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
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
        result._bitmap = unsafe {
            CreateDIBSection(
                ptr::null_mut(),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut result._pixels,
                ptr::null_mut(),
                0,
            )
        };
        if result._bitmap.is_null() {
            return Err(Error::last_os_error()).context("CreateDIBSection error");
        }

        result._memory_dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
        if result._memory_dc.is_null() {
            return Err(Error::last_os_error()).context("CreateCompatibleDC error");
        }

        let old_bitmap = unsafe { SelectObject(result._memory_dc, result._bitmap) };
        if old_bitmap.is_null() {
            anyhow::bail!("SelectObject error");
        }

        result._screen_dc = unsafe { GetDC(ptr::null_mut()) };
        if result._screen_dc.is_null() {
            anyhow::bail!("GetDC error");
        }

        Ok(result)
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        unsafe { slice::from_raw_parts_mut(self._pixels.cast(), self._area) }
    }

    pub fn update(&self, window: HWND, position: &POINT) -> anyhow::Result<()> {
        const POINT_ZERO: POINT = POINT { x: 0, y: 0 };
        let result = unsafe {
            UpdateLayeredWindow(
                window,
                self._screen_dc,
                position,
                &self._size,
                self._memory_dc,
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
        if !self._screen_dc.is_null() {
            unsafe {
                ReleaseDC(ptr::null_mut(), self._screen_dc);
            }
        }
        if !self._memory_dc.is_null() {
            unsafe {
                DeleteDC(self._memory_dc);
            }
        }
        if !self._bitmap.is_null() {
            unsafe {
                DeleteObject(self._bitmap);
            }
        }
    }
}
