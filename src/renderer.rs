use std::ffi::c_void;
use std::io::Error;
use std::{mem, ptr, slice};

use anyhow::Context;
use windows_sys::Win32::Foundation::{
    HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, SetLastError, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, RGBQUAD, ReleaseDC, SelectObject,
};
use windows_sys::Win32::System::SystemServices::MK_LBUTTON;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetSystemMetrics, GetWindowLongPtrW, IDC_ARROW,
    LoadCursorW, LoadIconW, PostMessageW, PostQuitMessage, RegisterClassExW, SM_CXSCREEN,
    SM_CYSCREEN, SW_SHOWNOACTIVATE, SetWindowLongPtrW, ShowWindow, ULW_ALPHA,
    UPDATELAYEREDWINDOWINFO, UpdateLayeredWindowIndirect, WM_CLOSE, WM_COMMAND, WM_DESTROY,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows_sys::w;

use crate::config::{DEFAULT_BLENDFUNCTION, MENU_EXIT, WINDOW_CLASS_NAME, WM_TRAYICON};
use crate::log;
use crate::timer::Timer;
use crate::tray::TrayIcon;
use crate::utils::get_lparam_xy;

type TimerCallback = fn(&mut Renderer) -> LRESULT;
type LButtonDown = fn(&mut Renderer, offset_x: i32, offset_y: i32) -> LRESULT;
type LButtonUp = fn(&mut Renderer) -> LRESULT;

pub struct Renderer {
    pixels: *mut c_void,
    bitmap: HBITMAP,
    memory_dc: HDC,
    screen_dc: HDC,
    screen_size: SIZE,
    area: usize,
    window: HWND,
    tray: TrayIcon,
    timers: Vec<(Timer, TimerCallback)>,
    on_lbutton_down: Option<LButtonDown>,
    on_lbutton_up: Option<LButtonUp>,
}

