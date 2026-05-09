use crate::cell_grid::{CellGrid, CellWriter};
use crate::color::{Color, Palette};
use crate::cursor::{Cursor, with_cursor_hidden};
use crate::fb::Framebuffer;
use crate::font::{BitmapFont, data::DEFAULT_FONT};
use crate::simple::{
    BlinkConsole, CellConsole, ColorConsole, Console, CursorConsole, Error, Result,
};

use core::cmp::min;

struct Backend<'a> {
    pub fb: Framebuffer,
    pub font: &'a BitmapFont<'a>,
    pub cursor: Cursor,
    pub fg: Color,
    pub bg: Color,
    pub palette: [(u8, u8, u8); 16],
    /// Glyph width in bytes.
    pub glyph_advance: usize,
    /// Bytes to get the next row's glyph.
    pub glyph_pitch: usize,
}

impl<'a> Backend<'a> {
    fn blink(&mut self, cell_grid: &CellGrid) {
        if !self.cursor.enabled {
            return;
        }
        let visible = !self.cursor.visible;
        let color = if visible { self.fg } else { self.bg };
        let color = self.color(color).to_ne_bytes();
        let bytes_per_pixel = self.fb.bpp / 8;

        let mut pixel = self.get_index(cell_grid);

        for _ in 0..self.font.height {
            for i in 0..bytes_per_pixel {
                unsafe {
                    pixel.add(i).write_volatile(color[i]);
                }
            }
            pixel = unsafe { pixel.add(self.fb.pitch) };
        }

        self.cursor.visible = visible;
    }

    fn clear(&mut self) -> Result<()> {
        let bg = self.color(self.bg).to_ne_bytes();
        let bytes_per_pixel = self.fb.bpp / 8;
        let pad = self.fb.pitch - self.fb.width * bytes_per_pixel;

        let mut pixel = self.fb.ptr;

        for _ in 0..self.fb.height {
            for _ in 0..self.fb.width {
                for i in 0..bytes_per_pixel {
                    unsafe {
                        pixel.write_volatile(bg[i]);
                        pixel = pixel.add(1)
                    }
                }
            }
            pixel = unsafe { pixel.add(pad) };
        }

        Ok(())
    }

    fn color(&self, color: Color) -> u32 {
        let rgb = match color {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Palette(color) => self.palette[color as usize],
        };
        let red_mask = self.fb.red_mask;
        let green_mask = self.fb.green_mask;
        let blue_mask = self.fb.blue_mask;

        // We just clip any bottom bits, which will produce banding.
        // We don't support dithering.
        let mut color: u32 = 0;
        color |= ((rgb.0 as u32) >> (8 - min(red_mask.size, 8))) << red_mask.shift;
        color |= ((rgb.1 as u32) >> (8 - min(green_mask.size, 8))) << green_mask.shift;
        color |= ((rgb.2 as u32) >> (8 - min(blue_mask.size, 8))) << blue_mask.shift;

        color
    }

    fn new(fb: Framebuffer, font: Option<&'a BitmapFont<'a>>) -> Option<Self> {
        let font = font.unwrap_or(DEFAULT_FONT);

        if (fb.bpp != 8 && fb.bpp != 16 && fb.bpp != 24 && fb.bpp != 32)
            || fb.red_mask.shift > 31
            || fb.green_mask.shift > 31
            || fb.blue_mask.shift > 31
        {
            return None;
        }

        let min_row_size = fb.width * (fb.bpp / 8);
        if fb.pitch < min_row_size {
            return None;
        }

        Some(Self {
            fb,
            font,
            cursor: Cursor::new(),
            fg: Color::Palette(Palette::White),
            bg: Color::Palette(Palette::Black),
            palette: [
                (0, 0, 0),
                (128, 0, 0),
                (0, 128, 0),
                (128, 128, 0),
                (0, 0, 128),
                (128, 0, 128),
                (0, 128, 128),
                (192, 192, 192),
                (128, 128, 128),
                (255, 0, 0),
                (0, 255, 0),
                (255, 255, 0),
                (0, 0, 255),
                (255, 0, 255),
                (0, 255, 255),
                (255, 255, 255),
            ],
            glyph_advance: font.width * (fb.bpp / 8),
            glyph_pitch: font.height * fb.pitch,
        })
    }

    #[inline(always)]
    fn get_index(&self, cell_grid: &CellGrid) -> *mut u8 {
        debug_assert!(cell_grid.x < cell_grid.width);
        debug_assert!(cell_grid.y < cell_grid.height);

        unsafe {
            self.fb
                .ptr
                .add(cell_grid.y * self.glyph_pitch + cell_grid.x * self.glyph_advance)
        }
    }

