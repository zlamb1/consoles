use super::{Capability, Color, Console, Error, Result, ScrollDirection, State};
use crate::fb::Framebuffer;
use crate::font::{BitmapFont, data::DEFAULT_FONT};
use core::cmp::min;

/// A framebuffer console designed for use with a memory-mapped framebuffer.
/// Write-combining memory is implicitly assumed and handled by Console::sync.
/// Currently only supports 32-bit pixel formats.
pub struct FbConsole<'a> {
    pub state: State,
    pub fb: Framebuffer,
    pub font: &'a BitmapFont<'a>,
    pub palette: [(u8, u8, u8); 16],
}

impl<'a> FbConsole<'a> {
    pub fn new(fb: Framebuffer, font: Option<&'a BitmapFont<'a>>) -> Option<Self> {
        let font = font.unwrap_or(DEFAULT_FONT);
        let width = fb.width / font.width;
        let height = fb.height / font.height;

        if fb.bpp != 32
            || fb.red_mask.shift > 31
            || fb.green_mask.shift > 31
            || fb.blue_mask.shift > 31
        {
            return None;
        }

        let min_row_size = fb.width * 4;
        if fb.pitch < min_row_size {
            return None;
        }

        Some(Self {
            state: State::new(
                width,
                height,
                Capability::RGB | Capability::CURSOR | Capability::BLINK,
            ),
            fb,
            font,
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
        })
    }

    /// Clear to default attributes and move cursor to start.
    pub fn init(&mut self) {
        self.state.x = 0;
        self.state.y = 0;
        let _ = self.clear();
        let _ = self.enable_cursor(true);
        let _ = self.sync();
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
}

impl Console for FbConsole<'_> {
    fn backspace(&mut self) -> Result<()> {
        if self.state.x > 0 {
            self.state.x -= 1;
        } else {
            if self.state.y == 0 {
                return Err(Error::Invalid);
            }
            self.state.y -= 1;
            self.state.x = self.state.width - 1;
        }
        Ok(())
    }

    fn blink_cursor(&mut self, visible: Option<bool>) -> Result<()> {
        let visible = visible.unwrap_or(!self.state.cursor.visible);
        let color = if visible {
            self.color(self.state.fg())
        } else {
            self.color(self.state.bg())
        };
        let color = color.to_ne_bytes();
        let font = self.font;
        let mut index = self.state.y * font.height * self.fb.pitch + self.state.x * font.width * 4;

        for _ in 0..font.height {
            unsafe {
                let ptr = self.fb.ptr.add(index);
                ptr.write_volatile(color[0]);
                ptr.add(1).write_volatile(color[1]);
                ptr.add(2).write_volatile(color[2]);
            }
            index += self.fb.pitch;
        }

        self.state.cursor.visible = visible;
        Ok(())
    }

    fn carriage_return(&mut self) -> Result<()> {
        self.state.x = 0;
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        let bg = self.color(self.state.bg()).to_ne_bytes();

        let width = self.fb.width;
        let height = self.fb.height;
        // Amount of bytes between rows.
        let pad = self.fb.pitch - width * 4;
        let mut ptr = self.fb.ptr;

        for _ in 0..height {
            for _ in 0..width {
                unsafe {
                    ptr.write_volatile(bg[0]);
                    ptr.add(1).write_volatile(bg[1]);
                    ptr.add(2).write_volatile(bg[2]);
                    ptr = ptr.add(4);
                }
            }
            unsafe {
                ptr = ptr.add(pad);
            }
        }

        Ok(())
    }

    fn enable_cursor(&mut self, enabled: bool) -> Result<()> {
        let cursor = self.state.cursor;

        if !enabled && cursor.visible {
            // Make the cursor invisible before disabling it.
            self.blink_cursor(Some(false))?;
        }
        self.state.cursor.enabled = enabled;

        Ok(())
    }

    fn newline(&mut self) -> Result<()> {
        self.state.x = 0;
        self.state.y += 1;
        if self.state.y >= self.state.height {
            self.state.y = self.state.height - 1;
            self.scroll(ScrollDirection::Down, 1)?;
        }
        Ok(())
    }

    fn move_cursor(&mut self, x: usize, y: usize) -> Result<()> {
        if x >= self.state.width || y >= self.state.height {
            return Err(Error::Invalid);
        }
        self.state.x = x;
        self.state.y = y;
        Ok(())
    }

