use crate::rule_set::RuleSet;
use crate::rule_set::RuleSetError;

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

    /// World depth, where `3` is a leaf [`Cell`], (8x8 world size).
    ///
    /// In general, `n` yields a world sidelength of `2^n`
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

        let root = self.buf[self.root];
        let root = root.grow(&mut self.buf);
        self.buf[self.root] = root;

        self.depth += 1;

        self.grow(k - 1);
    }

    pub fn set(&mut self, x: WorldOffset, y: WorldOffset) {
        let root = self.root;

        let w = 1 << (self.depth - 1);

        assert!(
            -w <= x && x < w,
            "x coordinate out of bounds: the range is {}..{} but the coordinate is {}",
            -w,
            w,
            x
        );

        assert!(
            -w <= y && y < w,
            "y coordinate out of bounds: the range is {}..{} but the coordinate is {}",
            -w,
            w,
            y
        );

        self.set_bit(root, x, y, self.depth);
    }

    fn set_bit(&mut self, ptr: usize, x: WorldOffset, y: WorldOffset, depth: u8) {
        assert!(depth >= 3);

        if depth == 3 {
            // Leaf
            let cell = &mut self.buf[ptr];

            let quad = Self::get_quadrant_mut(cell, x, y);
            *quad |= 1 << (3 - (x & 3) + 4 * (y & 3));
        } else {
            // Non-leaf
            let cell = self.buf[ptr];
            let quad = Self::get_quadrant(cell, x, y);

            let w = 1 << depth;
            let f = |c| c - if c < 0 { -(w >> 2) } else { w >> 2 };

            // We're pointing at nothing
            if quad == 0 {
                // Depth 4 means our child should be a leaf
                let new_child_ptr = if depth == 4 {
                    self.add_leaf()
                } else {
                    self.add_node()
                };

                let cell = &mut self.buf[ptr];

                let quad = Self::get_quadrant_mut(cell, x, y);
                *quad = new_child_ptr;

                self.set_bit(new_child_ptr, f(x), f(y), depth - 1)
            } else {
                self.set_bit(quad, f(x), f(y), depth - 1)
            }
        }
    }

    #[allow(clippy::collapsible_else_if)]
    fn get_quadrant(cell: Cell, x: i128, y: i128) -> usize {
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
    fn get_quadrant_mut(cell: &mut Cell, x: i128, y: i128) -> &mut usize {
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
