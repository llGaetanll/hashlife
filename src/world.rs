use crate::rle_data::RleBuffer;
use crate::rle_data::RleBufferEntry;
use crate::rule_set::RuleSet;

use crate::WorldOffset;
use crate::cell::{Cell, LEAF_MASK, RES_UNSET_MASK, bump_compute_count, cell_utils};

const INITIAL_HASH_SIZE: usize = 1021;

fn next_prime(mut n: usize) -> usize {
    if n.is_multiple_of(2) { n += 1; }
    while !is_prime(n) { n += 2; }
    n
}

fn is_prime(n: usize) -> bool {
    if n < 2 { return false; }
    if n < 4 { return true; }
    if n.is_multiple_of(2) || n.is_multiple_of(3) { return false; }
    let mut i = 5;
    while i * i <= n {
        if n.is_multiple_of(i) || n.is_multiple_of(i + 2) { return false; }
        i += 6;
    }
    true
}

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

    /// Hash table: each slot holds a buf index (head of chain) or 0 (empty)
    hashtab: Vec<usize>,

    /// Number of entries in the hash table
    hashpop: usize,
}

impl World {
    /// Create an empty new world
    pub fn new(rule: RuleSet) -> Self {
        let rules = rule.compute_rules();

        // Index 0 is the void cell — used as a sentinel for empty quadrants
        // and as the hash chain terminator. Not inserted into the hash table.
        let buf = vec![Cell::void()];

        let mut world = Self {
            rules,
            root: 0,
            buf,
            depth: 3,
            hashtab: vec![0; INITIAL_HASH_SIZE],
            hashpop: 0,
        };

        world.root = world.find_leaf(0, 0, 0, 0);
        world
    }

    /// Create a world from pre-built parts (for testing).
    /// Rebuilds the tree through the hash table so all cells are canonical.
    pub fn from_parts(rule: RuleSet, buf: Vec<Cell>, root: Cell, depth: u8) -> Self {
        let mut world = Self::new(rule);
        world.depth = depth;

        let old_buf = buf;
        let root_idx = old_buf.len();
        // Temporarily append root so we can refer to it by index
        let mut old_buf = old_buf;
        old_buf.push(root);

        world.root = world.canonicalize(&old_buf, root_idx);
        world
    }

    /// Recursively insert a cell from `old_buf` into the canonical hash table.
    fn canonicalize(&mut self, old_buf: &[Cell], idx: usize) -> usize {
        let cell = old_buf[idx];

        if cell.is_void() {
            return 0;
        }

        if cell.is_leaf() {
            return self.find_leaf(
                (cell.nw & !LEAF_MASK) as u16,
                cell.ne as u16,
                cell.sw as u16,
                cell.se as u16,
            );
        }

        let nw = self.canonicalize(old_buf, cell.nw);
        let ne = self.canonicalize(old_buf, cell.ne);
        let sw = self.canonicalize(old_buf, cell.sw);
        let se = self.canonicalize(old_buf, cell.se);
        self.find_node(nw, ne, sw, se)
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

        let (nw_idx, ne_idx, sw_idx, se_idx) = if cell.is_leaf() {
            (
                self.find_leaf(0, 0, 0, (cell.nw & !LEAF_MASK) as u16),
                self.find_leaf(0, 0, cell.ne as u16, 0),
                self.find_leaf(0, cell.sw as u16, 0, 0),
                self.find_leaf(cell.se as u16, 0, 0, 0),
            )
        } else {
            (
                self.find_node(0, 0, 0, cell.nw),
                self.find_node(0, 0, cell.ne, 0),
                self.find_node(0, cell.sw, 0, 0),
                self.find_node(cell.se, 0, 0, 0),
            )
        };

        self.find_node(nw_idx, ne_idx, sw_idx, se_idx)
    }

    /// Find or create a cell (dispatches to find_node or find_leaf)
    fn find_cell(&mut self, cell: Cell) -> usize {
        if cell.is_leaf() {
            self.find_leaf(
                (cell.nw & !LEAF_MASK) as u16,
                cell.ne as u16,
                cell.sw as u16,
                cell.se as u16,
            )
        } else {
            self.find_node(cell.nw, cell.ne, cell.sw, cell.se)
        }
    }

    /// Find or create a node with the given children. Returns buf index.
    fn find_node(&mut self, nw: usize, ne: usize, sw: usize, se: usize) -> usize {
        let cell = Cell::new(nw, ne, sw, se);
        let h = cell.hash() % self.hashtab.len();

        // Walk the chain
        let mut idx = self.hashtab[h];
        while idx != 0 {
            let existing = self.buf[idx];
            if existing.nw == nw && existing.ne == ne
                && existing.sw == sw && existing.se == se
                && !existing.is_leaf()
            {
                return idx;
            }
            idx = existing.next_hash;
        }

        // Not found — insert
        let new_idx = self.buf.len();
        let mut cell = cell;
        cell.next_hash = self.hashtab[h];
        self.buf.push(cell);
        self.hashtab[h] = new_idx;
        self.hashpop += 1;

        if self.hashpop > self.hashtab.len() {
            self.resize_hashtab();
        }

        new_idx
    }

