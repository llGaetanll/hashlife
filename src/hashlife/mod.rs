use std::collections::HashMap;

use crate::quadtree::Node;
use crate::quadtree::QuadTree;

pub struct HashLife {
    tree: QuadTree,
    hash: HashMap<Node, Node>
}

/// Create a new instance of `HashLife` with a universe of `2^k` cells on a side
pub fn new(k: u32) {

}
