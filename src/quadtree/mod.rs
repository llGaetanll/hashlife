use core::fmt::Debug;

pub use crate::quadtree::point::Point;
use crate::quadtree::aabb::Aabb;
use crate::quadtree::node::Node;
use crate::quadtree::node::NodeID;
use crate::quadtree::util::partition_in_place;

mod point;
mod aabb;
mod node;
mod util;

#[derive(Debug)]
pub struct QuadTree {
    /// The index of the root of the tree in `data`
    pub root: NodeID,

    /// Axis-Aligned Bounding Box of the QuadTree
    pub bbox: Aabb,

    nodes: Vec<Node>,
    data: Vec<Point>,
    indices: Vec<usize>
}

impl QuadTree {
    pub fn from_points(mut points: Vec<Point>) -> Self {
        let bbox = Aabb::from_points(&points);
        let mut tree = QuadTree {
            root: 0,
            bbox: bbox.clone(),
            data: vec![],
            nodes: vec![],
            indices: vec![]
        };

        let (lo, hi) = (0, points.len());
        tree.root = Self::build(&mut tree, &bbox, &mut points, lo, hi);

        tree.data = points;
        tree.indices.push(tree.data.len());

        tree
    }

    fn build(tree: &mut QuadTree, bbox: &Aabb, points: &mut [Point], lo: usize, hi: usize) -> NodeID {
        // this is a null node (not even a leaf node)
        if lo >= hi {
            return usize::MAX;
        }

        // by this point, we know we're adding a node. Either it's a leaf, or not
        // we first assume it's a leaf
        let result: NodeID = tree.nodes.len();
        tree.nodes.push(Node::empty());

        // leaf node
        if lo + 1 == hi {
            return result;
        }

        tree.indices.push(lo);

        let center = bbox.center();

        let points = &mut points[lo..hi];
        let (lo, hi) = (0, points.len());

        println!("center: {:?}", center);
        println!("{:?}", points);

        let y = partition_in_place(points, |p| p.y < center.y);
        let x_lo = partition_in_place(&mut points[..y], |p| p.x < center.x);
        let x_hi = partition_in_place(&mut points[y..], |p| p.x < center.x) + y;

        println!("{:?}", points);
        println!("lo: {}, x_lo: {}, y: {}, x_hi: {}, hi: {}", lo, x_lo, y, x_hi, hi);
        println!("---");

        let [nw, ne, sw, se] = bbox.split();

        tree.nodes[result].nw = Self::build(tree, &nw, points, lo, x_lo);
        tree.nodes[result].ne = Self::build(tree, &ne, points, x_lo, y);
        tree.nodes[result].sw = Self::build(tree, &sw, points, y, x_hi);
        tree.nodes[result].se = Self::build(tree, &se, points, x_hi, hi);

        result
    }
}

mod test {
    use crate::quadtree::QuadTree;
    use crate::quadtree::Point;

    fn parse_points(points: &[(i32, i32)]) -> Vec<Point> {
        points.iter().map(|&(x, y)| Point {x: x as f32, y: y as f32}).collect()
    }

    #[test]
    fn four_corner_points() {
        let points = parse_points(&[(4, 4), (4, 1), (1, 1), (1, 4)]);

        let qt = QuadTree::from_points(points);

        println!("{:#?}", qt);
    }

    #[test]
    fn single_point() {
        let points = parse_points(&[(4, 4)]);

        let qt = QuadTree::from_points(points);

        println!("{:#?}", qt);
    }

    #[test]
    fn many_points() {
        let points = parse_points(&[(1, 3), (2, 4), (3, 3), (1, 6), (4, 2), (6, 5), (7, 4), (9, 9)]);

        let qt = QuadTree::from_points(points);

        println!("{:#?}", qt);

        panic!()
    }
}
