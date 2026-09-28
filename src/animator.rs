use anyhow::Context;
use rand::RngExt;
use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;
use rand::rngs::ThreadRng;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};

use crate::config::{Config, FPS_MS_FAST, FPS_MS_NORMAL, FPS_MS_SLOW};
use crate::loader::SpritesheetLoader;
use crate::renderer::Renderer;
use crate::utils::get_cursor_pos;

#[derive(Debug)]
enum _State {
    Idle(usize),
    Run { x: i32 },
    Lift { offset_x: i32, offset_y: i32 },
    Drop { v: i32 },
    Custom(String),
}

pub struct Animator {
    config: Config,
    loader: SpritesheetLoader,
    renderer: Box<Renderer>,
    rng: ThreadRng,
    max_x: i32,
    max_y: i32,
    state: _State,
    next_frame_ii: usize,
    current_position: POINT,
    idle_range: WeightedIndex<u16>,
}

impl Animator {
    pub fn new(
        config: Config,
        loader: SpritesheetLoader,
        renderer: Box<Renderer>,
    ) -> anyhow::Result<Self> {
        let max_x = renderer
            .screen_width()
            .saturating_sub(loader.frame_width().try_into()?);
        let max_y = renderer
            .screen_height()
            .saturating_sub(loader.frame_height().try_into()?);

        let mut rng = rand::rng();
        let initial_position = POINT {
            x: rng.random_range(0..=max_x),
            y: 0,
        };

        let idle_range = WeightedIndex::new(
            config
                .actions
                .idle
                .iter()
                .map(|action| action.weight)
                .collect::<Vec<u16>>(),
        )
        .context("Cannot construct distribution")?;

        let random_range = config.random_action_interval_secs;
        let timer_change_action = renderer.add_timer(
            rng.random_range(
                random_range.0.saturating_mul(1000)..=random_range.1.saturating_mul(1000),
            ),
            callback,
        );
        let timer_fps = renderer.add_timer(FPS_MS_FAST, callback);

        Ok(Self {
            config,
            loader,
            renderer,
            rng,
            max_x,
            max_y,
            state: _State::Drop { v: 0 },
            next_frame_ii: 0,
            current_position: initial_position,
            idle_range,
        })
    }

    fn set_state(&mut self, state: _State) {
        self.state = state;
        self.next_frame_ii = 0;
    }

    pub fn reset_change_action_timer(&mut self) -> anyhow::Result<()> {
        let random_range = self.config.random_action_interval_secs;
        let interval_ms = self.rng.random_range(
            random_range.0.saturating_mul(1000)..=random_range.1.saturating_mul(1000),
        );
        self.renderer
            .update_timer(TIMER_ID_CHANGE_ACTION, interval_ms)?;

        Ok(())
    }

    pub fn reset_fps_timer(&self, ms: u32) -> anyhow::Result<()> {
        self.renderer.update_timer(TIMER_ID_FPS, ms)?;

        Ok(())
    }

