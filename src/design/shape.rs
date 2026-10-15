use gpui_kit::{Pixels, rems};
#[derive(Clone, Copy)]
pub enum Shape {
    Small,
    Large,
}
impl Shape {
    pub fn pixels(self, font_size: Pixels) -> Pixels {
        rems(match self {
            Self::Small => 0.125,
            Self::Large => 0.25,
        })
        .to_pixels(font_size)
    }
}
