use std::fmt::Debug;

pub type NodeID = usize;

pub struct Node {
    pub nw: NodeID,
    pub ne: NodeID,
    pub sw: NodeID,
    pub se: NodeID,
}

impl Node {
    pub fn empty() -> Self {
        Node {
            // here, we use usize::MAX to signify that we're not pointing at anything
            // remember: these are indices to other nodes in the vector of nodes.
            nw: usize::MAX,
            ne: usize::MAX,
            sw: usize::MAX,
            se: usize::MAX,
        }
    }
}

impl Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let g = |i: NodeID| -> isize {if i == usize::MAX { -1 } else { i as isize }}; 

        write!(f, "[nw: {}, ne: {}, sw: {}, se: {}]", g(self.nw), g(self.ne), g(self.sw), g(self.se))
    }
}
