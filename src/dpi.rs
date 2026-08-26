use crate::math::{Point, Quad, Rect, Scale, Size, Vector};

pub struct LogicalScreenSpace;
pub struct PhysicalScreenSpace;

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
