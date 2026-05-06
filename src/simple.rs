use crate::color::Color;
use core::{cell::Cell, unreachable};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unsupported,
    Invalid,
    /// A write is no longer making progress.
    Progress,
    /// The implementation is buggy.
    Implementation,
}

pub type Result<T> = core::result::Result<T, Error>;

/// Marker struct for capability bit flags.  
pub struct Capability;

impl Capability {
    /// Supports RGB colors beyond the normal 16-bit palette.
    const RGB: usize = 0x1;
    /// Supports cursor via Console::enable_cursor.
    const CURSOR: usize = 0x2;
    /// Supports blinking via Console::blink_cursor.
    const BLINK: usize = 0x4;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    /// Cursor is enabled or disabled.
    pub enabled: bool,
    /// Cursor is blinking or not. Only meaningful if the console
    /// has the BLINK capability.
    pub blinking: bool,
}

#[derive(Debug)]
pub struct State {
    pub width: usize,
    pub height: usize,
    pub x: usize,
    pub y: usize,
    capabilities: usize,
    fg: Cell<Color>,
    bg: Cell<Color>,
    pub cursor: Cursor,
}

impl State {
    pub fn new(width: usize, height: usize, capabilities: usize) -> Self {
        Self {
            width,
            height,
            x: 0,
            y: 0,
            capabilities,
            fg: Cell::new(Color::White),
            bg: Cell::new(Color::Black),
            cursor: Cursor {
                // Consoles with the CURSOR capability
                // must start with the cursor visible.
                enabled: capabilities & Capability::CURSOR > 0,
                blinking: false,
            },
        }
    }

    pub fn supports_rgb(&self) -> bool {
        self.capabilities & Capability::RGB > 0
    }

    pub fn supports_cursor(&self) -> bool {
        self.capabilities & Capability::CURSOR > 0
    }

    pub fn supports_blinking(&self) -> bool {
        self.capabilities & Capability::BLINK > 0
    }

    pub fn fg(&self) -> Color {
        self.fg.get()
    }

    pub fn set_fg(&self, fg: Color) -> Result<()> {
        match fg {
            Color::Rgb(_, _, _) => {
                if !self.supports_rgb() {
                    return Err(Error::Unsupported);
                }
            }
            _ => {}
        }
        self.fg.replace(fg);
        Ok(())
    }

    pub fn bg(&self) -> Color {
        self.bg.get()
    }

    pub fn set_bg(&self, bg: Color) -> Result<()> {
        match bg {
            Color::Rgb(_, _, _) => {
                if !self.supports_rgb() {
                    return Err(Error::Unsupported);
                }
            }
            _ => {}
        }
        self.bg.replace(bg);
        Ok(())
    }
}

pub trait Console {
    fn backspace(&mut self) -> Result<()>;
    /// Set blink state or toggle if None.
    fn blink_cursor(&mut self, blink: Option<bool>) -> Result<()>;
    fn carriage_return(&mut self) -> Result<()>;
    /// Clears the whole viewport using the current
    /// foreground and background color. Does not implicitly modify cursor state.
    fn clear(&mut self) -> Result<()>;
    fn enable_cursor(&mut self, enabled: bool) -> Result<()>;
    /// Carriage return is implied.
    fn newline(&mut self) -> Result<()>;
    /// The arguments x and y must be in bounds of width and height respectively.
    fn move_cursor(&mut self, x: usize, y: usize) -> Result<()>;
    /// Scroll either up or down by rows. If rows >= height,
    /// this operation is functionally equivalent to a clear.
    fn scroll(&mut self, direction: ScrollDirection, rows: usize) -> Result<()>;
    fn state(&self) -> &State;
    /// Character encoding is implementation-defined. In most cases it will either be ASCII or UTF-8.
    fn write(&mut self, s: &[u8]) -> Result<usize>;
    /// Implementation-defined synchronization. Should be called by consumers after some number of batched writes.
    /// Examples:
    /// - Hardware cursor syncing.
    /// - Flushing write-combining memory of a framebuffer.
    /// - A no-op.
    fn sync(&mut self) -> Result<()>;
    /// Default behavior is a no-op.
    fn tab(&mut self) -> Result<()> {
        Ok(())
    }
}

/// This helper expects ASCII or an ASCII-compatible encoding to interpret control codes.
pub fn console_write(console: &mut impl Console, s: &[u8]) -> Result<usize> {
    let mut i: usize = 0;
    // Note: written tracks _all_ bytes, including control codes.
    let mut written: usize = 0;

    loop {
        let mut j: usize = i;
        let len = s.len();

        while j < len {
            let ch = s[j];
            match ch {
                0x8 | b'\t' | b'\n' | b'\r' => break,
                _ => {}
            }
            j += 1;
        }

        while j > i {
            let did_write = console.write(&s[i..j])?;
            if did_write == 0 {
                let _ = console.sync();
                return Err(Error::Progress);
            }
            i += did_write;
            if i > j {
                // Bad implementation.
                let _ = console.sync();
                return Err(Error::Implementation);
            }
            written += did_write;
        }

        if i == len {
            break;
        }

        let ch = s[i];
        match ch {
            0x8 => {
                console.backspace()?;
            }
            b'\t' => {
                console.tab()?;
            }
            b'\n' => {
                console.newline()?;
            }
            b'\r' => {
                console.carriage_return()?;
            }
            _ => unreachable!(),
        }

        i += 1;
        written += 1;
    }

    console.sync()?;
    Ok(written)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod vga;
