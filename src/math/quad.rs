use crate::math::{Corners, Point, Rect, Size, Vector};
use bytemuck::{Pod, Zeroable};
use euclid::approxord::{max, min};
use euclid::num::Zero;
use std::fmt;
use std::hash::Hash;
use std::ops::Add;

#[repr(C)]
pub struct Quad<T, U> {
    pub p0: Point<T, U>,
    pub p1: Point<T, U>,
    pub p2: Point<T, U>,
    pub p3: Point<T, U>,
}

impl<T: Copy, U> Copy for Quad<T, U> {}

impl<T: Clone, U> Clone for Quad<T, U> {
    fn clone(&self) -> Self {
        Self {
            p0: self.p0.clone(),
            p1: self.p1.clone(),
            p2: self.p2.clone(),
            p3: self.p3.clone(),
        }
    }
}

impl<T, U> Eq for Quad<T, U> where T: Eq {}

impl<T, U> PartialEq for Quad<T, U>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.p0 == other.p0 && self.p1 == other.p1 && self.p2 == other.p2 && self.p3 == other.p3
    }
}

impl<T: fmt::Debug, U> fmt::Debug for Quad<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Quad")
            .field(&self.p0)
            .field(&self.p1)
            .field(&self.p2)
            .field(&self.p3)
            .finish()
    }
}

impl<T: Default, U> Default for Quad<T, U> {
    fn default() -> Self {
        Self::new(
            Point::default(),
            Point::default(),
            Point::default(),
            Point::default(),
        )
    }
}

impl<T, U> Hash for Quad<T, U>
where
    T: Hash,
{
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.p0.hash(state);
        self.p1.hash(state);
        self.p2.hash(state);
        self.p3.hash(state);
    }
}

// SAFETY: PhantomData in Point is zero-sized and repr(C) guarantees consistent layout
unsafe impl<T: Zeroable, U> Zeroable for Quad<T, U> {}
unsafe impl<T: Pod, U: 'static> Pod for Quad<T, U> {}

impl<T, U> Quad<T, U> {
    #[inline]
    pub const fn new(p0: Point<T, U>, p1: Point<T, U>, p2: Point<T, U>, p3: Point<T, U>) -> Self {
        Self { p0, p1, p2, p3 }
    }
}

impl<T, U> Quad<T, U>
where
    T: Copy,
{
    /// Creates a `Quad` of the given size, at offset zero.
    #[inline]
    pub fn from_size(size: Size<T, U>) -> Self
    where
        T: Zero,
    {
        Self {
            p0: Point::zero(),
            p1: Point::new(size.width, Zero::zero()),
            p2: Point::new(size.width, size.height),
            p3: Point::new(Zero::zero(), size.height),
        }
    }
}

impl<T, U> Quad<T, U>
where
    T: Zero,
{
    /// Constructor, setting all points to zero.
    pub fn zero() -> Self {
        Self::new(Point::zero(), Point::zero(), Point::zero(), Point::zero())
    }
}

impl<T, U> Quad<T, U>
where
    T: Copy + Add<T, Output = T>,
{
    /// Returns the same quad, translated by a vector.
    #[inline]
    pub fn translate(&self, by: Vector<T, U>) -> Self {
        Self::new(self.p0 + by, self.p1 + by, self.p2 + by, self.p3 + by)
    }
}

impl<T, U> From<Rect<T, U>> for Quad<T, U>
where
    T: Copy,
{
    fn from(value: Rect<T, U>) -> Self {
        Self::new(
            value.top_left(),
            value.top_right(),
            value.bottom_right(),
            value.bottom_left(),
        )
    }
}

impl<T, U> Quad<T, U>
where
    T: Copy + PartialOrd,
{
    /// Returns the axis-aligned bounding box containing this quad.
    #[inline]
    pub fn aabb(&self) -> Rect<T, U> {
        let min_x = min(min(min(self.p0.x, self.p1.x), self.p2.x), self.p3.x);
        let max_x = max(max(max(self.p0.x, self.p1.x), self.p2.x), self.p3.x);
        let min_y = min(min(min(self.p0.y, self.p1.y), self.p2.y), self.p3.y);
        let max_y = max(max(max(self.p0.y, self.p1.y), self.p2.y), self.p3.y);

        Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    /// Tests intersection between this quad's AABB and a rectangle.
    #[inline]
    pub fn aabb_intersects_rect(&self, rect: &Rect<T, U>) -> bool {
        self.aabb().intersects(rect)
    }
}
