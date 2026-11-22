#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Xrgb8888,
    Argb8888,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameData<'a> {
    Dumb {
        width: u32,
        height: u32,
        pitch: u32,
        format: PixelFormat,
        pixels: &'a [u8],
    },
    Gpu {
        width: u32,
        height: u32,
        format: PixelFormat,
    },
}

pub struct Frame<'a> {
    pub data: FrameData<'a>,
    pub damage: &'a [Rect],
}
