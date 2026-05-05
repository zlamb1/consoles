use crate::color::Color;

pub enum Error {
    Unsupported,
    Invalid,
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

pub enum ScrollDirection {
    Up,
    Down,
}

pub struct Cursor {
    /// Cursor is enabled or disabled.
    enabled: bool,
    /// Cursor is blinking or not. Only meaningful if the console
    /// has the BLINK capability.
    blinking: bool,
}

impl Cursor {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn blinking(&self) -> bool {
        self.blinking
    }
}

pub struct State {
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    capabilities: usize,
    fg: Color,
    bg: Color,
    cursor: Cursor,
}

impl State {
    pub fn new(width: usize, height: usize, capabilities: usize) -> Self {
        Self {
            width,
            height,
            x: 0,
            y: 0,
            capabilities,
            fg: Color::White,
            bg: Color::Black,
            cursor: Cursor {
                enabled: capabilities & Capability::CURSOR > 0,
                blinking: false,
            },
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn x(&self) -> usize {
        self.x
    }

    pub fn set_x(&mut self, x: usize) -> Result<()> {
        if x < self.width {
            self.x = x;
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }

    pub fn y(&self) -> usize {
        self.y
    }

    pub fn set_y(&mut self, y: usize) -> Result<()> {
        if y < self.height {
            self.y = y;
            Ok(())
        } else {
            Err(Error::Invalid)
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
        self.fg
    }

    pub fn set_fg(&mut self, fg: Color) -> Result<()> {
        match fg {
            Color::Rgb(_, _, _) => {
                if self.supports_rgb() {
                    self.fg = fg;
                    Ok(())
                } else {
                    Err(Error::Unsupported)
                }
            }
            _ => {
                self.fg = fg;
                Ok(())
            }
        }
    }

    pub fn bg(&self) -> Color {
        self.bg
    }

    pub fn set_bg(&mut self, bg: Color) -> Result<()> {
        match bg {
            Color::Rgb(_, _, _) => {
                if self.supports_rgb() {
                    self.bg = bg;
                    Ok(())
                } else {
                    Err(Error::Unsupported)
                }
            }
            _ => {
                self.bg = bg;
                Ok(())
            }
        }
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }
}

pub trait Console {
    fn blink_cursor(&mut self, blink: Option<bool>);
    fn clear(&mut self) -> Result<()>;
    fn enable_cursor(&mut self, enabled: bool);
    fn scroll(&mut self, direction: ScrollDirection, rows: usize) -> Result<()>;
    fn state(&self) -> &State;
    fn state_mut(&mut self) -> &mut State;
    fn write(&mut self, s: &[u8]) -> Result<usize>;
}
