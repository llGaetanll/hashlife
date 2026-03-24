use crate::rle_data::RleBuffer;
use crate::rle_data::RleBufferEntry;
use crate::rule_set::RuleSet;

use crate::WorldOffset;
use crate::cell::{Cell, LEAF_MASK, bump_compute_count, cell_utils};

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
    pub fn new(rule: RuleSet) -> Self {
        let rules = rule.compute_rules();

        // First cell is the canonical void cell, second is the root, an uninitialized leaf
        let buf = vec![Cell::void(), Cell::leaf_uninit()];

        let root = 1;

        Self {
            rules,
            root,
            buf,
            depth: 3,
        }
    }

    /// Create a world from pre-built parts (for testing)
    pub fn from_parts(rule: RuleSet, buf: Vec<Cell>, root: Cell, depth: u8) -> Self {
        let rules = rule.compute_rules();
        let root_idx = buf.len();
        let mut buf = buf;
        buf.push(root);

        Self {
            rules,
            root: root_idx,
            buf,
            depth,
        }
    }

    pub fn from_rle(set: RuleSet, data: RleBuffer) -> Self {
        let mut world = Self::new(set);

        let mut x: WorldOffset = 0;
        let mut y: WorldOffset = 0;

        for entry in data.iter() {
            match entry {
                RleBufferEntry::DeadCell(n) => {
                    x += n as WorldOffset;
                }
                RleBufferEntry::LiveCell(n) => {
                    for _ in 0..n {
                        world.ensure_fits(x, y);
                        world.set(x, y);
                        x += 1;
                    }
                }
                RleBufferEntry::LineBreak(n) => {
                    x = 0;
                    y += n as WorldOffset;
                }
            }
        }

        world
    }

    /// Grow the world until `(x, y)` is within bounds
    fn ensure_fits(&mut self, x: WorldOffset, y: WorldOffset) {
        loop {
            let w = 1 << (self.depth - 1);
            if -w <= x && x < w && -w <= y && y < w {
                break;
            }
            self.grow(1);
        }
    }

    /// Advance the world by `2^(k-1)` steps (k=1 is 1 step, k=2 is 2 steps, etc.)
    /// k=0 is maximal (2^(depth-3) steps).
    pub fn next(&mut self, k: u8) {
        self.root = self.compute_k(self.root, k, self.depth);
        self.depth -= 1;

        self.grow(1);
    }

    /// Grows the world by a factor of 2^k, keeping the previous root at the origin
    pub fn grow(&mut self, k: usize) {
        if k == 0 {
            return;
        }

        self.root = self.grow_cell(self.root);
        self.depth += 1;

        self.grow(k - 1);
    }

    /// Grow the cell at `idx` about its center by a factor of 2, return new index
    fn grow_cell(&mut self, idx: usize) -> usize {
        let cell = self.buf[idx];
        let mask = if cell.is_leaf() { LEAF_MASK } else { 0 };

        let nw = Cell {
            nw: mask,
            ne: 0,
            sw: 0,
            se: cell.nw & !mask,
        };

        let ne = Cell {
            nw: mask,
            ne: 0,
            sw: cell.ne,
            se: 0,
        };

        let sw = Cell {
            nw: mask,
            ne: cell.sw,
            sw: 0,
            se: 0,
        };

        let se = Cell {
            nw: cell.se | mask,
            ne: 0,
            sw: 0,
            se: 0,
        };

        let nw_idx = self.buf.len(); self.buf.push(nw);
        let ne_idx = self.buf.len(); self.buf.push(ne);
        let sw_idx = self.buf.len(); self.buf.push(sw);
        let se_idx = self.buf.len(); self.buf.push(se);

        let grown = Cell::new(nw_idx, ne_idx, sw_idx, se_idx);
        let grown_idx = self.buf.len();
        self.buf.push(grown);
        grown_idx
    }

    /// Push a cell into buf and return its index
    fn push_cell(&mut self, cell: Cell) -> usize {
        let idx = self.buf.len();
        self.buf.push(cell);
        idx
    }

    /// Compute the result of a cell, dispatching based on type.
    ///
    /// The returned `usize` is either a buf index or a u16 rule (for leaf results).
    pub fn compute_full(&mut self, idx: usize) -> usize {
        bump_compute_count();
        let cell = self.buf[idx];

        if cell.is_void() {
            0
        } else if cell.is_leaf() {
            self.compute_leaf(idx) as usize
        } else if cell.is_16(&self.buf) {
            let result = self.compute_node16_full(idx);
            self.push_cell(result)
        } else {
            let result = self.compute_node_full(idx);
            self.push_cell(result)
        }
    }

    /// Unified compute with `k` and depth `d`.
    ///
    /// Phase 2 runs at depth `d` if `k == 0` (maximal) or `d <= k + 2`.
    pub fn compute_k(&mut self, idx: usize, k: u8, d: u8) -> usize {
        bump_compute_count();
        let cell = self.buf[idx];
        let do_phase2 = k == 0 || d <= k + 2;

        if cell.is_void() {
            0
        } else if cell.is_leaf() {
            self.compute_leaf(idx) as usize
        } else if cell.is_16(&self.buf) {
            let result = if do_phase2 {
                self.compute_node16_full(idx)
            } else {
                self.compute_node16_half(idx)
            };
            self.push_cell(result)
        } else {
            let result = if do_phase2 {
                self.compute_node_full_k(idx, k, d)
            } else {
                self.compute_node_half_k(idx, k, d)
            };
            self.push_cell(result)
        }
    }

    /// Computes the result of a 2^k cell for k > 4
    #[rustfmt::skip]
    fn compute_node_full(&mut self, idx: usize) -> Cell {
        let cell = self.buf[idx];
        let nw = cell.nw;
        let ne = cell.ne;
        let sw = cell.sw;
        let se = cell.se;

        // cardinal pseudo-cells
        let n = cell_utils::h_center(self.buf[nw], self.buf[ne]);
        let s = cell_utils::h_center(self.buf[sw], self.buf[se]);
        let e = cell_utils::v_center(self.buf[ne], self.buf[se]);
        let w = cell_utils::v_center(self.buf[nw], self.buf[sw]);
        let c = cell_utils::center(cell, &self.buf);

        let n_idx = self.push_cell(n);
        let s_idx = self.push_cell(s);
        let e_idx = self.push_cell(e);
        let w_idx = self.push_cell(w);
        let c_idx = self.push_cell(c);

        let n00 = self.compute_full(nw);
        let n01 = self.compute_full(n_idx);
        let n02 = self.compute_full(ne);
        let n10 = self.compute_full(w_idx);
        let n11 = self.compute_full(c_idx);
        let n12 = self.compute_full(e_idx);
        let n20 = self.compute_full(sw);
        let n21 = self.compute_full(s_idx);
        let n22 = self.compute_full(se);

        let tl = self.push_cell(Cell::new(n00, n01, n10, n11));
        let tr = self.push_cell(Cell::new(n01, n02, n11, n12));
        let bl = self.push_cell(Cell::new(n10, n11, n20, n21));
        let br = self.push_cell(Cell::new(n11, n12, n21, n22));

        let nw = self.compute_full(tl);
        let ne = self.compute_full(tr);
        let sw = self.compute_full(bl);
        let se = self.compute_full(br);

        Cell::new(nw, ne, sw, se)
    }

    /// Phase 2 variant: runs phase 1 with `compute_k`, phase 2 with `compute_full`.
    #[rustfmt::skip]
    fn compute_node_full_k(&mut self, idx: usize, k: u8, d: u8) -> Cell {
        let cell = self.buf[idx];
        let nw = cell.nw;
        let ne = cell.ne;
        let sw = cell.sw;
        let se = cell.se;

        let n = cell_utils::h_center(self.buf[nw], self.buf[ne]);
        let s = cell_utils::h_center(self.buf[sw], self.buf[se]);
        let e = cell_utils::v_center(self.buf[ne], self.buf[se]);
        let w = cell_utils::v_center(self.buf[nw], self.buf[sw]);
        let c = cell_utils::center(cell, &self.buf);

        let n_idx = self.push_cell(n);
        let s_idx = self.push_cell(s);
        let e_idx = self.push_cell(e);
        let w_idx = self.push_cell(w);
        let c_idx = self.push_cell(c);

        let d1 = d - 1;
        let n00 = self.compute_k(nw,    k, d1);
        let n01 = self.compute_k(n_idx, k, d1);
        let n02 = self.compute_k(ne,    k, d1);
        let n10 = self.compute_k(w_idx, k, d1);
        let n11 = self.compute_k(c_idx, k, d1);
        let n12 = self.compute_k(e_idx, k, d1);
        let n20 = self.compute_k(sw,    k, d1);
        let n21 = self.compute_k(s_idx, k, d1);
        let n22 = self.compute_k(se,    k, d1);

        let tl = self.push_cell(Cell::new(n00, n01, n10, n11));
        let tr = self.push_cell(Cell::new(n01, n02, n11, n12));
        let bl = self.push_cell(Cell::new(n10, n11, n20, n21));
        let br = self.push_cell(Cell::new(n11, n12, n21, n22));

        let nw = self.compute_full(tl);
        let ne = self.compute_full(tr);
        let sw = self.compute_full(bl);
        let se = self.compute_full(br);

        Cell::new(nw, ne, sw, se)
    }

    /// No-phase-2 variant: runs phase 1 with `compute_k`, then extracts centers.
    #[rustfmt::skip]
    fn compute_node_half_k(&mut self, idx: usize, k: u8, d: u8) -> Cell {
        let cell = self.buf[idx];

        let results_are_leaves = self.buf[cell.nw].is_16(&self.buf)
            || self.buf[cell.ne].is_16(&self.buf)
            || self.buf[cell.sw].is_16(&self.buf)
            || self.buf[cell.se].is_16(&self.buf);

        let nw = cell.nw;
        let ne = cell.ne;
        let sw = cell.sw;
        let se = cell.se;

        let n = cell_utils::h_center(self.buf[nw], self.buf[ne]);
        let s = cell_utils::h_center(self.buf[sw], self.buf[se]);
        let e = cell_utils::v_center(self.buf[ne], self.buf[se]);
        let w = cell_utils::v_center(self.buf[nw], self.buf[sw]);
        let c = cell_utils::center(cell, &self.buf);

        let n_idx = self.push_cell(n);
        let s_idx = self.push_cell(s);
        let e_idx = self.push_cell(e);
        let w_idx = self.push_cell(w);
        let c_idx = self.push_cell(c);

        let d1 = d - 1;
        let n00 = self.compute_k(nw,    k, d1);
        let n01 = self.compute_k(n_idx, k, d1);
        let n02 = self.compute_k(ne,    k, d1);
        let n10 = self.compute_k(w_idx, k, d1);
        let n11 = self.compute_k(c_idx, k, d1);
        let n12 = self.compute_k(e_idx, k, d1);
        let n20 = self.compute_k(sw,    k, d1);
        let n21 = self.compute_k(s_idx, k, d1);
        let n22 = self.compute_k(se,    k, d1);

        // Skip phase 2: extract centers
        if results_are_leaves {
            let nw = self.push_cell(Cell::leaf(
                self.buf[n00].se as u16,
                self.buf[n01].sw as u16,
                self.buf[n10].ne as u16,
                (self.buf[n11].nw & !LEAF_MASK) as u16,
            ));
            let ne = self.push_cell(Cell::leaf(
                self.buf[n01].se as u16,
                self.buf[n02].sw as u16,
                self.buf[n11].ne as u16,
                (self.buf[n12].nw & !LEAF_MASK) as u16,
            ));
            let sw = self.push_cell(Cell::leaf(
                self.buf[n10].se as u16,
                self.buf[n11].sw as u16,
                self.buf[n20].ne as u16,
                (self.buf[n21].nw & !LEAF_MASK) as u16,
            ));
            let se = self.push_cell(Cell::leaf(
                self.buf[n11].se as u16,
                self.buf[n12].sw as u16,
                self.buf[n21].ne as u16,
                (self.buf[n22].nw & !LEAF_MASK) as u16,
            ));

            Cell::new(nw, ne, sw, se)
        } else {
            let nw = self.push_cell(cell_utils::center(
                Cell::new(n00, n01, n10, n11), &self.buf));
            let ne = self.push_cell(cell_utils::center(
                Cell::new(n01, n02, n11, n12), &self.buf));
            let sw = self.push_cell(cell_utils::center(
                Cell::new(n10, n11, n20, n21), &self.buf));
            let se = self.push_cell(cell_utils::center(
                Cell::new(n11, n12, n21, n22), &self.buf));

            Cell::new(nw, ne, sw, se)
        }
    }

    /// Computes the result of a 16 cell. Returns a leaf.
    #[rustfmt::skip]
    fn compute_node16_full(&mut self, idx: usize) -> Cell {
        let cell = self.buf[idx];
        let nw = cell.nw;
        let ne = cell.ne;
        let sw = cell.sw;
        let se = cell.se;

        // cardinal pseudo-leaves
        let n_idx = self.push_cell(cell_utils::h_center8(self.buf[nw], self.buf[ne]));
        let s_idx = self.push_cell(cell_utils::h_center8(self.buf[sw], self.buf[se]));
        let e_idx = self.push_cell(cell_utils::v_center8(self.buf[ne], self.buf[se]));
        let w_idx = self.push_cell(cell_utils::v_center8(self.buf[nw], self.buf[sw]));
        let c_idx = self.push_cell(cell_utils::center16(cell, &self.buf));

        // All of these are rules (u16 results from leaves)
        let n00 = self.compute_full(nw)    as u16;
        let n01 = self.compute_full(n_idx) as u16;
        let n02 = self.compute_full(ne)    as u16;
        let n10 = self.compute_full(w_idx) as u16;
        let n11 = self.compute_full(c_idx) as u16;
        let n12 = self.compute_full(e_idx) as u16;
        let n20 = self.compute_full(sw)    as u16;
        let n21 = self.compute_full(s_idx) as u16;
        let n22 = self.compute_full(se)    as u16;

        // Build phase 2 leaves and compute their results
        let tl = self.push_cell(Cell::leaf(n00, n01, n10, n11));
        let tr = self.push_cell(Cell::leaf(n01, n02, n11, n12));
        let bl = self.push_cell(Cell::leaf(n10, n11, n20, n21));
        let br = self.push_cell(Cell::leaf(n11, n12, n21, n22));

        let tl_res = self.compute_full(tl) as u16;
        let tr_res = self.compute_full(tr) as u16;
        let bl_res = self.compute_full(bl) as u16;
        let br_res = self.compute_full(br) as u16;

        Cell::leaf(tl_res, tr_res, bl_res, br_res)
    }

    /// Like compute_node16_full but skips phase 2.
    #[rustfmt::skip]
    fn compute_node16_half(&mut self, idx: usize) -> Cell {
        let cell = self.buf[idx];
        let nw = cell.nw;
        let ne = cell.ne;
        let sw = cell.sw;
        let se = cell.se;

        let n_idx = self.push_cell(cell_utils::h_center8(self.buf[nw], self.buf[ne]));
        let s_idx = self.push_cell(cell_utils::h_center8(self.buf[sw], self.buf[se]));
        let e_idx = self.push_cell(cell_utils::v_center8(self.buf[ne], self.buf[se]));
        let w_idx = self.push_cell(cell_utils::v_center8(self.buf[nw], self.buf[sw]));
        let c_idx = self.push_cell(cell_utils::center16(cell, &self.buf));

        let n00 = self.compute_full(nw)    as u16;
        let n01 = self.compute_full(n_idx) as u16;
        let n02 = self.compute_full(ne)    as u16;
        let n10 = self.compute_full(w_idx) as u16;
        let n11 = self.compute_full(c_idx) as u16;
        let n12 = self.compute_full(e_idx) as u16;
        let n20 = self.compute_full(sw)    as u16;
        let n21 = self.compute_full(s_idx) as u16;
        let n22 = self.compute_full(se)    as u16;

        // Skip phase 2: extract centers
        let center = |a: u16, b: u16, c: u16, d: u16| -> u16 {
            let a = a & 0b0000_0000_0011_0011;
            let b = b & 0b0000_0000_1100_1100;
            let c = c & 0b0011_0011_0000_0000;
            let d = d & 0b1100_1100_0000_0000;
            (a << 10) | (b << 6) | (c >> 6) | (d >> 10)
        };

        Cell::leaf(
            center(n00, n01, n10, n11),
            center(n01, n02, n11, n12),
            center(n10, n11, n20, n21),
            center(n11, n12, n21, n22),
        )
    }

    /// Compute the result of a leaf cell. Returns a u16 rule.
    #[rustfmt::skip]
    pub fn compute_leaf(&mut self, idx: usize) -> u16 {
        let cell = self.buf[idx];
        assert!(cell.is_leaf());

        // Read nw without the leaf mask
        let nw = (cell.nw & !LEAF_MASK) as u16;
        let ne = cell.ne as u16;
        let sw = cell.sw as u16;
        let se = cell.se as u16;

        let t00 =   nw & 0b0000_0110_0110_0000;

        let t01 = ((nw & 0b0000_0001_0001_0000) << 2)
                | ((ne & 0b0000_1000_1000_0000) >> 2);

        let t02 =   ne & 0b0000_0110_0110_0000;

        let t10 = ((nw & 0b0000_0000_0000_0110) << 8)
                | ((sw & 0b0110_0000_0000_0000) >> 8);

        let t11 = ((nw & 0b0000_0000_0000_0001) << 10)
                | ((ne & 0b0000_0000_0000_1000) << 6)
                | ((sw & 0b0001_0000_0000_0000) >> 6)
                | ((se & 0b1000_0000_0000_0000) >> 10);

        let t12 = ((ne & 0b0000_0000_0000_0110) << 8)
                | ((se & 0b0110_0000_0000_0000) >> 8);

        let t20 =   sw & 0b0000_0110_0110_0000;

        let t21 = ((sw & 0b0000_0001_0001_0000) << 2)
                | ((se & 0b0000_1000_1000_0000) >> 2);

        let t22 =   se & 0b0000_0110_0110_0000;

        let tl = (t00 << 5) | (t01 << 3) | (t10 >> 3) | (t11 >> 5);
        let tr = (t01 << 5) | (t02 << 3) | (t11 >> 3) | (t12 >> 5);
        let bl = (t10 << 5) | (t11 << 3) | (t20 >> 3) | (t21 >> 5);
        let br = (t11 << 5) | (t12 << 3) | (t21 >> 3) | (t22 >> 5);

        (self.rules[tl as usize] << 5)
         | (self.rules[tr as usize] << 3)
         | (self.rules[bl as usize] >> 3)
         | (self.rules[br as usize] >> 5)
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
            if y < 0 { cell.sw } else { cell.nw }
        } else {
            if y < 0 { cell.se } else { cell.ne }
        }
    }

    #[allow(clippy::collapsible_else_if)]
    fn get_quadrant_mut(cell: &mut Cell, x: i128, y: i128) -> &mut usize {
        if x < 0 {
            if y < 0 { &mut cell.sw } else { &mut cell.nw }
        } else {
            if y < 0 { &mut cell.se } else { &mut cell.ne }
        }
    }

    /// Print the world tree structure for debugging
    pub fn dump_tree(&self) {
        eprintln!("World: depth={}, root={}, buf.len={}", self.depth, self.root, self.buf.len());
        self.dump_node(self.root, self.depth, 0);
    }

    fn dump_node(&self, idx: usize, depth: u8, indent: usize) {
        let cell = self.buf[idx];
        let prefix = "  ".repeat(indent);

        if cell.is_void() {
            eprintln!("{prefix}[{idx}] void");
        } else if cell.is_leaf() {
            eprintln!("{prefix}[{idx}] {cell:?}");
        } else if depth <= 3 {
            // Expected a leaf or void at depth 3, but got a node — flag it
            eprintln!("{prefix}[{idx}] BUG: node at depth {depth}: {cell:?}");
        } else {
            eprintln!("{prefix}[{idx}] node (depth={depth}):");
            for child in [cell.nw, cell.ne, cell.sw, cell.se] {
                self.dump_node(child, depth - 1, indent + 1);
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
