use std::io::{BufRead, Seek};
use std::ops::{Div, Rem};

use anyhow::Context;
use image::ImageReader;

pub struct SpritesheetLoader {
    _frame_width: usize,
    _frame_height: usize,
    _frames: Vec<Vec<u32>>,
}

impl SpritesheetLoader {
    pub fn new<R>(reader: R, rows: usize, columns: usize) -> anyhow::Result<Self>
    where
        R: BufRead + Seek,
    {
        if rows == 0 || columns == 0 {
            anyhow::bail!("rows ({rows}) and columns ({columns}) must be positive");
        }

        let image = ImageReader::new(reader)
            .with_guessed_format()
            .context("Cannot read image")?
            .decode()
            .context("Cannot decode image")?
            .into_rgba8();

        let width = usize::try_from(image.width())?;
        let height = usize::try_from(image.height())?;

        let frame_height = height.div(rows);
        let frame_width = width.div(columns);

        /// [`UpdateLayeredWindow`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow)
        /// with [`AC_SRC_ALPHA`](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-blendfunction)
        fn convert_src_alpha(byte: u8, a: u8) -> u32 {
            u32::from(byte).saturating_mul(a.into()).div(255)
        }

        let frame_area = frame_height
            .checked_mul(frame_width)
            .context("Cannot calculate frame area")?;
        let frames_count = rows
            .checked_mul(columns)
            .context("Cannot calculate frames count")?;

        let mut frames = vec![vec![0; frame_area]; frames_count];
        for (index, pixel) in image.pixels().enumerate() {
            let [r, g, b, a] = pixel.0;
            let r = convert_src_alpha(r, a);
            let g = convert_src_alpha(g, a);
            let b = convert_src_alpha(b, a);

            let value = b | (g << 8) | (r << 16) | (u32::from(a) << 24);
            let x = index.rem(width);
            let y = index.div(width);

            let frame_x = x.div(frame_width);
            let frame_y = y.div(frame_height);
            let frame_index = frame_y
                .checked_mul(columns)
                .context("Cannot calculate frame_index")?
                .checked_add(frame_x)
                .context("Cannot calculate frame_index")?;

            let local_x = x.rem(frame_width);
            let local_y = y.rem(frame_height);
            let frame_pixel_index = local_y
                .checked_mul(frame_width)
                .context("Cannot calculate frame_pixel_index")?
                .checked_add(local_x)
                .context("Cannot calculate frame_pixel_index")?;

            frames[frame_index][frame_pixel_index] = value;
        }

        Ok(Self {
            _frame_width: frame_width,
            _frame_height: frame_height,
            _frames: frames,
        })
    }

    pub fn frame_width(&self) -> usize {
        self._frame_width
    }

    pub fn frame_height(&self) -> usize {
        self._frame_height
    }

    pub fn frame(&self, index: usize) -> Option<&[u32]> {
        Some(self._frames.get(index)?.as_slice())
    }
}
