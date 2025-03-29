/// On 64 bit machines: 1 followed by 63 0s, `9_223_372_036_854_775_808`.
/// On 32 bit machines: 1 followed by 31 0s, `2_147_483_648`.
///
/// We make an important assumption here, that our memory buffer will never contain this many
/// entries. Under this assumption, we can use the most significant bit of our `nw` index to
/// indicate whether the current cell is a leaf. This keeps the structure small, and the routine
/// fast.
pub const LEAF_MASK: usize = {
    const WORD_SIZE_BITS: usize = std::mem::size_of::<usize>() * 8;

    1usize << (WORD_SIZE_BITS - 1)
};

/// If we see a leading bit on `res`, that means the result is not computed
const RES_UNSET_MASK: usize = LEAF_MASK;

/// A `CellHash` is either an index into a list of `Cell`s, or a leaf cell
pub type CellHash = usize;

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Cell {
    pub nw: CellHash,
    pub ne: CellHash,
    pub sw: CellHash,
    pub se: CellHash,

    /// The index of the result of a [`Cell`].
    ///
    /// The result of a 2^n cell is a pointer to a 2^{n - 1} cell,
    /// specifically it's what this cell will look like in 2^{n - 2}
    /// iterations.
    pub res: CellHash,
}

impl Cell {
    /// Return the canonical "empty" cell
    pub const fn void() -> Self {
        Self {
            nw: 0,
            ne: 0,
            sw: 0,
            se: 0,
            res: RES_UNSET_MASK,
        }
    }

    /// Return an unset cell
    pub const fn unset() -> Self {
        Self {
            nw: 0,
            ne: 0,
            sw: 0,
            se: 0,
            res: RES_UNSET_MASK,
        }
    }

    fn next(&mut self, next: &[u16], buf: &[Cell]) -> Cell {
        let index = self.hash() % buf.len();

        // check if the result is in cache
        if buf[index] != Cell::unset() {
            let _ = buf[index];
        } else {
            self.compute_res(next, buf);
        }

        todo!()
    }

    // WARNING: A leaf check would fail after this. It's important to remask the leaf as early as
    // possible.
    fn unmask_leaf(&mut self) {
        assert!(self.is_leaf());

        self.nw ^= LEAF_MASK;
    }

    fn mask_leaf(&mut self) {
        // We mask leaves so that we have a way to differentiate between non-leaf cells and leaf
        // cells
        self.nw &= LEAF_MASK;
    }

    /// Check if the cell is a leaf.
    pub fn is_leaf(&self) -> bool {
        self.nw & LEAF_MASK == LEAF_MASK
    }

    pub fn children(&self) -> Option<[usize; 4]> {
        if self.is_leaf() {
            None
        } else {
            Some([self.nw, self.ne, self.sw, self.se])
        }
    }

    /// Compute the result of a cell, in case the cache failed
    fn compute_res(&mut self, next: &[u16], buf: &[Cell]) {
        if self.is_leaf() {
            self.compute_leaf_res(next);
        } else {
            self.compute_node_res(buf);
        }
    }

    /// For a leaf cell, this computes its result.
    ///
    ///   t00 t01 t02
    ///   t10 t11 t12
    ///   t20 t21 t22
    ///
    fn compute_leaf_res(&mut self, next: &[u16]) {
        assert!(self.is_leaf());

        self.unmask_leaf();
        {
            let t00 = self.nw as u16 & 0b0000_0110_0110_0000;

            let t01 = ((self.nw as u16 & 0b0000_0001_0001_0000) << 2)
                & ((self.ne as u16 & 0b0000_1000_1000_0000) >> 2);

            let t02 = self.ne as u16 & 0b0000_0110_0110_0000;

            let t10 = ((self.nw as u16 & 0b0000_0000_0000_0110) << 8)
                & ((self.sw as u16 & 0b0110_0000_0000_0000) >> 8);

            let t11 = ((self.nw as u16 & 0b0000_0000_0000_0001) << 10)
                & ((self.ne as u16 & 0b0000_0000_0000_1000) << 6)
                & ((self.sw as u16 & 0b0001_0000_0000_0000) >> 6)
                & ((self.se as u16 & 0b1000_0000_0000_0000) >> 10);

            let t12 = ((self.ne as u16 & 0b0000_0000_0000_0110) << 8)
                & ((self.se as u16 & 0b0110_0000_0000_0000) >> 8);

            let t20 = self.sw as u16 & 0b0000_0110_0110_0000;

            let t21 = ((self.sw as u16 & 0b0000_0001_0001_0000) << 2)
                & ((self.se as u16 & 0b0000_1000_1000_0000) >> 2);

            let t22 = self.se as u16 & 0b0000_0110_0110_0000;

            let tl = (t00 << 5) & (t01 << 3) & (t10 >> 1) & (t11 >> 5);
            let tr = (t01 << 5) & (t02 << 3) & (t11 >> 1) & (t12 >> 5);
            let bl = (t10 << 5) & (t11 << 3) & (t20 >> 1) & (t21 >> 5);
            let br = (t11 << 5) & (t12 << 3) & (t21 >> 1) & (t22 >> 5);

            let _ = (next[tl as usize] << 5)
                & (next[tr as usize] << 3)
                & (next[bl as usize] >> 1)
                & (next[br as usize] >> 5);
        }
        self.mask_leaf();
    }