    fn scroll(&mut self, direction: ScrollDirection, rows: usize) -> Result<()> {
        if rows >= self.state.height {
            return self.clear();
        }

        // Bytes per pixel.
        let bpp = self.fb.bpp / 8;
        let pad = self.fb.pitch - self.fb.width * bpp;
        let pitch = self.fb.pitch;

        let clear = self.font.height * rows;
        let bg = self.color(self.state.bg()).to_ne_bytes();

        match direction {
            ScrollDirection::Down => {
                let mut dst = self.fb.ptr;
                let mut src = unsafe { dst.add(clear * pitch) };

                for _ in 0..self.fb.height - clear {
                    for _ in 0..self.fb.width * bpp {
                        unsafe {
                            dst.write_volatile(src.read_volatile());
                            dst = dst.add(1);
                            src = src.add(1);
                        }
                    }
                    unsafe {
                        dst = dst.add(pad);
                        src = src.add(pad);
                    }
                }

                for _ in 0..clear {
                    for _ in 0..self.fb.width {
                        for i in 0..bpp {
                            unsafe {
                                dst.write_volatile(bg[i]);
                                dst = dst.add(1);
                            }
                        }
                    }
                    unsafe {
                        dst = dst.add(pad);
                    }
                }
            }
            ScrollDirection::Up => {
                let mut dst = unsafe { self.fb.ptr.add(self.fb.height * pitch - pad - 1) };
                let mut src = unsafe { dst.sub(clear * pitch) };

                for _ in 0..self.fb.height - clear {
                    for _ in 0..self.fb.width * bpp {
                        unsafe {
                            dst.write_volatile(src.read_volatile());
                            dst = dst.sub(1);
                            src = src.sub(1);
                        }
                    }
                    unsafe {
                        dst = dst.sub(pad);
                        src = src.sub(pad);
                    }
                }

                for _ in 0..clear {
                    for _ in 0..self.fb.width {
                        for i in 0..bpp {
                            unsafe {
                                dst.write_volatile(bg[bpp - 1 - i]);
                                dst = dst.sub(1);
                            }
                        }
                    }
                    unsafe {
                        dst = dst.sub(pad);
                    }
                }
            }
        }
        Ok(())
    }

    fn state(&self) -> &State {
        &self.state
    }

    fn write(&mut self, s: &[u8]) -> Result<usize> {
        let visible = self.state.cursor.visible;
        let len = s.len();

        let font: BitmapFont = *self.font;
        // Bytes per glyph row.
        let font_bpr = (font.width + 7) / 8;
        // Bytes per glyph.
        let font_bpg = font_bpr * font.height;

        let pixel_advance = font.width * 4;
        let mut pixel_index: usize =
            self.state.y * font.height * self.fb.pitch + self.state.x * pixel_advance;

        let cell_count: usize = self.state.width * self.state.height;
        let mut cell_index: usize = self.state.y * self.state.width + self.state.x;
        let mut i: usize = 0;

        let reset_row = self.state.height - 1;
        let reset_pixel_index = reset_row * font.height * self.fb.pitch;
        let reset_cell_index = reset_row * self.state.width;

        let fg = self.color(self.state.fg()).to_ne_bytes();
        let bg = self.color(self.state.bg()).to_ne_bytes();

        if visible {
            // Don't leave the cursor behind before we move it.
            self.blink_cursor(Some(false))?;
        }

        while i < len {
            let target = i + min(len - i, cell_count - cell_index);
            while i < target {
                let ch = s[i];
                // Points at the start of each row.
                let mut row_pixel = unsafe { self.fb.ptr.add(pixel_index) };
                let glyph = &font.data[ch as usize * font_bpg..][..font_bpg];
                for gy in 0..font.height {
                    let mut pixel = row_pixel;
                    for gx in 0..font.width {
                        let is_set = (glyph[gy * font_bpr + gx / 8] << (gx & 7)) & 0x80 == 0x80;
                        let color = if is_set { fg } else { bg };
                        unsafe {
                            pixel.write_volatile(color[0]);
                            pixel.add(1).write_volatile(color[1]);
                            pixel.add(2).write_volatile(color[2]);
                            pixel = pixel.add(4);
                        }
                    }
                    row_pixel = unsafe { row_pixel.add(self.fb.pitch) };
                }
                cell_index += 1;
                if cell_index % self.state.width != 0 {
                    pixel_index += pixel_advance;
                } else {
                    // Move to the next row.
                    pixel_index = (cell_index / self.state.width) * font.height * self.fb.pitch;
                }
                i += 1;
            }
            if cell_index == cell_count {
                pixel_index = reset_pixel_index;
                cell_index = reset_cell_index;
                self.scroll(ScrollDirection::Down, 1)?;
            }
        }

        self.state.x = cell_index % self.state.width;
        self.state.y = cell_index / self.state.width;

        if visible {
            // If the cursor was visible before, render at the new position.
            self.blink_cursor(Some(true))?;
        }

        Ok(len)
    }

    fn sync(&mut self) -> Result<()> {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        #[cfg(not(miri))]
        unsafe {
            core::arch::asm!("sfence", options(nomem, nostack));
        }
        Ok(())
    }
}
