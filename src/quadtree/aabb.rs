use core::fmt::Debug;

use crate::quadtree::point::Point;

#[derive(Debug, Clone)]
pub struct Aabb {
    min: Point,
    max: Point,
}

impl Aabb {
    pub fn new() -> Self {
        Aabb {
            min: Point {
                x: f32::INFINITY,
                y: f32::INFINITY,
            },
            max: Point {
                x: f32::NEG_INFINITY,
                y: f32::NEG_INFINITY,
            },
        }
    }

    /// Create an AABB from a list of `Point`s.
    pub fn from_points(points: &[Point]) -> Self {
        let mut b = Aabb::new();

        for p in points {
            b.add(p);
        }

        b
    }

    /// Add a `Point` `p` to the current Axis-Aligned Bounding Box.
    pub fn add(&mut self, p: &Point) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
    }

    pub fn center(&self) -> Point {
        self.min.mid(&self.max)
    }

    /// Splits the bounding box into four equal quadrants
    pub fn split(&self) -> [Aabb; 4] {
        let center = self.center();

        let nw = Aabb {
            min: self.min,
            max: center,
        };

        let ne = Aabb {
            min: Point {
                x: center.x,
                y: self.min.y,
            },
            max: Point {
                x: self.max.x,
                y: center.y,
            },
        };

        let sw = Aabb {
            min: Point {
                x: self.min.x,
                y: center.y,
            },
            max: Point {
                x: center.x,
                y: self.max.y,
            },
        };

        let se = Aabb {
            min: center,
            max: self.max,
        };

        [nw, ne, sw, se]
    }
}