    fn compute_node_res(&self, buf: &[Cell]) -> (CellHash, Cell) {
        //  n00 n01 n02
        //  n10 n11 n12
        //  n20 n21 n22

        // TODO: These should be indices
        let n00 = cell_utils::center(buf[self.nw], buf);
        let n01 = cell_utils::h_center(buf[self.nw], buf[self.ne], buf);
        let n02 = cell_utils::center(buf[self.ne], buf);
        let n10 = cell_utils::v_center(buf[self.nw], buf[self.sw], buf);
        let n11 = cell_utils::super_center(buf[self.nw], buf);
        let n12 = cell_utils::v_center(buf[self.ne], buf[self.se], buf);
        let n20 = cell_utils::center(buf[self.sw], buf);
        let n21 = cell_utils::h_center(buf[self.sw], buf[self.se], buf);
        let n22 = cell_utils::center(buf[self.se], buf);

        todo!()
    }

    /// Hash the cell
    pub fn hash(&self) -> CellHash {
        if self.is_leaf() {
            self.leaf_hash()
        } else {
            self.node_hash()
        }
    }

    /// Hash the cell as a node
    fn node_hash(&self) -> CellHash {
        let se = ::std::num::Wrapping(self.se);
        let sw = ::std::num::Wrapping(self.sw);
        let ne = ::std::num::Wrapping(self.ne);
        let nw = ::std::num::Wrapping(self.nw);

        let c = ::std::num::Wrapping(3);

        let h = se + c * (sw + c * (ne + c * nw + c));
        h.0
    }

    /// Hash the cell as a leaf
    fn leaf_hash(&self) -> CellHash {
        let se = ::std::num::Wrapping(self.se);
        let sw = ::std::num::Wrapping(self.sw);
        let ne = ::std::num::Wrapping(self.ne);
        let nw = ::std::num::Wrapping(self.nw);

        let c = ::std::num::Wrapping(9);

        let h = se + c * (sw + c * (ne + c * nw));
        h.0
    }
}

/// TODO: On all of these below, gracefully handle any cases where indexing into buf may resolve to
/// `Cell::unset()`.
mod cell_utils {
    use super::Cell;
    use super::RES_UNSET_MASK;

    // pub fn find_cell(nw: usize, ne: usize, sw: usize, se: usize, buf: &[Cell]) -> usize {
    //     // Houston, we have a problem.
    //     //
    //     // Hashing a cell produces its result. What we want here is to return the index of a cell
    //     // in `buf`, whose quandrants are identical those passed in.
    //
    //     todo!()
    // }

    /// Given an n-cell, returns the n/2 cell at its center
    pub fn center(c: Cell, buf: &[Cell]) -> Cell {
        Cell {
            nw: buf[c.nw].se,
            ne: buf[c.ne].sw,
            sw: buf[c.sw].ne,
            se: buf[c.se].nw,
            res: RES_UNSET_MASK,
        }
    }

    /// Given two n-cells with `w` to the left and `e` to the right, this returns the n/2 cell centered
    /// on their boundary
    pub fn h_center(w: Cell, e: Cell, buf: &[Cell]) -> Cell {
        Cell {
            nw: buf[w.ne].se,
            ne: buf[e.nw].sw,
            sw: buf[w.se].ne,
            se: buf[e.sw].nw,
            res: RES_UNSET_MASK,
        }
    }

    /// Given two n-cells with `n` above and `s` below, this returns the n/2 cell centered on their
    /// boundary
    pub fn v_center(n: Cell, s: Cell, buf: &[Cell]) -> Cell {
        Cell {
            nw: buf[n.sw].se,
            ne: buf[n.se].sw,
            sw: buf[s.nw].ne,
            se: buf[s.ne].nw,
            res: RES_UNSET_MASK,
        }
    }

    /// On an n-cell, this is its n/4 center
    pub fn super_center(cell: Cell, buf: &[Cell]) -> Cell {
        Cell {
            nw: buf[buf[cell.nw].se].se,
            ne: buf[buf[cell.ne].sw].sw,
            sw: buf[buf[cell.sw].ne].ne,
            se: buf[buf[cell.se].nw].nw,
            res: RES_UNSET_MASK,
        }
    }
}
