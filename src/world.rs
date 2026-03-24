use crate::rle_data::RleBuffer;
use crate::rle_data::RleBufferEntry;
use crate::rule_set::RuleSet;
use crate::{info_log, trace_log};

use crate::WorldOffset;
use crate::cell::{Cell, GC_UNREACHABLE, HASH_CHAIN_END, LEAF_MASK, RES_UNSET_MASK, VOID_MASK, bump_compute_count, cell_utils};

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
    pub hashpop: usize,

    /// Step size parameter. `next()` advances by `2^(k-1)` generations.
    /// k=0 means maximal (2^(depth-3) generations).
    /// Cached results (`cell.res`) are only valid for the current `k`.
    k: u8,
}

impl World {
    /// Create an empty new world
    pub fn new(rule: RuleSet) -> Self {
        let rules = rule.compute_rules();

        let buf = vec![];

        let mut world = Self {
            rules,
            root: 0,
            buf,
            depth: 3,
            hashtab: vec![HASH_CHAIN_END; INITIAL_HASH_SIZE],
            hashpop: 0,
            k: 1,
        };

        // Void cell lands at index 0 (first find_node call on empty buf)
        let void_idx = world.find_node(0, 0, 0, 0);
        debug_assert_eq!(void_idx, 0);

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

    /// Set the step size parameter. `next()` advances by `2^(k-1)` generations.
    /// k=0 means maximal (2^(depth-3) generations).
    /// Invalidates all cached results since they depend on `k`.
    pub fn set_k(&mut self, k: u8) {
        if k == self.k {
            return;
        }
        self.k = k;
        for cell in &mut self.buf {
            cell.res = RES_UNSET_MASK | (cell.res & VOID_MASK);
        }
    }

    /// Returns the current step size parameter.
    pub fn k(&self) -> u8 {
        self.k
    }

    /// Advance the world by `2^(k-1)` steps, where `k` is the current step size.
    pub fn next(&mut self) {
        info_log!(
            "next(k={}) start: depth={}, buf_len={}, hashpop={}",
            self.k, self.depth, self.buf.len(), self.hashpop
        );

        #[cfg(feature = "trace")]
        let t = std::time::Instant::now();
        self.root = self.compute(self.root, self.depth);
        self.depth -= 1;
        info_log!(
            "next: compute done in {:?}, buf_len={}, hashpop={}",
            t.elapsed(), self.buf.len(), self.hashpop
        );

        #[cfg(feature = "trace")]
        let t = std::time::Instant::now();
        self.grow(1);
        info_log!(
            "next: grow(1) done in {:?}, depth={}",
            t.elapsed(), self.depth
        );
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
        while idx != HASH_CHAIN_END {
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

        // Set the void bit if all children are void.
        // For the bootstrap case (buf empty, all children 0), this is trivially true.
        let all_void = if self.buf.is_empty() {
            nw == 0 && ne == 0 && sw == 0 && se == 0
        } else {
            self.buf[nw].is_void() && self.buf[ne].is_void()
                && self.buf[sw].is_void() && self.buf[se].is_void()
        };
        if all_void {
            cell.res |= VOID_MASK;
        }

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
        while idx != HASH_CHAIN_END {
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
        info_log!(
            "resize_hashtab: {} -> {}, hashpop={}",
            self.hashtab.len(), new_size, self.hashpop
        );
        let mut new_tab = vec![HASH_CHAIN_END; new_size];

        // Rehash all entries
        for i in 0..self.buf.len() {
            let h = self.buf[i].hash() % new_size;
            self.buf[i].next_hash = new_tab[h];
            new_tab[h] = i;
        }

        self.hashtab = new_tab;
    }

    /// Lightweight garbage collection: clears the hash table and reinserts only
    /// cells reachable from the root. Orphaned cells remain in buf but are no
    /// longer findable by `find_node`/`find_leaf`.
    pub fn gc(&mut self) {
        // Clear hash table
        self.hashtab.fill(HASH_CHAIN_END);
        self.hashpop = 0;

        // Mark all cells as unreachable
        for cell in self.buf.iter_mut() {
            cell.next_hash = GC_UNREACHABLE;
        }

        // Rewalk live tree from root, reinserting reachable cells
        self.gc_mark(self.root);
    }

    /// Recursively reinsert a cell and its children into the hash table.
    fn gc_mark(&mut self, idx: usize) {
        if self.buf[idx].next_hash != GC_UNREACHABLE {
            // Already visited and reinserted
            return;
        }

        // Insert into hash table first (marks as visited)
        let h = self.buf[idx].hash() % self.hashtab.len();
        self.buf[idx].next_hash = self.hashtab[h];
        self.hashtab[h] = idx;
        self.hashpop += 1;

        let cell = self.buf[idx];

        if !cell.is_leaf() {
            // Recurse into children
            self.gc_mark(cell.nw);
            self.gc_mark(cell.ne);
            self.gc_mark(cell.sw);
            self.gc_mark(cell.se);

            // Mark cached result if present.
            // Leaf results are u16 rules (not buf indices), so skip those.
            let res = cell.res & !VOID_MASK;
            if res & RES_UNSET_MASK == 0 {
                self.gc_mark(res);
            }
        }
    }

    /// Compute the result of a cell at depth `d`, using the current `k` value.
    ///
    /// The returned `usize` is either a buf index or a u16 rule (for leaf results).
    /// Results are cached in `cell.res`.
    pub fn compute(&mut self, idx: usize, d: u8) -> usize {
        bump_compute_count();
        let cell = self.buf[idx];

        // Check result cache (mask off VOID_MASK which is metadata, not part of the result)
        let cached = cell.res & !VOID_MASK;
        if cached & RES_UNSET_MASK == 0 {
            return cached;
        }

        let k = self.k;
        let do_phase2 = k == 0 || d <= k + 2;

        trace_log!(
            "compute: idx={}, k={}, d={}, phase2={}, void={}, leaf={}",
            idx, k, d, do_phase2, cell.is_void(), cell.is_leaf()
        );

        let res = if cell.is_void() {
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
                self.compute_node_full(idx, d)
            } else {
                self.compute_node_half(idx, d)
            };
            self.find_cell(result)
        };

        self.buf[idx].res = res | (self.buf[idx].res & VOID_MASK);
        res
    }

    /// Computes the full (phase-2) result of a node at depth `d`.
    #[rustfmt::skip]
    fn compute_node_full(&mut self, idx: usize, d: u8) -> Cell {
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

        let d1 = d - 1;
        let n00 = self.compute(nw,    d1);
        let n01 = self.compute(n_idx, d1);
        let n02 = self.compute(ne,    d1);
        let n10 = self.compute(w_idx, d1);
        let n11 = self.compute(c_idx, d1);
        let n12 = self.compute(e_idx, d1);
        let n20 = self.compute(sw,    d1);
        let n21 = self.compute(s_idx, d1);
        let n22 = self.compute(se,    d1);

        let tl = self.find_node(n00, n01, n10, n11);
        let tr = self.find_node(n01, n02, n11, n12);
        let bl = self.find_node(n10, n11, n20, n21);
        let br = self.find_node(n11, n12, n21, n22);

        let nw = self.compute(tl, d1);
        let ne = self.compute(tr, d1);
        let sw = self.compute(bl, d1);
        let se = self.compute(br, d1);

        Cell::new(nw, ne, sw, se)
    }

    /// Computes the half (no-phase-2) result of a node at depth `d`.
    #[rustfmt::skip]
    fn compute_node_half(&mut self, idx: usize, d: u8) -> Cell {
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
        let n00 = self.compute(nw,    d1);
        let n01 = self.compute(n_idx, d1);
        let n02 = self.compute(ne,    d1);
        let n10 = self.compute(w_idx, d1);
        let n11 = self.compute(c_idx, d1);
        let n12 = self.compute(e_idx, d1);
        let n20 = self.compute(sw,    d1);
        let n21 = self.compute(s_idx, d1);
        let n22 = self.compute(se,    d1);

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
        let n00 = self.compute(nw,    3) as u16;
        let n01 = self.compute(n_idx, 3) as u16;
        let n02 = self.compute(ne,    3) as u16;
        let n10 = self.compute(w_idx, 3) as u16;
        let n11 = self.compute(c_idx, 3) as u16;
        let n12 = self.compute(e_idx, 3) as u16;
        let n20 = self.compute(sw,    3) as u16;
        let n21 = self.compute(s_idx, 3) as u16;
        let n22 = self.compute(se,    3) as u16;

        // Build phase 2 leaves and compute their results
        let tl = self.find_leaf(n00, n01, n10, n11);
        let tr = self.find_leaf(n01, n02, n11, n12);
        let bl = self.find_leaf(n10, n11, n20, n21);
        let br = self.find_leaf(n11, n12, n21, n22);

        let tl_res = self.compute(tl, 3) as u16;
        let tr_res = self.compute(tr, 3) as u16;
        let bl_res = self.compute(bl, 3) as u16;
        let br_res = self.compute(br, 3) as u16;

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

        let n00 = self.compute(nw,    3) as u16;
        let n01 = self.compute(n_idx, 3) as u16;
        let n02 = self.compute(ne,    3) as u16;
        let n10 = self.compute(w_idx, 3) as u16;
        let n11 = self.compute(c_idx, 3) as u16;
        let n12 = self.compute(e_idx, 3) as u16;
        let n20 = self.compute(sw,    3) as u16;
        let n21 = self.compute(s_idx, 3) as u16;
        let n22 = self.compute(se,    3) as u16;

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

#[cfg(test)]
mod test_gc {
    use crate::cell::Cell;
    use crate::rule_set::B3S23;
    use crate::world::World;

    /// Build a World with a glider near the center at the given depth
    #[rustfmt::skip]
    fn make_glider(depth: u8) -> World {
        assert!(depth >= 4, "need at least a 16-cell for a glider");

        let mut buf = vec![Cell::void()];

        let empty_leaf = Cell::leaf(0, 0, 0, 0);
        let glider_leaf = Cell::leaf(0, 0, 0, 0b0010_0001_0111_0000);

        let el = buf.len(); buf.push(empty_leaf);
        let gl = buf.len(); buf.push(glider_leaf);

        let nw16 = Cell::new(el, el, el, gl);
        let empty16 = Cell::new(el, el, el, el);
        let nw16_idx = buf.len(); buf.push(nw16);
        let empty16_idx = buf.len(); buf.push(empty16);

        let mut root = Cell::new(nw16_idx, empty16_idx, empty16_idx, empty16_idx);

        for _ in 5..=depth {
            let root_idx = buf.len(); buf.push(root);
            root = Cell::new(root_idx, 0, 0, 0);
        }

        World::from_parts(B3S23, buf, root, depth)
    }

    /// Compare two world trees structurally (indices may differ)
    fn trees_equal(a: &World, a_idx: usize, b: &World, b_idx: usize) -> bool {
        let ca = a.buf[a_idx];
        let cb = b.buf[b_idx];
        if ca.is_void() && cb.is_void() { return true; }
        if ca.is_leaf() != cb.is_leaf() { return false; }
        if ca.is_leaf() {
            return ca == cb;
        }
        trees_equal(a, ca.nw, b, cb.nw)
            && trees_equal(a, ca.ne, b, cb.ne)
            && trees_equal(a, ca.sw, b, cb.sw)
            && trees_equal(a, ca.se, b, cb.se)
    }

    #[test]
    fn test_gc_preserves_tree() {
        // GC itself should not change the tree — only the hash table internals
        let mut world = make_glider(6);
        world.set_k(0);
        world.next();
        // Can't snapshot, so we just verify GC + step matches no-GC + step
        // (test_gc_step_matches_no_gc covers this). Here, verify GC doesn't
        // crash and hashpop is sane.
        let pop_before = world.hashpop;
        world.gc();
        let pop_after = world.hashpop;

        assert!(pop_after <= pop_before,
            "GC should not increase live count: {pop_after} vs {pop_before}");
        assert!(pop_after > 0, "GC should keep at least the root alive");
    }

    #[test]
    fn test_gc_step_matches_no_gc() {
        // Step, GC, step again — should match stepping without GC
        let mut world_gc = make_glider(6);
        let mut world_no_gc = make_glider(6);

        world_gc.set_k(0);
        world_gc.next();
        world_no_gc.set_k(0);
        world_no_gc.next();

        let buf_before_gc = world_gc.buf.len();
        world_gc.gc();
        let pop_after_gc = world_gc.hashpop;

        eprintln!("buf size: {buf_before_gc}, live cells after gc: {pop_after_gc}");
        assert!(pop_after_gc < buf_before_gc,
            "GC should reduce live cell count: {pop_after_gc} < {buf_before_gc}");

        world_gc.set_k(0);
        world_gc.next();
        world_no_gc.set_k(0);
        world_no_gc.next();

        assert!(trees_equal(&world_gc, world_gc.root, &world_no_gc, world_no_gc.root),
            "GC should not affect computation results");
    }

    #[test]
    fn test_gc_twice() {
        // GC twice in a row should be safe and idempotent
        let mut world = make_glider(6);
        world.set_k(0);
        world.next();

        world.gc();
        let pop_first = world.hashpop;

        world.gc();
        let pop_second = world.hashpop;

        assert_eq!(pop_first, pop_second,
            "Second GC should not change live count: {pop_first} vs {pop_second}");

        // Should still compute correctly after double GC
        let mut world_ref = make_glider(6);
        world_ref.set_k(0);
        world_ref.next();
        world_ref.set_k(0);
        world_ref.next();

        world.set_k(0);
        world.next();

        assert!(trees_equal(&world, world.root, &world_ref, world_ref.root),
            "Double GC should not affect computation");
    }

    #[test]
    fn test_gc_fresh_world() {
        // GC on a world with no computation should not crash
        // and should preserve the tree (verify by stepping after)
        let mut world_gc = make_glider(6);
        let mut world_ref = make_glider(6);

        world_gc.gc();

        world_gc.set_k(0);
        world_gc.next();
        world_ref.set_k(0);
        world_ref.next();

        assert!(trees_equal(&world_gc, world_gc.root, &world_ref, world_ref.root),
            "GC on fresh world should not affect subsequent computation");
    }

    #[test]
    fn test_gc_after_set() {
        // Build a pattern via set(), GC, step — should match stepping without GC
        let mut world_gc = World::new(B3S23);
        let mut world_ref = World::new(B3S23);

        for world in [&mut world_gc, &mut world_ref] {
            world.grow(3); // depth 6
            world.set(0, 0);
            world.set(1, 0);
            world.set(2, 0);
            world.set(2, -1);
            world.set(1, -2);
        }

        world_gc.gc();

        world_gc.set_k(0);
        world_gc.next();
        world_ref.set_k(0);
        world_ref.next();

        assert!(trees_equal(&world_gc, world_gc.root, &world_ref, world_ref.root),
            "GC after set() should not affect computation");
    }

    #[test]
    fn test_gc_preserves_result_cache() {
        use crate::cell::{reset_compute_count, get_compute_count};

        // Both worlds do the same two steps.
        // One GCs between steps, the other doesn't.
        // The GC world should do no more compute calls on the second step
        // than the non-GC world, proving caches on live cells survive GC.
        let mut world_gc = make_glider(6);
        let mut world_no_gc = make_glider(6);

        world_gc.set_k(0);
        world_gc.next();
        world_no_gc.set_k(0);
        world_no_gc.next();

        world_gc.gc();

        // Measure second step on both
        reset_compute_count();
        world_no_gc.set_k(0);
        world_no_gc.next();
        let count_no_gc = get_compute_count();

        reset_compute_count();
        world_gc.set_k(0);
        world_gc.next();
        let count_gc = get_compute_count();

        eprintln!("compute calls — with GC: {count_gc}, without GC: {count_no_gc}");

        // GC may lose some intermediate caches, so allow a small overhead,
        // but it should not be dramatically worse.
        let max_allowed = count_no_gc * 2;
        assert!(count_gc <= max_allowed,
            "Post-GC step should not be dramatically worse: {count_gc} vs {count_no_gc} (max {max_allowed})");
    }
}
