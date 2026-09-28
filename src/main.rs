#![windows_subsystem = "windows"]

mod animator;
mod config;
mod debug;
mod loader;
mod renderer;
mod tray;
mod utils;

use std::io::{BufReader, Error};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::{env, fs, mem, ptr};

use anyhow::Context;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemServices::MK_LBUTTON;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW, KillTimer,
    LoadCursorW, LoadIconW, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
    SW_SHOWNOACTIVATE, SetTimer, ShowWindow, TranslateMessage, WM_CLOSE, WM_COMMAND, WM_DESTROY,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows_sys::w;

use crate::animator::Animator;
use crate::config::{
    Config, FPS_MS_FAST, MENU_EXIT, TIMER_ID_FPS, TIMER_ID_START_RUNNING, WINDOW_CLASS_NAME,
    WM_TRAYICON,
};
use crate::loader::SpritesheetLoader;
use crate::renderer::Renderer;
use crate::tray::TrayIcon;
use crate::utils::{DropGuard, get_lparam_xy};

static ANIMATOR: AtomicPtr<Animator> = AtomicPtr::new(ptr::null_mut());
static TRAY_ICON: AtomicPtr<TrayIcon> = AtomicPtr::new(ptr::null_mut());

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // log!("Received {hwnd:?} {msg:#x} {wparam:?} {lparam:?}");
    match msg {
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        WM_TIMER => {
            match wparam {
                TIMER_ID_FPS => {
                    let animator = ANIMATOR.load(Ordering::Acquire);
                    // SAFETY: Single-thread
                    if let Some(animator) = unsafe { animator.as_mut() }
                        && let Err(e) = animator.render_next_frame(hwnd)
                    {
                        log!("Cannot render next frame: {e:?}");
                    }
                }
                TIMER_ID_START_RUNNING => {
                    let animator = ANIMATOR.load(Ordering::Acquire);
                    // SAFETY: Single-thread
                    if let Some(animator) = unsafe { animator.as_mut() } {
                        if let Err(e) = animator.run(hwnd) {
                            log!("Cannot change action: {e:?}");
                        }

                        if let Err(e) = animator.reset_change_action_timer(hwnd) {
                            log!("Cannot reset change action timer: {e:?}");
                        }
                    }
                }
                other => {
                    log!("Unknown timer id: {other}");
                }
            }
            0
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

                let animator = ANIMATOR.load(Ordering::Acquire);
                // SAFETY: Single-thread
                if let Some(animator) = unsafe { animator.as_mut() }
                    && let Err(e) = animator.lift(hwnd, offset_x, offset_y)
                {
                    log!("Cannot lift: {e:?}");
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let animator = ANIMATOR.load(Ordering::Acquire);
            // SAFETY: Single-thread
            if let Some(animator) = unsafe { animator.as_mut() }
                && let Err(e) = animator.drop(hwnd)
            {
                log!("Cannot drop: {e:?}");
            }

            0
        }
        WM_TRAYICON => {
            let tray_icon = TRAY_ICON.load(Ordering::Acquire);
            if let Ok(lparam) = u32::try_from(lparam)
                && (lparam == WM_RBUTTONUP || lparam == WM_LBUTTONUP)
                // SAFETY: Single-thread
                && let Some(tray_icon) = unsafe { tray_icon.as_ref() }
                && let Err(e) = tray_icon.show()
            {
                log!("Error showing tray icon menu: {e:?}");
            }
            0
        }
        WM_COMMAND => {
            if let MENU_EXIT = wparam {
                unsafe {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn create_window(instance: &mut HINSTANCE, width: i32, height: i32) -> anyhow::Result<HWND> {
    *instance = unsafe { GetModuleHandleW(ptr::null()) };

    let cls_attr = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>().try_into()?,
        style: 0,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: *instance,
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

    let window = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            WINDOW_CLASS_NAME,
            w!("Windows Desktop Pet"),
            WS_POPUP,
            0, // Will be overwritten later by UpdateLayeredWindow anyway
            0, // Will be overwritten later by UpdateLayeredWindow anyway
            width,
            height,
            ptr::null_mut(),
            ptr::null_mut(),
            *instance,
            ptr::null(),
        )
    };
    if window.is_null() {
        return Err(Error::last_os_error()).context("CreateWindowExW error");
    }

    Ok(window)
}

fn main() {
    if let Err(e) = _main() {
        log!("Application error: {e:?}");
    }
}

fn _main() -> anyhow::Result<()> {
    let current_dir = env::current_exe()
        .context("Cannot get current exe path")?
        .parent()
        .context("Cannot get current directory")?
        .to_path_buf();

    let config_path = current_dir.join("config.json");
    log!("Loading config from {}", config_path.display());

    let file = fs::File::open(&config_path).context("Cannot open config file")?;
    let config = serde_json::from_reader::<_, Config>(file).context("Cannot parse config file")?;
    log!("Loaded config: {config:?}");
    config.validate().context("Config is invalid")?;

    let spritesheet_path = current_dir.join(&config.spritesheet_path);
    log!("Loading spritesheet from {}", spritesheet_path.display());
    let spritesheet = fs::File::open(&spritesheet_path).with_context(|| {
        format!(
            "Cannot open spritesheet file {}",
            config.spritesheet_path.display()
        )
    })?;
    let loader = SpritesheetLoader::new(BufReader::new(spritesheet), config.rows, config.columns)
        .context("Cannot load spritesheet")?;
    log!(
        "Frame width {}, frame height {}",
        loader.frame_width(),
        loader.frame_height(),
    );

    let renderer = Renderer::new(
        loader.frame_width().try_into()?,
        loader.frame_height().try_into()?,
    )
    .with_context(|| "Cannot create renderer")?;

    let mut instance = HINSTANCE::default();
    let window = create_window(
        &mut instance,
        loader.frame_width().try_into()?,
        loader.frame_height().try_into()?,
    )
    .with_context(|| "Cannot create window")?;
    unsafe {
        ShowWindow(window, SW_SHOWNOACTIVATE);
    }

    // Initial state: drop -> FPS_MS_FAST -> idle -> FPS_MS_SLOW
    if unsafe { SetTimer(window, TIMER_ID_FPS, FPS_MS_FAST, None) } == 0 {
        return Err(Error::last_os_error()).context("SetTimer error");
    }

    let guard1 = DropGuard::new((), |_| unsafe {
        KillTimer(window, TIMER_ID_FPS);
    });

    let mut animator = Animator::new(config, loader, renderer).context("Cannot create animator")?;
    animator
        .reset_change_action_timer(window)
        .context("Cannot initialize change action timer")?;
    let guard2 = DropGuard::new((), |_| unsafe {
        KillTimer(window, TIMER_ID_START_RUNNING);
    });

    ANIMATOR.store(Box::into_raw(Box::new(animator)), Ordering::Release);

    let icon = unsafe { LoadIconW(instance, w!("IDI_APP_ICON")) };
    if icon.is_null() {
        return Err(Error::last_os_error()).context("LoadIconW error");
    }

    let tray_icon = TrayIcon::new(window, icon).with_context(|| "Cannot create tray icon")?;

    TRAY_ICON.store(Box::into_raw(Box::new(tray_icon)), Ordering::Release);

    log!("Starting message loop");
    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, ptr::null_mut(), 0, 0) };
        match result {
            -1 => {
                log!("GetMessageW error: {}", Error::last_os_error());
                break;
            }
            0 => {
                break;
            }
            _ => unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
    log!("Message loop ended");

    let tray_icon = TRAY_ICON.swap(ptr::null_mut(), Ordering::AcqRel);
    if !tray_icon.is_null() {
        unsafe {
            let _ = Box::from_raw(tray_icon);
        }
    }

    let animator = ANIMATOR.swap(ptr::null_mut(), Ordering::AcqRel);
    if !animator.is_null() {
        unsafe {
            let _ = Box::from_raw(animator);
        }
    }

    drop(guard2);
    drop(guard1);
    Ok(())
}
