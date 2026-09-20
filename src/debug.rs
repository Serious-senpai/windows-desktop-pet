#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        let mut s = format!($($arg)*);
        s.push('\0');
        unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringA(s.as_ptr().cast());
        }
    };
}
