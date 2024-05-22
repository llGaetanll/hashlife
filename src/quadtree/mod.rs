#[derive(Debug)]
pub struct QuadTree {
    pub level: u32,
    pub root: Box<Node>,
}

#[derive(Debug, Clone)]
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
    pub fn new() -> Box<Self> {
        Box::new(Node {
            on: false,
            nw: None,
            ne: None,
            sw: None,
            se: None,
        })
    }

    pub fn from(
        nw: &Option<Box<Node>>,
        ne: &Option<Box<Node>>,
        sw: &Option<Box<Node>>,
        se: &Option<Box<Node>>,
    ) -> Box<Node> {
        Box::new(Node {
            on: false,
            nw: nw.clone(),
            ne: ne.clone(),
            sw: sw.clone(),
            se: se.clone(),
        })
    }

    pub fn is_empty(&self) -> bool {
        !self.on && self.nw.is_none() && self.ne.is_none() && self.sw.is_none() && self.se.is_none()
    }

    pub fn is_leaf(&self) -> bool {
        self.on && self.nw.is_none() && self.ne.is_none() && self.sw.is_none() && self.se.is_none()
    }

    pub fn center(&self) -> Box<Node> {
        let mut node = Node::new();

        node.nw = self.nw.as_ref().and_then(|nw| nw.se.clone());
        node.ne = self.ne.as_ref().and_then(|ne| ne.sw.clone());
        node.sw = self.sw.as_ref().and_then(|sw| sw.ne.clone());
        node.se = self.se.as_ref().and_then(|se| se.nw.clone());

        node
    }

    pub fn north(&self) -> Box<Node> {
        let mut node = Node::new();

        node.nw = self.nw.as_ref().and_then(|nw| nw.ne.clone());
        node.ne = self.ne.as_ref().and_then(|ne| ne.nw.clone());
        node.sw = self.nw.as_ref().and_then(|nw| nw.se.clone());
        node.se = self.ne.as_ref().and_then(|ne| ne.sw.clone());

        node
    }

    pub fn south(&self) -> Box<Node> {
        let mut node = Node::new();

        node.nw = self.sw.as_ref().and_then(|sw| sw.ne.clone());
        node.ne = self.se.as_ref().and_then(|se| se.nw.clone());
        node.sw = self.sw.as_ref().and_then(|sw| sw.se.clone());
        node.se = self.se.as_ref().and_then(|se| se.sw.clone());

        node
    }

    pub fn east(&self) -> Box<Node> {
        let mut node = Node::new();

        node.nw = self.ne.as_ref().and_then(|ne| ne.sw.clone());
        node.ne = self.ne.as_ref().and_then(|ne| ne.se.clone());
        node.sw = self.se.as_ref().and_then(|se| se.nw.clone());
        node.se = self.se.as_ref().and_then(|se| se.ne.clone());

        node
    }

    pub fn west(&self) -> Box<Node> {
        let mut node = Node::new();

        node.nw = self.nw.as_ref().and_then(|nw| nw.sw.clone());
        node.ne = self.nw.as_ref().and_then(|nw| nw.se.clone());
        node.sw = self.sw.as_ref().and_then(|sw| sw.nw.clone());
        node.se = self.sw.as_ref().and_then(|sw| sw.ne.clone());

        node
    }

    // computes the result of a macrocell
    pub fn next_gen(&self, depth: u32) -> Box<Node> {
        if depth == 0 {
            // this is where the hashing goes
            todo!()
        } else {
            let n00 = self.nw.as_ref().map(|nw| nw.center());
            let n01 = Some(self.north().center());
            let n02 = self.ne.as_ref().map(|ne| ne.center());
            let n10 = Some(self.west().center());
            let n11 = Some(self.center().center());
            let n12 = Some(self.east().center());
            let n20 = self.sw.as_ref().map(|sw| sw.center());
            let n21 = Some(self.south().center());
            let n22 = self.se.as_ref().map(|se| se.center());

            Node::from(
                &Some(Node::from(&n00, &n01, &n10, &n11)),
                &Some(Node::from(&n01, &n02, &n11, &n12)),
                &Some(Node::from(&n10, &n11, &n20, &n21)),
                &Some(Node::from(&n11, &n12, &n21, &n22))
            )
        }
    }
}

impl QuadTree {
    /// Create a new `QuadTree` with sidelength `2^k` with `k >= 0`.
    pub fn new(k: u32) -> Self {
        QuadTree {
            // we say that single nodes (`QuadTree`s of sidelength 1) are of level `0`, and that
            // 2x2 `QuadTree`s are level `1`, and so on...
            level: k,
            root: Node::new(),
        }
    }

    /// Grows the tree by a factor of `2` while maintaining the centering
    pub fn grow(self) -> QuadTree {
        let mut nw = Node::new();
        nw.se = self.root.nw;

        let mut ne = Node::new();
        ne.sw = self.root.ne;

        let mut sw = Node::new();
        sw.ne = self.root.sw;

        let mut se = Node::new();
        se.nw = self.root.se;

        let root = Box::new(Node {
            on: false,
            nw: Some(nw),
            ne: Some(ne),
            sw: Some(sw),
            se: Some(se),
        });

        QuadTree {
            level: self.level + 1,
            root,
        }
    }

    /// Send a bit in our `QuadTree`. Cannonically, the bottom left corner of the tree is the
    /// origin.
    pub fn set(&mut self, x: i32, y: i32) {
        let (mut x, mut y) = (x, y);
        let mut depth = self.level;
        let mut node = &mut self.root;

        while depth > 0 {
            // sidelength of the node
            let s = 1 << depth;

            if x < 0 {
                if y < 0 {
                    if node.sw.is_none() {
                        node.sw = Some(Node::new());
                    }

                    node = node.sw.as_mut().unwrap();
                } else {
                    if node.nw.is_none() {
                        node.nw = Some(Node::new());
                    }

                    node = node.nw.as_mut().unwrap();
                }
            } else if y < 0 {
                if node.se.is_none() {
                    node.se = Some(Node::new());
                }

                node = node.se.as_mut().unwrap();
            } else {
                if node.ne.is_none() {
                    node.ne = Some(Node::new());
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
