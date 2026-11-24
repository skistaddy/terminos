#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Xrgb8888,
    Argb8888,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Damage<'a> {
    None,
    Full,
    Rects(&'a [Rect]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame<'a> {
    Dumb {
        w: u32,
        h: u32,
        pitch: u32,
        format: Format,
        pixels: &'a [u8],
        damage: Damage<'a>,
    },
    Gpu {
        w: u32,
        h: u32,
        format: Format,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layer<'a> {
    pub id: u64,
    pub dst: Rect, // viewport
    pub scroll: (i32, i32),
    pub z: i32,
    pub visible: bool,
    pub frame: Frame<'a>,
}
