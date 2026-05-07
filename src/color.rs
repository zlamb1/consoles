#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    Black,
    Red,
    Green,
    Brown,
    Blue,
    Magenta,
    Cyan,
    LightGray,
    DarkGray,
    LightRed,
    LightGreen,
    Yellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Rgb(u8, u8, u8),
    Palette(Palette),
}
