use core::fmt::Debug;

#[derive(Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// Compute the midpoint of the current `Point` and some `other` `Point`.
    pub fn mid(&self, other: &Self) -> Self {
        Point {
            x: (self.x + other.x) / 2f32,
            y: (self.y + other.y) / 2f32,
        }
    }
}

impl Debug for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
