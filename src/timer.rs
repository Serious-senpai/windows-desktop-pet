use std::io::Error;

use anyhow::Context;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

pub struct Timer {
    window: HWND,
    id: usize,
}

impl Timer {
    pub fn new(window: HWND, id: usize, interval_ms: u32) -> anyhow::Result<Self> {
        if unsafe { SetTimer(window, id, interval_ms, None) } == 0 {
            return Err(Error::last_os_error()).context("SetTimer error");
        }

        Ok(Self { window, id })
    }

    pub fn set_new_interval(&self, interval_ms: u32) -> anyhow::Result<()> {
        if unsafe { SetTimer(self.window, self.id, interval_ms, None) } == 0 {
            return Err(Error::last_os_error()).context("SetTimer error");
        }

        Ok(())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        unsafe {
            KillTimer(self.window, self.id);
        }
    }
}
