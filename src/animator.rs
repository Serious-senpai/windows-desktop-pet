use std::io::Error;

use anyhow::Context;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SetTimer,
};

use crate::config::{Config, TIMER_ID_CHANGE_ACTION};
use crate::loader::SpritesheetLoader;
use crate::renderer::Renderer;

enum _Action {
    Idle(usize),
    Run { target: POINT },
}

pub struct Animator {
    _config: Config,
    _loader: SpritesheetLoader,
    _renderer: Renderer,
    _max_x: i32,
    _max_y: i32,
    _action: _Action,
    _next_frame_ii: usize,
    _current_position: POINT,
}

impl Animator {
    pub fn new(
        config: Config,
        loader: SpritesheetLoader,
        renderer: Renderer,
    ) -> anyhow::Result<Self> {
        let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if screen_width <= 0 || screen_height <= 0 {
            anyhow::bail!("GetSystemMetrics error");
        }

        let max_x = screen_width.saturating_sub(loader.frame_width().try_into()?);
        let max_y = screen_height.saturating_sub(loader.frame_height().try_into()?);
        let initial_position = POINT {
            x: config.initial_position_xy.0.clamp(0, max_x),
            y: config.initial_position_xy.1.clamp(0, max_y),
        };

        Ok(Self {
            _config: config,
            _loader: loader,
            _renderer: renderer,
            _max_x: max_x,
            _max_y: max_y,
            _action: _Action::Idle(0),
            _next_frame_ii: 0,
            _current_position: initial_position,
        })
    }

    fn _set_action(&mut self, action: _Action) {
        self._action = action;
        self._next_frame_ii = 0;
    }

    fn _random_idle_action(&self) -> _Action {
        _Action::Idle(rand::random_range(0..self._config.actions.idle.len()))
    }

    pub fn reset_change_action_timer(&self, window: HWND) -> anyhow::Result<()> {
        let random_range = self._config.random_action_interval_secs;
        let change_action_ms = rand::random_range(
            random_range.0.saturating_mul(1000)..random_range.1.saturating_mul(1000),
        );
        if unsafe { SetTimer(window, TIMER_ID_CHANGE_ACTION, change_action_ms, None) } == 0 {
            return Err(Error::last_os_error()).context("SetTimer error");
        }

        Ok(())
    }

    pub fn next_action(&mut self) -> anyhow::Result<()> {
        match self._action {
            _Action::Idle(..) => {
                let target = POINT {
                    x: rand::random_range(0..self._max_x),
                    y: if self._config.gravity {
                        self._max_y
                    } else {
                        rand::random_range(0..self._max_y)
                    },
                };

                self._set_action(_Action::Run { target });
            }
            _Action::Run { .. } => {
                self._set_action(self._random_idle_action());
            }
        }

        Ok(())
    }

    /// Renders the next frame of the current action and updates the window.
    ///
    /// This function is called every frame, so it has to be extremely lightweight.
    pub fn render_next_frame(&mut self, window: HWND) -> anyhow::Result<()> {
        let mut dx = 0;
        let mut dy = 0;
        let action = match self._action {
            _Action::Idle(index) => &self
                ._config
                .actions
                .idle
                .get(index)
                .context("Invalid idle action index")?,
            _Action::Run { target } => {
                dx = target.x.saturating_sub(self._current_position.x);
                dy = target.y.saturating_sub(self._current_position.y);

                if self._current_position.x < target.x {
                    &self._config.actions.run_right
                } else if self._current_position.x > target.x {
                    &self._config.actions.run_left
                } else {
                    self.next_action()?;
                    return Ok(());
                }
            }
        };

        if dx != 0 || dy != 0 {
            let step_x;
            let step_y;
            if dx.abs() > dy.abs() {
                step_x = self._config.step_per_frame.min(dx.abs());
                step_y = step_x.saturating_mul(dy).saturating_div(dx).abs();
            } else {
                step_y = self._config.step_per_frame.min(dy.abs());
                step_x = step_y.saturating_mul(dx).saturating_div(dy).abs();
            }

            if dx > 0 {
                self._current_position.x = self._current_position.x.saturating_add(step_x);
            } else {
                self._current_position.x = self._current_position.x.saturating_sub(step_x);
            }

            if dy > 0 {
                self._current_position.y = self._current_position.y.saturating_add(step_y);
            } else {
                self._current_position.y = self._current_position.y.saturating_sub(step_y);
            }
        }

        if self._next_frame_ii >= action.frames.len() {
            if action.repeat {
                self._next_frame_ii = 0;
            } else {
                // If the action is not repeatable, we switch back to idle action.
                self._set_action(self._random_idle_action());
                return Ok(());
            }
        }

        if let Some(frame_index) = action.frames.get(self._next_frame_ii)
            && let Some(frame) = self._loader.frame(*frame_index)
        {
            self._renderer.pixels_mut().copy_from_slice(frame);
            self._next_frame_ii = self._next_frame_ii.wrapping_add(1);

            self._renderer
                .update(window, &self._current_position)
                .context("Cannot render next frame")?;
        }

        Ok(())
    }
}
