use super::{Capability, Color, Console, Error, Palette, Result, ScrollDirection, State};

const CRTC_ADDRESS_REG: u16 = 0x3D4;
const CRTC_DATA_REG: u16 = 0x3D5;
const CRTC_CURSOR_START_REG: u8 = 0xA;
const CRTC_CURSOR_HIGH_REG: u8 = 0xE;
const CRTC_CURSOR_LOW_REG: u8 = 0xF;

#[derive(Clone, Copy)]
#[repr(C)]
struct VgaCell {
    ch: u8,
    attributes: u8,
}

pub struct VgaConsole {
    state: State,
    ptr: *mut VgaCell,
}

impl VgaConsole {
    const ROWS: usize = 25;
    const COLS: usize = 80;
    const PTR: *mut u16 = 0xB8000 as *mut u16;

    pub fn new() -> Self {
        Self::new_with_ptr(Self::PTR)
    }

    pub fn new_with_ptr(ptr: *mut u16) -> Self {
        Self {
            state: State::new(Self::COLS, Self::ROWS, Capability::CURSOR),
            ptr: ptr as *mut VgaCell,
        }
    }

    /// Clear to default attributes and move cursor to start.
    pub fn init(&mut self) {
        self.state.x = 0;
        self.state.y = 0;
        let _ = self.clear();
        let _ = self.enable_cursor(true);
        let _ = self.sync();
    }

    fn vga_color(&self, color: Color) -> u8 {
        match color {
            Color::Rgb(_, _, _) => unreachable!(),
            Color::Palette(color) => match color {
                Palette::Black => 0,
                Palette::Red => 4,
                Palette::Green => 2,
                Palette::Brown => 6,
                Palette::Blue => 1,
                Palette::Magenta => 5,
                Palette::Cyan => 3,
                Palette::LightGray => 7,
                Palette::DarkGray => 8,
                Palette::LightRed => 12,
                Palette::LightGreen => 10,
                Palette::Yellow => 14,
                Palette::LightBlue => 9,
                Palette::LightMagenta => 13,
                Palette::LightCyan => 11,
                Palette::White => 15,
            },
        }
    }

    fn attributes(&self) -> u8 {
        (self.vga_color(self.state.fg()) & 0xF) | ((self.vga_color(self.state.bg()) & 0xF) << 4)
    }

    fn index(&self) -> usize {
        self.state.y * self.state.width + self.state.x
    }

    fn cell_count(&self) -> usize {
        self.state.height * self.state.width
    }

    fn blank_cell(&self) -> VgaCell {
        VgaCell {
            ch: b' ',
            attributes: self.attributes(),
        }
    }
}

fn inb(port: u16) -> u8 {
    let mut v: u8;
    unsafe {
        core::arch::asm!("in al, dx", out("al")v, in("dx")port, options(nomem, nostack));
    }
    v
}

fn outb(port: u16, v: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx")port, in("al")v, options(nomem, nostack));
    }
}

impl Console for VgaConsole {
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

    fn blink_cursor(&mut self, _: Option<bool>) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn carriage_return(&mut self) -> Result<()> {
        self.state.x = 0;
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        let cell_count = self.cell_count();
        let mut index: usize = 0;
        let blank_cell = self.blank_cell();
        while index < cell_count {
            unsafe {
                self.ptr.add(index).write_volatile(blank_cell);
            }
            index += 1;
        }
        Ok(())
    }

    fn enable_cursor(&mut self, enabled: bool) -> Result<()> {
        outb(CRTC_ADDRESS_REG, CRTC_CURSOR_START_REG);
        let old_value = inb(CRTC_DATA_REG);
        let new_value = if enabled {
            old_value & !0x20
        } else {
            old_value | 0x20
        };
        if new_value != old_value {
            outb(CRTC_ADDRESS_REG, CRTC_CURSOR_START_REG);
            outb(CRTC_DATA_REG, new_value);
        }
        self.state.cursor.enabled = enabled;
        Ok(())
    }

    fn newline(&mut self) -> super::Result<()> {
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

        let width = self.state.width;
        let cells = self.state.height * width;
        let clear = width * rows;

        match direction {
            ScrollDirection::Down => {
                let mut dst = self.ptr;
                let mut src = unsafe { dst.add(clear) };

                for _ in 0..cells - clear {
                    unsafe {
                        dst.write_volatile(src.read_volatile());
                        dst = dst.add(1);
                        src = src.add(1);
                    }
                }

                let blank_cell = self.blank_cell();
                for _ in 0..clear {
                    unsafe {
                        dst.write_volatile(blank_cell);
                        dst = dst.add(1);
                    }
                }
            }
            ScrollDirection::Up => {
                let mut dst = unsafe { self.ptr.add(cells - 1) };
                let mut src = unsafe { dst.sub(clear) };

                for _ in 0..cells - clear {
                    unsafe {
                        dst.write_volatile(src.read_volatile());
                        dst = dst.sub(1);
                        src = src.sub(1);
                    }
                }

                let blank_cell = self.blank_cell();
                for _ in 0..clear {
                    unsafe {
                        dst.write_volatile(blank_cell);
                        dst = dst.sub(1);
                    }
                }
            }
        }
        Ok(())
    }

    fn state(&self) -> &State {
        &self.state
    }

    fn sync(&mut self) -> Result<()> {
        // Synchronize VGA hardware cursor.
        let index = self.index();
        outb(CRTC_ADDRESS_REG, CRTC_CURSOR_LOW_REG);
        outb(CRTC_DATA_REG, (index & 0xFF) as u8);
        outb(CRTC_ADDRESS_REG, CRTC_CURSOR_HIGH_REG);
        outb(CRTC_DATA_REG, ((index >> 8) & 0xFF) as u8);
        Ok(())
    }

    fn write(&mut self, s: &[u8]) -> Result<usize> {
        let cell_count = self.cell_count();
        let mut cell_index = self.index();
        let mut i: usize = 0;

        let reset_row: usize = (self.state.height - 1) * self.state.width;

        let mut cell = VgaCell {
            ch: 0,
            attributes: self.attributes(),
        };

        let len = s.len();
        while i < len {
            let target = i + core::cmp::min(len - i, cell_count - cell_index);
            while i < target {
                cell.ch = s[i];
                unsafe {
                    self.ptr.add(cell_index).write_volatile(cell);
                }
                cell_index += 1;
                i += 1;
            }
            if cell_index == cell_count {
                cell_index = reset_row;
                self.scroll(ScrollDirection::Down, 1)?;
            }
        }

        let width = self.state.width;
        self.state.x = cell_index % width;
        self.state.y = cell_index / width;

        Ok(len)
    }
}
