pub use euclid::Box2D as Rect;
pub use euclid::Point2D as Point;
pub use euclid::Size2D as Size;
pub use euclid::Vector2D as Vector;
pub use quad::Quad;

mod quad;

pub trait Corners<T, U> {
    fn top_left(&self) -> Point<T, U>;
    fn top_right(&self) -> Point<T, U>;
    fn bottom_left(&self) -> Point<T, U>;
    fn bottom_right(&self) -> Point<T, U>;
}

impl<T, U> Corners<T, U> for Rect<T, U>
where
    T: Copy,
{
    #[inline]
    fn top_left(&self) -> Point<T, U> {
        self.min
    }

    #[inline]
    fn top_right(&self) -> Point<T, U> {
        Point::new(self.max.x, self.min.y)
    }

    #[inline]
    fn bottom_left(&self) -> Point<T, U> {
        Point::new(self.min.x, self.max.y)
    }

    #[inline]
    fn bottom_right(&self) -> Point<T, U> {
        self.max
    }
}
