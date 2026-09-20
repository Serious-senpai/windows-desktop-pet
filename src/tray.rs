use std::io::Error;
use std::{mem, ptr};

use anyhow::Context;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, NOTIFYICONDATAW_0,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, GetCursorPos, HICON, MF_STRING, PostMessageW,
    SetForegroundWindow, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows_sys::core::GUID;
use windows_sys::w;

use crate::config::{MENU_EXIT, TRAY_ICON_ID, TRAY_ICON_TOOLTIP, WM_TRAYICON};

pub struct TrayIcon {
    _window: HWND,
    _data: NOTIFYICONDATAW,
}

impl TrayIcon {
    pub fn new(window: HWND, icon: HICON) -> anyhow::Result<Self> {
        let mut result = Self {
            _window: window,
            _data: NOTIFYICONDATAW {
                cbSize: mem::size_of::<NOTIFYICONDATAW>().try_into()?,
                hWnd: window,
                uID: TRAY_ICON_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: icon,
                szTip: [0; 128],
                dwState: 0,
                dwStateMask: 0,
                szInfo: [0; 256],
                Anonymous: NOTIFYICONDATAW_0::default(),
                szInfoTitle: [0; 64],
                dwInfoFlags: 0,
                guidItem: GUID::default(),
                hBalloonIcon: HICON::default(),
            },
        };

        let tip = TRAY_ICON_TOOLTIP.as_slice_with_nul();
        result._data.szTip[..tip.len()].copy_from_slice(tip);
        result._data.szInfo[..tip.len()].copy_from_slice(tip);
        result._data.szInfoTitle[..tip.len()].copy_from_slice(tip);

        if unsafe { Shell_NotifyIconW(NIM_ADD, &mut result._data) } == 0 {
            anyhow::bail!("Shell_NotifyIconW error");
        }

        Ok(result)
    }

    pub fn show(&self) -> anyhow::Result<()> {
        let menu = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return Err(Error::last_os_error()).context("CreatePopupMenu error");
        }

        if unsafe { AppendMenuW(menu, MF_STRING, MENU_EXIT, w!("Exit")) } == 0 {
            return Err(Error::last_os_error()).context("AppendMenuW error");
        }

        let mut cursor = POINT::default();
        unsafe {
            SetForegroundWindow(self._window);
            GetCursorPos(&mut cursor);
            TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON,
                cursor.x,
                cursor.y,
                0,
                self._window,
                ptr::null(),
            );
            PostMessageW(self._window, WM_NULL, 0, 0);
        }

        Ok(())
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &mut self._data);
        }
    }
}
