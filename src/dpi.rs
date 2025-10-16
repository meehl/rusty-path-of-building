use crate::math::{Point, Quad, Rect, Size, Vector};
use core::f32;
use num_traits::{Float, NumCast, ToPrimitive};

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

pub trait ConvertToLogical {
    type Output<V>;
    fn to_logical<V: NumCast, F: Float>(&self, scale_factor: F) -> Self::Output<V>;
}

pub trait ConvertToPhysical {
    type Output<V>;
    fn to_physical<V: NumCast, F: Float>(&self, scale_factor: F) -> Self::Output<V>;
}

#[inline]
pub fn validate_scale_factor<F: Float>(scale_factor: F) -> bool {
    scale_factor.is_sign_positive() && scale_factor.is_normal()
}

impl<T> ConvertToLogical for PhysicalPoint<T>
where
    T: Copy + std::ops::Div<Output = T> + ToPrimitive,
{
    type Output<V> = LogicalPoint<V>;

    #[inline]
    fn to_logical<V: NumCast, F: Float>(&self, scale_factor: F) -> Self::Output<V> {
        assert!(validate_scale_factor(scale_factor));
        let x = F::from(self.x).unwrap() / scale_factor;
        let y = F::from(self.y).unwrap() / scale_factor;
        LogicalPoint::new(x, y).cast()
    }
}

impl<T> ConvertToLogical for PhysicalSize<T>
where
    T: Copy + std::ops::Div<Output = T> + ToPrimitive,
{
    type Output<V> = LogicalSize<V>;

    #[inline]
    fn to_logical<V: NumCast, F: Float>(&self, scale_factor: F) -> Self::Output<V> {
        assert!(validate_scale_factor(scale_factor));
        let width = F::from(self.width).unwrap() / scale_factor;
        let height = F::from(self.height).unwrap() / scale_factor;
        LogicalSize::new(width, height).cast()
    }
}

impl<T> ConvertToLogical for PhysicalRect<T>
where
    T: Copy + std::ops::Div<Output = T> + ToPrimitive,
{
    type Output<V> = LogicalRect<V>;

    #[inline]
    fn to_logical<V: NumCast, F: Float>(&self, scale_factor: F) -> Self::Output<V> {
        assert!(validate_scale_factor(scale_factor));
        let min = self.min.to_logical(scale_factor);
        let max = self.max.to_logical(scale_factor);
        LogicalRect::new(min, max)
    }
}

impl<T> ConvertToPhysical for LogicalPoint<T>
where
    T: Copy + std::ops::Div<Output = T> + ToPrimitive,
{
    type Output<V> = PhysicalPoint<V>;

    #[inline]
    fn to_physical<V: NumCast, F: Float>(&self, scale_factor: F) -> Self::Output<V> {
        assert!(validate_scale_factor(scale_factor));
        let x = F::from(self.x).unwrap() * scale_factor;
        let y = F::from(self.y).unwrap() * scale_factor;
        PhysicalPoint::new(x, y).cast()
    }
}

impl<T> ConvertToPhysical for LogicalRect<T>
where
    T: Copy + std::ops::Div<Output = T> + ToPrimitive,
{
    type Output<V> = PhysicalRect<V>;

    #[inline]
    fn to_physical<V: NumCast, F: Float>(&self, scale_factor: F) -> Self::Output<V> {
        assert!(validate_scale_factor(scale_factor));
        let min = self.min.to_physical(scale_factor);
        let max = self.max.to_physical(scale_factor);
        PhysicalRect::new(min, max)
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
