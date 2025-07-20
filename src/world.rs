use crate::rules::RuleSet;
use crate::rules::RuleSetError;

use crate::cell::Cell;
use crate::WorldOffset;

pub struct World {
    /// Life rules
    ///
    /// Indexing into this array with rule `r` yields the result of `r`.
    rules: Vec<u16>,

    /// Index of the root [`Cell`] in `buf`
    pub root: usize,

    /// This is where all of our memory goes
    pub buf: Vec<Cell>,

    /// World depth, where `0` is a leaf [`Cell`], (8x8 world size).
    ///
    /// In general, `n` yields a world sidelength of `2^(n + 3)`
    pub depth: u8,
}

impl World {
    /// Create an empty new world
    pub fn new(rule_set: &str) -> Result<Self, RuleSetError> {
        let rule_set: RuleSet = rule_set.parse()?;
        let rules = rule_set.compute_rules();

        // First cell is the canonical void cell, second is the root, an uninitialized leaf
        let buf = vec![Cell::void(), Cell::leaf_uninit()];

        let root = 1;

        Ok(Self {
            rules,
            root,
            buf,
            depth: 3,
        })
    }

    pub fn next(&mut self) {
        // The root is always last
        let mut root = self.buf.pop().unwrap();

        self.root = root.next(&self.rules, &mut self.buf);
        self.depth -= 1;

        self.grow(1);
    }

    /// Grows the world by a factor of 2^k, keeping the previous root at the origin
    pub fn grow(&mut self, k: usize) {
        if k == 0 {
            return;
        }

        // The root is always the last
        let root = self.buf.pop().unwrap();
        let root = root.grow(&mut self.buf);

        let n = self.buf.len();

        self.buf.push(root);

        self.root = n;
        self.depth += 1;

        self.grow(k - 1);
    }

    pub fn set(&mut self, x: WorldOffset, y: WorldOffset) {
        let root = self.root;

        self.set_bit(root, x, y, self.depth);
    }

    fn set_bit(&mut self, ptr: usize, x: WorldOffset, y: WorldOffset, depth: u8) {
        assert!(depth >= 3);

        if depth == 3 {
            // Leaf
            let cell = &mut self.buf[ptr];

            let child = Self::get_child_idx_mut(cell, x, y);
            *child |= 1 << (3 - (x & 3) + 4 * (y & 3));
        } else {
            // Non-leaf
            let cell = self.buf[ptr];
            let child_idx = Self::get_child_idx(cell, x, y);

            // We're pointing at nothing
            if child_idx == 0 {
                // Initialize the nodes
                let new_child_ptr = if depth == 3 {
                    self.add_leaf()
                } else {
                    self.add_node()
                };

                let cell = &mut self.buf[ptr];
                let child_idx = Self::get_child_idx_mut(cell, x, y);

                *child_idx = new_child_ptr;
            }

            let w = 1 << depth;
            let x = (x & (w - 1)) - (w >> 1);
            let y = (y & (w - 1)) - (w >> 1);

            self.set_bit(child_idx, x, y, depth - 1)
        }
    }

    #[allow(clippy::collapsible_else_if)]
    fn get_child_idx(cell: Cell, x: i128, y: i128) -> usize {
        if x < 0 {
            if y < 0 {
                cell.sw
            } else {
                cell.nw
            }
        } else {
            if y < 0 {
                cell.se
            } else {
                cell.ne
            }
        }
    }

    #[allow(clippy::collapsible_else_if)]
    fn get_child_idx_mut(cell: &mut Cell, x: i128, y: i128) -> &mut usize {
        if x < 0 {
            if y < 0 {
                &mut cell.sw
            } else {
                &mut cell.nw
            }
        } else {
            if y < 0 {
                &mut cell.se
            } else {
                &mut cell.ne
            }
        }
    }

    /// Add a leaf cell to the world and return its index
    fn add_leaf(&mut self) -> usize {
        let n = self.buf.len();

        self.buf.push(Cell::leaf_uninit());

        n
    }

    /// Add a non-leaf cell to the world and return its index
    fn add_node(&mut self) -> usize {
        let n = self.buf.len();

        self.buf.push(Cell::uninit());

        n
    }
}
