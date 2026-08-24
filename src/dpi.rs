use crate::math::{Point, Quad, Rect, Size, Vector};
use core::f32;
use euclid::Scale;
use num_traits::{Float, ToPrimitive};

pub struct LogicalScreenSpace;
pub struct PhysicalScreenSpace;
pub struct Normalized;

pub type LogicalPoint<T> = Point<T, LogicalScreenSpace>;
pub type LogicalVector<T> = Vector<T, LogicalScreenSpace>;
pub type LogicalSize<T> = Size<T, LogicalScreenSpace>;
pub type LogicalRect<T> = Rect<T, LogicalScreenSpace>;
pub type LogicalQuad<T> = Quad<T, LogicalScreenSpace>;

pub type PhysicalPoint<T> = Point<T, PhysicalScreenSpace>;
pub type PhysicalVector<T> = Vector<T, PhysicalScreenSpace>;
pub type PhysicalSize<T> = Size<T, PhysicalScreenSpace>;
pub type PhysicalRect<T> = Rect<T, PhysicalScreenSpace>;

pub type ScaleFactor<F> = Scale<F, LogicalScreenSpace, PhysicalScreenSpace>;

pub type NormalizedPoint = Point<f32, Normalized>;
pub type NormalizedRect = Rect<f32, Normalized>;
pub type NormalizedQuad = Quad<f32, Normalized>;

pub trait Normalize<T, U> {
    type Output<F>;
    fn normalize<F: Float>(&self, size: Size<T, U>) -> Self::Output<F>;
}

impl<T, U> Normalize<T, U> for Point<T, U>
where
    T: Copy + std::ops::Div<Output = T> + ToPrimitive,
{
    type Output<V> = Point<V, Normalized>;

    #[inline]
    fn normalize<F: Float>(&self, size: Size<T, U>) -> Self::Output<F> {
        Point::new(
            F::from(self.x).unwrap() / F::from(size.width).unwrap(),
            F::from(self.y).unwrap() / F::from(size.height).unwrap(),
        )
    }
}

impl<T, U> Normalize<T, U> for Rect<T, U>
where
    T: Copy + std::ops::Div<Output = T> + ToPrimitive,
{
    type Output<V> = Rect<V, Normalized>;

    #[inline]
    fn normalize<F: Float>(&self, size: Size<T, U>) -> Self::Output<F> {
        Rect::new(self.min.normalize(size), self.max.normalize(size))
    }
}

pub trait Uv {
    fn default_uv() -> Self;
    fn white_uv() -> Self;
}

impl Uv for NormalizedPoint {
    #[inline]
    fn default_uv() -> Self {
        Self::zero()
    }

    #[inline]
    fn white_uv() -> Self {
        Self::zero()
    }
}

impl Uv for NormalizedRect {
    #[inline]
    fn default_uv() -> Self {
        Self::from_size(Size::new(1.0, 1.0))
    }

    #[inline]
    fn white_uv() -> Self {
        Self::zero()
    }
}

impl Uv for NormalizedQuad {
    #[inline]
    fn default_uv() -> Self {
        Self::from_size(Size::new(1.0, 1.0))
    }

    #[inline]
    fn white_uv() -> Self {
        Self::zero()
    }
}