    fn scroll(&mut self, _: &CellGrid) -> Result<()> {
        let clear = self.font.height;
        let bytes_per_pixel = self.fb.bpp / 8;
        let pad = self.fb.pitch - self.fb.width * bytes_per_pixel;
        let bg = self.color(self.bg).to_ne_bytes();

        let mut dst = self.fb.ptr;
        let mut src = unsafe { dst.add(clear * self.fb.pitch) };

        for _ in 0..self.fb.height - clear {
            for _ in 0..self.fb.width {
                for _ in 0..bytes_per_pixel {
                    unsafe {
                        dst.write_volatile(src.read_volatile());
                        dst = dst.add(1);
                        src = src.add(1);
                    }
                }
            }
            unsafe {
                dst = dst.add(pad);
                src = src.add(pad);
            }
        }

        for _ in 0..clear {
            for _ in 0..self.fb.width {
                for i in 0..bytes_per_pixel {
                    unsafe {
                        dst.write_volatile(bg[i]);
                        dst = dst.add(1);
                    }
                }
            }
            unsafe { dst = dst.add(pad) };
        }

        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (
            self.fb.width / self.font.width,
            self.fb.height / self.font.height,
        )
    }

    fn write_cell(&mut self, _: &CellGrid, pixel: *mut u8, ch: u8) -> Result<()> {
        let fg = self.color(self.fg).to_ne_bytes();
        let bg = self.color(self.bg).to_ne_bytes();
        let font = self.font;
        let bytes_per_pixel = self.fb.bpp / 8;

        // Bytes per row in glyph.
        let bpr = (font.width + 7) / 8;
        // Bytes per glyph.
        let bpg = bpr * font.height;
        let glyph = &font.data[bpg * ch as usize..][..bpg];
        let mut pixel = pixel;

        for gy in 0..font.height {
            let mut cell_pixel = pixel;
            for gx in 0..font.width {
                let is_set = (glyph[gy * bpr + gx / 8] << (gx & 7)) & 0x80 == 0x80;
                let color = if is_set { fg } else { bg };

                for i in 0..bytes_per_pixel {
                    unsafe {
                        cell_pixel.write_volatile(color[i]);
                        cell_pixel = cell_pixel.add(1);
                    }
                }
            }
            pixel = unsafe { pixel.add(self.fb.pitch) };
        }

        Ok(())
    }
}

/// A framebuffer console designed for use with a memory-mapped framebuffer.
/// Write-combining memory is implicitly assumed and handled by Console::flush.
pub struct FbConsole<'a> {
    pub cell_grid: CellGrid,
    backend: Backend<'a>,
}

impl<'a> FbConsole<'a> {
    pub fn new(fb: Framebuffer, font: Option<&'a BitmapFont<'a>>) -> Option<Self> {
        let backend = Backend::new(fb, font)?;
        let size = backend.size();
        Some(Self {
            cell_grid: CellGrid::new(size.0, size.1),
            backend,
        })
    }
}

impl Console for FbConsole<'_> {
    fn clear(&mut self) -> Result<()> {
        self.backend.clear()
    }

    fn flush(&mut self) -> Result<()> {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        #[cfg(not(miri))]
        unsafe {
            core::arch::asm!("sfence", options(nomem, nostack));
        }
        Ok(())
    }

    fn write(&mut self, s: &[u8]) -> Result<usize> {
        let backend = &mut self.backend;
        let cursor = backend.cursor;

        with_cursor_hidden(
            backend,
            &mut self.cell_grid,
            cursor,
            |backend, cell_grid| {
                let mut cell_writer = CellWriter::new(
                    backend,
                    Backend::get_index,
                    Backend::write_cell,
                    Backend::scroll,
                );
                cell_writer.write(cell_grid, s)
            },
            Backend::blink,
        )
    }
}

impl CellConsole for FbConsole<'_> {
    fn cell_grid(&self) -> &CellGrid {
        &self.cell_grid
    }

    fn position(&mut self, x: usize, y: usize) -> Result<()> {
        if x >= self.cell_grid.width || y >= self.cell_grid.height {
            return Err(Error::Invalid);
        }
        let cursor = self.backend.cursor;
        with_cursor_hidden(
            &mut self.backend,
            &mut self.cell_grid,
            cursor,
            |_, cell_grid| {
                cell_grid.x = x;
                cell_grid.y = y;
                Ok(())
            },
            Backend::blink,
        )
    }

    fn scroll(&mut self) -> Result<()> {
        let cursor = self.backend.cursor;
        with_cursor_hidden(
            &mut self.backend,
            &mut self.cell_grid,
            cursor,
            |backend, cell_grid| backend.scroll(cell_grid),
            Backend::blink,
        )
    }
}

impl CursorConsole for FbConsole<'_> {
    fn enable(&mut self, enable: bool) -> Result<()> {
        if !enable && self.backend.cursor.visible {
            self.backend.blink(&self.cell_grid);
        }
        self.backend.cursor.enabled = enable;
        Ok(())
    }
}

impl BlinkConsole for FbConsole<'_> {
    fn blink(&mut self) {
        self.backend.blink(&self.cell_grid)
    }

    fn visible(&self) -> bool {
        self.backend.cursor.visible
    }
}

impl ColorConsole for FbConsole<'_> {
    fn fg(&self) -> Color {
        self.backend.fg
    }

    fn bg(&self) -> Color {
        self.backend.bg
    }

    fn set_fg(&mut self, fg: Color) -> Result<()> {
        self.backend.fg = fg;
        Ok(())
    }

    fn set_bg(&mut self, bg: Color) -> Result<()> {
        self.backend.bg = bg;
        Ok(())
    }
}
