use std::io::Error;

use anyhow::Context;
use windows_sys::Win32::Foundation::{LPARAM, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub struct DropGuard<T, F>
where
    F: FnOnce(T),
{
    argument: Option<T>,
    drop_fn: Option<F>,
}

impl<T, F> DropGuard<T, F>
where
    F: FnOnce(T),
{
    pub fn new(argument: T, drop_fn: F) -> Self {
        Self {
            argument: Some(argument),
            drop_fn: Some(drop_fn),
        }
    }
}

impl<T, F> Drop for DropGuard<T, F>
where
    F: FnOnce(T),
{
    fn drop(&mut self) {
        if let Some(argument) = self.argument.take()
            && let Some(drop_fn) = self.drop_fn.take()
        {
            drop_fn(argument);
        }
    }
}

pub fn get_lparam_xy(lparam: LPARAM) -> anyhow::Result<(i32, i32)> {
    let x = i32::try_from(lparam & 0xFFFF)?;
    let y = i32::try_from((lparam >> 16) & 0xFFFF)?;

    Ok((x, y))
}

pub fn get_cursor_pos() -> anyhow::Result<POINT> {
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) } == 0 {
        return Err(Error::last_os_error()).context("GetCursorPos error");
    }

    Ok(cursor)
}