    /// Find or create a leaf with the given quadrants. Returns buf index.
    fn find_leaf(&mut self, nw: u16, ne: u16, sw: u16, se: u16) -> usize {
        let cell = Cell::leaf(nw, ne, sw, se);
        let h = cell.hash() % self.hashtab.len();

        // Walk the chain
        let mut idx = self.hashtab[h];
        while idx != 0 {
            let existing = self.buf[idx];
            if existing.is_leaf()
                && (existing.nw & !LEAF_MASK) as u16 == nw
                && existing.ne as u16 == ne
                && existing.sw as u16 == sw
                && existing.se as u16 == se
            {
                return idx;
            }
            idx = existing.next_hash;
        }

        // Not found — insert
        let new_idx = self.buf.len();
        let mut cell = cell;
        cell.next_hash = self.hashtab[h];
        self.buf.push(cell);
        self.hashtab[h] = new_idx;
        self.hashpop += 1;

        if self.hashpop > self.hashtab.len() {
            self.resize_hashtab();
        }

        new_idx
    }

    /// Resize the hash table to roughly double its size (next prime).
    fn resize_hashtab(&mut self) {
        let new_size = next_prime(self.hashtab.len() * 2);
        let mut new_tab = vec![0usize; new_size];

        // Rehash all entries
        for i in 1..self.buf.len() {
            let cell = self.buf[i];
            if cell.is_void() && i == 0 {
                continue;
            }
            let h = cell.hash() % new_size;
            self.buf[i].next_hash = new_tab[h];
            new_tab[h] = i;
        }

        self.hashtab = new_tab;
    }

