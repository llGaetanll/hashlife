pub struct QuadTree {
    level: usize,
    root: Box<Node>,
}

pub struct Node {
    /// Whether the cell is alive or dead.
    /// If the cell is alive, it *must* be a leaf, but if is it dead, it doesn't have to be.
    on: bool,

    // `None` if leaf
    pub nw: Option<Box<Node>>,
    pub ne: Option<Box<Node>>,
    pub sw: Option<Box<Node>>,
    pub se: Option<Box<Node>>,
}

impl Node {
    pub fn alive() -> Box<Self> {
        Box::new(Node {
            on: true,
            nw: None,
            ne: None,
            sw: None,
            se: None,
        })
    }

    pub fn dead() -> Box<Self> {
        Box::new(Node {
            on: false,
            nw: None,
            ne: None,
            sw: None,
            se: None,
        })
    }
}

impl QuadTree {
    /// Create a new `QuadTree` with sidelength `2^k` with `k >= 0`.
    pub fn new(k: usize) -> Self {
        QuadTree {
            // we say that single nodes (`QuadTree`s of sidelength 1) are of level `0`, and that
            // 2x2 `QuadTree`s are level `1`, and so on...
            level: k,
            root: Node::dead(),
        }
    }

    /// Send a bit in our `QuadTree`. Cannonically, the bottom left corner of the tree is the
    /// origin.
    pub fn set(&mut self, x: isize, y: isize) {
        let (mut x, mut y) = (x, y);
        let mut depth = self.level;
        let mut node = &mut self.root;

        while depth > 0 {
            let s = 1 << depth;

            if x < 0 {
                if y < 0 {
                    if node.sw.is_none() {
                        node.sw = Some(Node::dead());
                    }

                    node = node.sw.as_mut().unwrap();
                } else {
                    if node.nw.is_none() {
                        node.nw = Some(Node::dead());
                    }

                    node = node.nw.as_mut().unwrap();
                }
            } else if y < 0 {
                if node.se.is_none() {
                    node.se = Some(Node::dead());
                }

                node = node.se.as_mut().unwrap();
            } else {
                if node.ne.is_none() {
                    node.ne = Some(Node::dead());
                }

                node = node.ne.as_mut().unwrap();
            }

            depth -= 1;
            x = (x & (s - 1)) - (s >> 1);
            y = (y & (s - 1)) - (s >> 1);
        }

        node.on = true;
    }
}
