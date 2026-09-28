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

pub const TIMER_ID_FPS: usize = 1;
pub const TIMER_ID_START_RUNNING: usize = 2;

pub const FPS_MS_MAXIMUM_ACTION: u64 = 1000 / 4; // 4 FPS
pub const FPS_MS_SLOW: u32 = 1000 / 4; // 4 FPS
pub const FPS_MS_NORMAL: u32 = 1000 / 15; // 15 FPS
pub const FPS_MS_FAST: u32 = 1000 / 25; // 25 FPS

#[derive(Debug, Deserialize)]
pub struct Config {
    pub spritesheet_path: PathBuf,
    pub rows: usize,
    pub columns: usize,
    pub random_action_interval_secs: (u32, u32),
    pub step_per_frame: i32,
    pub gravity: i32,
    pub actions: Actions,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.rows == 0 || self.columns == 0 {
            anyhow::bail!("Rows and columns must be greater than 0");
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

        if self.gravity <= 0 {
            anyhow::bail!("Gravity must be greater than 0");
        }

        self.actions.validate().context("Actions are invalid")?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Actions {
    pub idle: Vec<WeightedAction>,
    pub run_right: Action,
    pub run_left: Action,
    pub lift: Action,
    pub drop: Action,
}

impl Actions {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.idle.is_empty() {
            anyhow::bail!("Idle actions must not be empty");
        }
        for action in &self.idle {
            action.validate().context("Idle action is invalid")?;
        }

        self.run_right
            .validate()
            .context("Run right action is invalid")?;
        self.run_left
            .validate()
            .context("Run left action is invalid")?;
        self.lift.validate().context("Lift action is invalid")?;
        self.drop.validate().context("Drop action is invalid")?;

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct WeightedAction {
    pub action: Action,
    pub weight: u16,
}

impl WeightedAction {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.action.validate().context("Action is invalid")?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Action {
    pub frames: Vec<usize>,
    pub repeat: bool,
}

impl Action {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.frames.is_empty() {
            anyhow::bail!("Frames must not be empty");
        }

        Ok(())
    }
}