impl Renderer {
    pub fn new(instance: HINSTANCE) -> anyhow::Result<Box<Self>> {
        let icon = unsafe { LoadIconW(instance, w!("IDI_APP_ICON")) };
        if icon.is_null() {
            return Err(Error::last_os_error()).context("LoadIconW error");
        }

        let cls_attr = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>().try_into()?,
            style: 0,
            lpfnWndProc: Some(Self::window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: ptr::null_mut(),
            hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: WINDOW_CLASS_NAME,
            hIconSm: ptr::null_mut(),
        };
        if unsafe { RegisterClassExW(&cls_attr) } == 0 {
            return Err(Error::last_os_error()).context("RegisterClassExW error");
        }

        let screen_size = SIZE {
            cx: unsafe { GetSystemMetrics(SM_CXSCREEN) },
            cy: unsafe { GetSystemMetrics(SM_CYSCREEN) },
        };
        if screen_size.cx <= 0 || screen_size.cy <= 0 {
            anyhow::bail!("GetSystemMetrics error");
        }
        let area = usize::try_from(screen_size.cx)?
            .checked_mul(screen_size.cy.try_into()?)
            .context("Cannot calculate screen area")?;

        let window = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                WINDOW_CLASS_NAME,
                w!("Windows Desktop Pet"),
                WS_POPUP,
                0,
                0,
                screen_size.cx,
                screen_size.cy,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            )
        };
        if window.is_null() {
            return Err(Error::last_os_error()).context("CreateWindowExW error");
        }

        let mut result = Box::new(Self {
            pixels: ptr::null_mut(),
            bitmap: ptr::null_mut(),
            memory_dc: ptr::null_mut(),
            screen_dc: ptr::null_mut(),
            screen_size,
            area,
            window,
            tray: TrayIcon::new(window, icon)?,
            timers: vec![],
            on_lbutton_down: None,
            on_lbutton_up: None,
        });

        if unsafe {
            ShowWindow(window, SW_SHOWNOACTIVATE);
            SetLastError(0);
            SetWindowLongPtrW(
                result.window,
                GWLP_USERDATA,
                result.as_mut() as *mut Self as isize,
            )
        } == 0
        {
            let e = Error::last_os_error();
            if e.raw_os_error() != Some(0) {
                return Err(e).context("SetWindowLongPtrW error");
            }
        }

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: result.screen_size.cx,
                biHeight: -result.screen_size.cy,
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

    pub fn screen_width(&self) -> i32 {
        self.screen_size.cx
    }

    pub fn screen_height(&self) -> i32 {
        self.screen_size.cy
    }

    pub fn set_on_lbutton_down(&mut self, callback: LButtonDown) {
        self.on_lbutton_down = Some(callback);
    }

    pub fn set_on_lbutton_up(&mut self, callback: LButtonUp) {
        self.on_lbutton_up = Some(callback);
    }

    pub fn add_timer(
        &mut self,
        interval_ms: u32,
        callback: TimerCallback,
    ) -> anyhow::Result<usize> {
        let id = self.timers.len();
        let timer = Timer::new(self.window, id, interval_ms)?;
        self.timers.push((timer, callback));
        Ok(id)
    }

    pub fn update_timer(&self, id: usize, interval_ms: u32) -> anyhow::Result<()> {
        if let Some((timer, _)) = self.timers.get(id) {
            timer.set_new_interval(interval_ms)?;
            Ok(())
        } else {
            anyhow::bail!("Unknown timer id: {id}");
        }
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        unsafe { slice::from_raw_parts_mut(self.pixels.cast(), self.area) }
    }

    pub fn draw_pixels(&mut self, area: &RECT, mut data: &[u32]) -> anyhow::Result<()> {
        let x_start = usize::try_from(area.left)?;
        let x_end = usize::try_from(area.right)?;
        let y_start = usize::try_from(area.top)?;
        let y_end = usize::try_from(area.bottom)?;

        if x_end < x_start || y_end < y_start {
            anyhow::bail!("Invalid update area");
        }

        let dx = x_end.saturating_sub(x_start);

        let screen_width = usize::try_from(self.screen_size.cx)?;
        let pixels = self.pixels_mut();
        for y in y_start..=y_end {
            let start = y.saturating_mul(screen_width).saturating_add(x_start);
            let end = start.saturating_add(dx).min(pixels.len());
            let size = end.saturating_sub(start);

            let (to_copy, remaining) = data.split_at(size);
            pixels[start..end].copy_from_slice(to_copy);
            data = remaining;
        }

        Ok(())
    }

    pub fn clear_pixels(&mut self, area: &RECT) -> anyhow::Result<()> {
        let x_start = usize::try_from(area.left)?;
        let x_end = usize::try_from(area.right)?;
        let y_start = usize::try_from(area.top)?;
        let y_end = usize::try_from(area.bottom)?;

        if x_end < x_start || y_end < y_start {
            anyhow::bail!("Invalid clear area");
        }

        let dx = x_end.saturating_sub(x_start);

        let screen_width = usize::try_from(self.screen_size.cx)?;
        let pixels = self.pixels_mut();
        for y in y_start..=y_end {
            let start = y.saturating_mul(screen_width).saturating_add(x_start);
            let end = start.saturating_add(dx).min(pixels.len());
            pixels[start..end].fill(0);
        }

        Ok(())
    }

    pub fn update(&self, area: &RECT) -> anyhow::Result<()> {
        const POINT_ZERO: POINT = POINT { x: 0, y: 0 };
        let update = UPDATELAYEREDWINDOWINFO {
            cbSize: mem::size_of::<UPDATELAYEREDWINDOWINFO>().try_into()?,
            hdcDst: self.screen_dc,
            pptDst: ptr::null_mut(),
            psize: ptr::null_mut(),
            hdcSrc: self.memory_dc,
            pptSrc: &POINT_ZERO,
            crKey: 0,
            pblend: &DEFAULT_BLENDFUNCTION,
            dwFlags: ULW_ALPHA,
            prcDirty: area,
        };
        let result = unsafe { UpdateLayeredWindowIndirect(self.window, &update) };
        if result == 0 {
            return Err(Error::last_os_error()).context("UpdateLayeredWindowIndirect error");
        }

        Ok(())
    }

    fn get_this_mut<'a>(window: HWND) -> Option<&'a mut Self> {
        let ptr = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut Self };
        if ptr.is_null() {
            log!("GetWindowLongPtrW error: {:?}", Error::last_os_error());
        }

        unsafe { ptr.as_mut() }
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_DESTROY => {
                unsafe {
                    PostQuitMessage(0);
                }
                0
            }
            WM_TIMER => {
                let timer_id = wparam;
                match Self::get_this_mut(window) {
                    Some(this) => match this.timers.get(timer_id) {
                        Some((_, callback)) => callback(this),
                        None => {
                            log!("Unknown timer id: {timer_id}");
                            0
                        }
                    },
                    None => 0,
                }
            }
            WM_LBUTTONDOWN => {
                if u32::try_from(wparam).unwrap_or_default() == MK_LBUTTON {
                    let (offset_x, offset_y) = match get_lparam_xy(lparam) {
                        Ok((x, y)) => (x, y),
                        Err(e) => {
                            log!("get_lparam_xy error: {e:?}");
                            return 0;
                        }
                    };

                    if let Some(this) = Self::get_this_mut(window)
                        && let Some(callback) = this.on_lbutton_down
                    {
                        return callback(this, offset_x, offset_y);
                    }
                }
                0
            }
            WM_LBUTTONUP => {
                if let Some(this) = Self::get_this_mut(window)
                    && let Some(callback) = this.on_lbutton_up
                {
                    return callback(this);
                }

                0
            }
            WM_TRAYICON => {
                if let Ok(lparam) = u32::try_from(lparam)
                    && (lparam == WM_RBUTTONUP || lparam == WM_LBUTTONUP)
                    && let Some(this) = Self::get_this_mut(window)
                    && let Err(e) = this.tray.show()
                {
                    log!("Error showing tray icon menu: {e:?}");
                }
                0
            }
            WM_COMMAND => {
                if wparam == MENU_EXIT
                    && let Some(this) = Self::get_this_mut(window)
                {
                    unsafe {
                        PostMessageW(this.window, WM_CLOSE, 0, 0);
                    }
                }
                0
            }
            _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
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