    pub fn idle(&mut self) -> anyhow::Result<()> {
        match &self.state {
            _State::Idle(..) => Ok(()),
            _State::Run { .. } | _State::Drop { .. } | _State::Custom(..) => {
                let index = self.idle_range.sample(&mut self.rng);
                self.set_state(_State::Idle(index));
                self.reset_fps_timer(FPS_MS_SLOW)?;
                Ok(())
            }
            other => {
                anyhow::bail!("Cannot transition to Idle from {other:?}");
            }
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        match &self.state {
            _State::Idle(..) | _State::Run { .. } => {
                let next_state = _State::Run {
                    x: self.rng.random_range(0..=self.max_x),
                };
                self.set_state(next_state);
                self.reset_fps_timer(FPS_MS_SLOW)?;
                Ok(())
            }
            other => {
                anyhow::bail!("Cannot transition to Run from {other:?}");
            }
        }
    }

    pub fn lift(&mut self, offset_x: i32, offset_y: i32) -> anyhow::Result<()> {
        self.set_state(_State::Lift { offset_x, offset_y });
        self.reset_fps_timer(FPS_MS_FAST)?;
        Ok(())
    }

    pub fn drop(&mut self) -> anyhow::Result<()> {
        match &self.state {
            _State::Lift { .. } => {
                self.set_state(_State::Drop { v: 0 });
                self.reset_fps_timer(FPS_MS_NORMAL)?;
                Ok(())
            }
            other => {
                anyhow::bail!("Cannot transition to Drop from {other:?}");
            }
        }
    }

    pub fn custom(&mut self, name: String) -> anyhow::Result<()> {
        match &self.state {
            _State::Idle(..) | _State::Run { .. } => {
                self.set_state(_State::Custom(name));
                Ok(())
            }
            other => {
                anyhow::bail!("Cannot transition to Custom({name:?}) from {other:?}");
            }
        }
    }

    /// Renders the next frame of the current action and updates the window.
    ///
    /// This function is called every frame, so it has to be extremely lightweight.
    pub fn render_next_frame(&mut self, window: HWND) -> anyhow::Result<()> {
        let mut new_position = self.current_position;
        let action = match &mut self.state {
            _State::Idle(index) => {
                &self
                    .config
                    .actions
                    .idle
                    .get(*index)
                    .context("Invalid idle action index")?
                    .action
            }
            _State::Run { x } => {
                let dx = x.saturating_sub(new_position.x);

                if dx == 0 {
                    self.idle().context("Cannot transition to Idle")?;
                    return self.render_next_frame(window);
                }

                let step = dx.abs().min(self.config.step_per_frame);
                if dx > 0 {
                    new_position.x = new_position.x.saturating_add(step);
                    &self.config.actions.run_right
                } else {
                    new_position.x = new_position.x.saturating_sub(step);
                    &self.config.actions.run_left
                }
            }
            _State::Lift { offset_x, offset_y } => {
                let cursor = get_cursor_pos().context("Cannot get cursor position")?;

                new_position.x = cursor.x.saturating_sub(*offset_x);
                new_position.y = cursor.y.saturating_sub(*offset_y);
                &self.config.actions.lift
            }
            _State::Drop { v } => {
                new_position.y = new_position.y.saturating_add(*v);
                *v = v.saturating_add(self.config.gravity);
                if new_position.y >= self.max_y {
                    new_position.y = self.max_y;
                    self.idle().context("Cannot transition to Idle")?;
                    return self.render_next_frame(window);
                }

                &self.config.actions.drop
            }
            _State::Custom(..) => {
                anyhow::bail!("Custom actions are not implemented yet");
            }
        };

        // If the action is not repeatable, we stay at the final frame.
        if self.next_frame_ii >= action.frames.len() && action.repeat {
            self.next_frame_ii = 0;
        }

        let old_left = self.current_position.x;
        let old_top = self.current_position.y;
        let old_right = old_left.saturating_add(self.loader.frame_width().try_into()?);
        let old_bottom = old_top.saturating_add(self.loader.frame_height().try_into()?);

        self.renderer.clear_pixels(&RECT {
            left: old_left,
            top: old_top,
            right: old_right,
            bottom: old_bottom,
        });

        let new_left = new_position.x;
        let new_top = new_position.y;
        let new_right = new_left.saturating_add(self.loader.frame_width().try_into()?);
        let new_bottom = new_top.saturating_add(self.loader.frame_height().try_into()?);

        if let Some(frame_index) = action.frames.get(self.next_frame_ii)
            && let Some(frame) = self.loader.frame(*frame_index)
        {
            self.renderer.draw_pixels(
                &RECT {
                    left: new_left,
                    top: new_top,
                    right: new_right,
                    bottom: new_bottom,
                },
                frame,
            )?;
            self.next_frame_ii = self.next_frame_ii.wrapping_add(1);
        }

        let dirty = RECT {
            left: new_position.x.min(self.current_position.x),
            top: new_position.y.min(self.current_position.y),
            right: new_position
                .x
                .saturating_add(self.loader.frame_width().try_into()?)
                .max(
                    self.current_position
                        .x
                        .saturating_add(self.loader.frame_width().try_into()?),
                ),
            bottom: new_position
                .y
                .saturating_add(self.loader.frame_height().try_into()?)
                .max(
                    self.current_position
                        .y
                        .saturating_add(self.loader.frame_height().try_into()?),
                ),
        };

        self.renderer
            .update(&dirty)
            .context("Cannot render next frame")?;
        self.current_position = new_position;

        Ok(())
    }
}
