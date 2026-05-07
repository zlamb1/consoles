#[derive(Clone, Copy, Debug)]
pub struct BitmapFont<'a> {
    pub width: usize,
    pub height: usize,
    /// Data format is top-left mapping to most significant bit.
    pub data: &'a [u8],
}

pub mod data;