    /// Compute the result of a cell, dispatching based on type.
    ///
    /// The returned `usize` is either a buf index or a u16 rule (for leaf results).
    pub fn compute_full(&mut self, idx: usize) -> usize {
        bump_compute_count();
        let cell = self.buf[idx];

        // Check result cache
        if cell.res != RES_UNSET_MASK {
            return cell.res;
        }

        let res = if cell.is_void() {
            0
        } else if cell.is_leaf() {
            self.compute_leaf(idx) as usize
        } else if cell.is_16(&self.buf) {
            let result = self.compute_node16_full(idx);
            self.find_cell(result)
        } else {
            let result = self.compute_node_full(idx);
            self.find_cell(result)
        };

        self.buf[idx].res = res;
        res
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
            self.find_cell(result)
        } else {
            let result = if do_phase2 {
                self.compute_node_full_k(idx, k, d)
            } else {
                self.compute_node_half_k(idx, k, d)
            };
            self.find_cell(result)
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

        let n_idx = self.find_cell(n);
        let s_idx = self.find_cell(s);
        let e_idx = self.find_cell(e);
        let w_idx = self.find_cell(w);
        let c_idx = self.find_cell(c);

        let n00 = self.compute_full(nw);
        let n01 = self.compute_full(n_idx);
        let n02 = self.compute_full(ne);
        let n10 = self.compute_full(w_idx);
        let n11 = self.compute_full(c_idx);
        let n12 = self.compute_full(e_idx);
        let n20 = self.compute_full(sw);
        let n21 = self.compute_full(s_idx);
        let n22 = self.compute_full(se);

        let tl = self.find_node(n00, n01, n10, n11);
        let tr = self.find_node(n01, n02, n11, n12);
        let bl = self.find_node(n10, n11, n20, n21);
        let br = self.find_node(n11, n12, n21, n22);

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

        let n_idx = self.find_cell(n);
        let s_idx = self.find_cell(s);
        let e_idx = self.find_cell(e);
        let w_idx = self.find_cell(w);
        let c_idx = self.find_cell(c);

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

        let tl = self.find_node(n00, n01, n10, n11);
        let tr = self.find_node(n01, n02, n11, n12);
        let bl = self.find_node(n10, n11, n20, n21);
        let br = self.find_node(n11, n12, n21, n22);

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

        let n_idx = self.find_cell(n);
        let s_idx = self.find_cell(s);
        let e_idx = self.find_cell(e);
        let w_idx = self.find_cell(w);
        let c_idx = self.find_cell(c);

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
            let nw = self.find_leaf(
                self.buf[n00].se as u16,
                self.buf[n01].sw as u16,
                self.buf[n10].ne as u16,
                (self.buf[n11].nw & !LEAF_MASK) as u16,
            );
            let ne = self.find_leaf(
                self.buf[n01].se as u16,
                self.buf[n02].sw as u16,
                self.buf[n11].ne as u16,
                (self.buf[n12].nw & !LEAF_MASK) as u16,
            );
            let sw = self.find_leaf(
                self.buf[n10].se as u16,
                self.buf[n11].sw as u16,
                self.buf[n20].ne as u16,
                (self.buf[n21].nw & !LEAF_MASK) as u16,
            );
            let se = self.find_leaf(
                self.buf[n11].se as u16,
                self.buf[n12].sw as u16,
                self.buf[n21].ne as u16,
                (self.buf[n22].nw & !LEAF_MASK) as u16,
            );

            Cell::new(nw, ne, sw, se)
        } else {
            let nw = self.find_cell(cell_utils::center(
                Cell::new(n00, n01, n10, n11), &self.buf));
            let ne = self.find_cell(cell_utils::center(
                Cell::new(n01, n02, n11, n12), &self.buf));
            let sw = self.find_cell(cell_utils::center(
                Cell::new(n10, n11, n20, n21), &self.buf));
            let se = self.find_cell(cell_utils::center(
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
        let n_idx = self.find_cell(cell_utils::h_center8(self.buf[nw], self.buf[ne]));
        let s_idx = self.find_cell(cell_utils::h_center8(self.buf[sw], self.buf[se]));
        let e_idx = self.find_cell(cell_utils::v_center8(self.buf[ne], self.buf[se]));
        let w_idx = self.find_cell(cell_utils::v_center8(self.buf[nw], self.buf[sw]));
        let c_idx = self.find_cell(cell_utils::center16(cell, &self.buf));

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
        let tl = self.find_leaf(n00, n01, n10, n11);
        let tr = self.find_leaf(n01, n02, n11, n12);
        let bl = self.find_leaf(n10, n11, n20, n21);
        let br = self.find_leaf(n11, n12, n21, n22);

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

        let n_idx = self.find_cell(cell_utils::h_center8(self.buf[nw], self.buf[ne]));
        let s_idx = self.find_cell(cell_utils::h_center8(self.buf[sw], self.buf[se]));
        let e_idx = self.find_cell(cell_utils::v_center8(self.buf[ne], self.buf[se]));
        let w_idx = self.find_cell(cell_utils::v_center8(self.buf[nw], self.buf[sw]));
        let c_idx = self.find_cell(cell_utils::center16(cell, &self.buf));

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

        self.root = self.set_bit(root, x, y, self.depth);
    }

    /// Set a bit in the tree, returning the new root index for this subtree.
    /// Does not mutate existing cells — rebuilds the path with canonical nodes.
    fn set_bit(&mut self, ptr: usize, x: WorldOffset, y: WorldOffset, depth: u8) -> usize {
        assert!(depth >= 3);

        if depth == 3 {
            // Leaf: read, modify the relevant quadrant, create new canonical leaf
            let cell = self.buf[ptr];
            let mut nw = (cell.nw & !LEAF_MASK) as u16;
            let mut ne = cell.ne as u16;
            let mut sw = cell.sw as u16;
            let mut se = cell.se as u16;

            let bit = 1 << (3 - (x & 3) + 4 * (y & 3));
            let quad = Self::get_quadrant_mut_u16(&mut nw, &mut ne, &mut sw, &mut se, x, y);
            *quad |= bit;

            self.find_leaf(nw, ne, sw, se)
        } else {
            // Non-leaf: recurse into the appropriate child, rebuild this node
            let cell = self.buf[ptr];
            let mut children = [cell.nw, cell.ne, cell.sw, cell.se];
            let child_idx = Self::quadrant_index(x, y);
            let child = children[child_idx];

            let w = 1 << depth;
            let f = |c: WorldOffset| c - if c < 0 { -(w >> 2) } else { w >> 2 };

            // Create empty child if needed
            let child = if child == 0 {
                if depth == 4 { self.add_leaf() } else { self.add_node() }
            } else {
                child
            };

            children[child_idx] = self.set_bit(child, f(x), f(y), depth - 1);
            self.find_node(children[0], children[1], children[2], children[3])
        }
    }

    /// Returns which quadrant index (0=nw, 1=ne, 2=sw, 3=se) a coordinate falls in
    fn quadrant_index(x: WorldOffset, y: WorldOffset) -> usize {
        match (x >= 0, y < 0) {
            (false, false) => 0, // nw
            (true, false)  => 1, // ne
            (false, true)  => 2, // sw
            (true, true)   => 3, // se
        }
    }

    /// Get a mutable reference to the appropriate u16 quadrant
    fn get_quadrant_mut_u16<'a>(
        nw: &'a mut u16, ne: &'a mut u16, sw: &'a mut u16, se: &'a mut u16,
        x: WorldOffset, y: WorldOffset,
    ) -> &'a mut u16 {
        match Self::quadrant_index(x, y) {
            0 => nw,
            1 => ne,
            2 => sw,
            _ => se,
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

    /// Add an empty leaf cell to the world and return its index
    fn add_leaf(&mut self) -> usize {
        self.find_leaf(0, 0, 0, 0)
    }

    /// Add an empty non-leaf cell to the world and return its index
    fn add_node(&mut self) -> usize {
        self.find_node(0, 0, 0, 0)
    }
}
