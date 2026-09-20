use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;
use widestring::{U16CStr, u16cstr};
use windows_sys::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
use windows_sys::Win32::UI::WindowsAndMessaging::WM_USER;
use windows_sys::w;

pub const WINDOW_CLASS_NAME: *const u16 = w!("WindowsDesktopPetClass");
pub const DEFAULT_BLENDFUNCTION: BLENDFUNCTION = BLENDFUNCTION {
    BlendOp: AC_SRC_OVER as u8,
    BlendFlags: 0,
    SourceConstantAlpha: 255,
    AlphaFormat: AC_SRC_ALPHA as u8,
};
pub const TRAY_ICON_ID: u32 = 1;
pub const WM_TRAYICON: u32 = WM_USER + 1;
pub const TRAY_ICON_TOOLTIP: &U16CStr = u16cstr!("Windows Desktop Pet");

pub const MENU_EXIT: usize = 1;

pub const TIMER_ID_RENDER: usize = 1;
pub const TIMER_ID_CHANGE_ACTION: usize = 2;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub spritesheet_path: PathBuf,
    pub rows: usize,
    pub columns: usize,
    pub initial_position_xy: (i32, i32),
    pub random_action_interval_secs: (u32, u32),
    pub step_per_frame: i32,
    pub interval_ms: u32,
    pub gravity: bool,
    pub actions: Actions,
}

impl Config {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.rows == 0 || self.columns == 0 {
            anyhow::bail!("Rows and columns must be greater than 0");
        }

        if self.initial_position_xy.0 < 0 || self.initial_position_xy.1 < 0 {
            anyhow::bail!("Initial position must be non-negative");
        }

        if self.random_action_interval_secs.0 > self.random_action_interval_secs.1 {
            anyhow::bail!("Random action interval min must be less than or equal to max");
        }

        if self.random_action_interval_secs.0 == 0 {
            anyhow::bail!("Random action interval min must be greater than 0");
        }

        if self.step_per_frame <= 0 {
            anyhow::bail!("Step per frame must be greater than 0");
        }

        if self.interval_ms == 0 {
            anyhow::bail!("Interval must be greater than 0");
        }

        self.actions.check().context("Actions are invalid")?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Actions {
    pub idle: Vec<Action>,
    pub run_right: Action,
    pub run_left: Action,
}

impl Actions {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.idle.is_empty() {
            anyhow::bail!("Idle actions must not be empty");
        }

        for action in &self.idle {
            action.check().context("Idle action is invalid")?;
        }
        self.run_right
            .check()
            .context("Run right action is invalid")?;
        self.run_left
            .check()
            .context("Run left action is invalid")?;

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Action {
    pub frames: Vec<usize>,
    pub repeat: bool,
}

impl Action {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.frames.is_empty() {
            anyhow::bail!("Frames must not be empty");
        }

        Ok(())
    }
}
