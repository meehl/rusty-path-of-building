use crate::math::{Point, Size, Vector};
use bytemuck::{Pod, Zeroable};
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
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
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
        Quad {
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
        Quad::new(Point::zero(), Point::zero(), Point::zero(), Point::zero())
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
