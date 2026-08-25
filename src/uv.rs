use crate::math::{Point, Quad, Rect, Size};

pub struct UvSpace;

pub type UvPoint = Point<f32, UvSpace>;
pub type UvRect = Rect<f32, UvSpace>;
pub type UvQuad = Quad<f32, UvSpace>;

pub trait UvConstructors {
    fn full_uv() -> Self;
    fn white_uv() -> Self;
}

impl UvConstructors for UvRect {
    #[inline]
    fn full_uv() -> Self {
        Self::from_size(Size::new(1.0, 1.0))
    }

    #[inline]
    fn white_uv() -> Self {
        Self::zero()
    }
}

impl UvConstructors for UvQuad {
    #[inline]
    fn full_uv() -> Self {
        Self::from_size(Size::new(1.0, 1.0))
    }

    #[inline]
    fn white_uv() -> Self {
        Self::zero()
    }
}
