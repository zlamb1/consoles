use crate::cell_grid::{CellGrid, CellWriter};
use crate::color::{Color, Palette};
use crate::cursor::Cursor;
use crate::simple::{CellConsole, ColorConsole, Console, CursorConsole, Error, Result};

use x86::{inb, outb};

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

struct VgaBackend {
    ptr: *mut VgaCell,
    fg: Color,
    bg: Color,
}

impl VgaBackend {
    fn attributes(&self) -> u8 {
        (self.vga_color(self.fg) & 0xF) | ((self.vga_color(self.bg) & 0xF) << 4)
    }

    fn clear(&mut self, cell_grid: &CellGrid) -> Result<()> {
        let blank_cell = VgaCell {
            ch: b' ',
            attributes: self.attributes(),
        };
        let mut ptr = self.ptr;
        for _ in 0..cell_grid.cell_count() {
            unsafe {
                ptr.write_volatile(blank_cell);
                ptr = ptr.add(1);
            }
        }
        Ok(())
    }

    #[inline(always)]
    fn get_index(&self, cell_grid: &CellGrid) -> usize {
        cell_grid.cell_index()
    }

    fn new(ptr: *mut VgaCell) -> Self {
        Self {
            ptr,
            fg: Color::Palette(Palette::White),
            bg: Color::Palette(Palette::Black),
        }
    }

    fn scroll(&mut self, cell_grid: &CellGrid) -> Result<()> {
        let cell_count = cell_grid.cell_count();
        let clear = cell_grid.width;
        let blank_cell = VgaCell {
            ch: b' ',
            attributes: self.attributes(),
        };

        let mut dst = self.ptr;
        let mut src = unsafe { dst.add(clear) };

        for _ in 0..cell_count - clear {
            unsafe {
                dst.write_volatile(src.read_volatile());
                dst = dst.add(1);
                src = src.add(1);
            }
        }

        for _ in 0..clear {
            unsafe {
                dst.write_volatile(blank_cell);
                dst = dst.add(1);
            }
        }

        Ok(())
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

    fn write_cell(&mut self, cell_grid: &CellGrid, cell_index: usize, ch: u8) -> Result<()> {
        debug_assert!(cell_index < cell_grid.cell_count());
        let vga_cell = VgaCell {
            ch,
            attributes: self.attributes(),
        };
        unsafe {
            self.ptr.add(cell_index).write_volatile(vga_cell);
        }
        Ok(())
    }
}

pub struct VgaConsole {
    backend: VgaBackend,
    cell_grid: CellGrid,
    cursor: Cursor,
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
            backend: VgaBackend::new(ptr as *mut VgaCell),
            cell_grid: CellGrid::new(Self::COLS, Self::ROWS),
            cursor: Cursor::new(),
        }
    }
}

impl Console for VgaConsole {
    fn clear(&mut self) -> Result<()> {
        self.backend.clear(&self.cell_grid)
    }

    fn flush(&mut self) -> Result<()> {
        // Synchronize VGA hardware cursor.
        let cell_index = self.cell_grid.cell_index();
        debug_assert!(cell_index < self.cell_grid.cell_count());
        outb(CRTC_ADDRESS_REG, CRTC_CURSOR_LOW_REG);
        outb(CRTC_DATA_REG, (cell_index & 0xFF) as u8);
        outb(CRTC_ADDRESS_REG, CRTC_CURSOR_HIGH_REG);
        outb(CRTC_DATA_REG, ((cell_index >> 8) & 0xFF) as u8);
        Ok(())
    }

    fn write(&mut self, s: &[u8]) -> Result<usize> {
        let mut cell_writer = CellWriter::new(
            &mut self.backend,
            VgaBackend::get_index,
            VgaBackend::write_cell,
            VgaBackend::scroll,
        );
        cell_writer.write(&mut self.cell_grid, s)
    }
}

impl CellConsole for VgaConsole {
    fn cell_grid(&self) -> &CellGrid {
        &self.cell_grid
    }

    fn position(&mut self, x: usize, y: usize) -> Result<()> {
        if x >= self.cell_grid.width || y >= self.cell_grid.height {
            return Err(Error::Invalid);
        }
        self.cell_grid.x = x;
        self.cell_grid.y = y;
        Ok(())
    }

    fn scroll(&mut self) -> Result<()> {
        self.backend.scroll(&self.cell_grid)
    }
}

impl CursorConsole for VgaConsole {
    fn enable(&mut self, enable: bool) -> Result<()> {
        outb(CRTC_ADDRESS_REG, CRTC_CURSOR_START_REG);
        let old_value = inb(CRTC_DATA_REG);
        let new_value = if enable {
            old_value & !0x20
        } else {
            old_value | 0x20
        };
        if new_value != old_value {
            outb(CRTC_ADDRESS_REG, CRTC_CURSOR_START_REG);
            outb(CRTC_DATA_REG, new_value);
        }
        self.cursor.enabled = enable;
        Ok(())
    }
}

impl ColorConsole for VgaConsole {
    fn fg(&self) -> Color {
        self.backend.fg
    }

    fn bg(&self) -> Color {
        self.backend.bg
    }

    fn set_fg(&mut self, fg: Color) -> Result<()> {
        match fg {
            Color::Rgb(_, _, _) => return Err(Error::Unsupported),
            Color::Palette(_) => {}
        }
        self.backend.fg = fg;
        Ok(())
    }

    fn set_bg(&mut self, bg: Color) -> Result<()> {
        match bg {
            Color::Rgb(_, _, _) => return Err(Error::Unsupported),
            Color::Palette(_) => {}
        }
        self.backend.bg = bg;
        Ok(())
    }
}
