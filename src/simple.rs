use crate::cell_grid::CellGrid;
use crate::color::Color;

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

pub trait Console {
    /// Clears any remnant text.
    fn clear(&mut self) -> Result<()>;
    /// Implementation-defined synchronization. Should be called by consumers after some number of batched writes.
    /// Examples:
    /// - Hardware cursor syncing.
    /// - Flushing write-combining memory of a framebuffer.
    /// - A no-op.
    fn flush(&mut self) -> Result<()>;
    /// Character encoding is implementation-defined. In most cases it will either be ASCII or UTF-8.
    fn write(&mut self, s: &[u8]) -> Result<usize>;
}

/// A console defined to use a visual representation
/// that is a grid. It has a width, height, x, and y.
pub trait CellConsole: Console {
    fn cell_grid(&self) -> &CellGrid;
    /// Move the cursor to a new x and y.
    fn position(&mut self, x: usize, y: usize) -> Result<()>;
    /// Scrolls the console down one row.
    fn scroll(&mut self) -> Result<()>;
}

/// A console that supports enabling or disabling a cursor.
pub trait CursorConsole: CellConsole {
    /// Enable or disable the cursor.
    fn enable(&mut self, enable: bool) -> Result<()>;
}

/// A cursor console that supports manual blinking of the cursor.
pub trait BlinkConsole: CursorConsole {
    /// Toggle blink state of the cursor. This operation is not failable.
    fn blink(&mut self);
    /// Whether the cursor is currently visible or not.
    fn visible(&self) -> bool;
}

/// A console that supports a visual foreground and background
/// for text.
pub trait ColorConsole: Console {
    fn fg(&self) -> Color;
    fn bg(&self) -> Color;
    fn set_fg(&mut self, fg: Color) -> Result<()>;
    fn set_bg(&mut self, bg: Color) -> Result<()>;
}

pub mod fb;

#[cfg(test)]
pub mod fb_test;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod vga;
