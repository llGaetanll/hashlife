use crate::quadtree::{Point, QuadTree};

mod quadtree;
mod qt;

fn main() {
    let points = vec![
        Point {x: 4f32, y: 4f32},
        Point {x: 4f32, y: 1f32},
        Point {x: 1f32, y: 1f32},
        Point {x: 1f32, y: 4f32},
    ];

    let qt = QuadTree::from_points(points);

    println!("{:#?}", qt);
}
