#[derive(Clone, Copy, Debug)]
pub struct Mask {
    /// Bit count of this component.
    pub size: u16,
    pub shift: u16,
}

impl Mask {
    pub fn new(size: u16, shift: u16) -> Self {
        Self { size, shift }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Framebuffer {
    pub ptr: *mut u8,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    /// Bits per pixel.
    pub bpp: usize,
    pub red_mask: Mask,
    pub green_mask: Mask,
    pub blue_mask: Mask,
}
