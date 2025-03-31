use tracing::trace;

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
pub const RES_UNSET_MASK: usize = LEAF_MASK;

/// A `CellHash` is either an index into a list of `Cell`s, or 4 cell stored directly as a u16
pub type CellHash = usize;

#[derive(PartialEq, Debug, Eq, Clone, Copy)]
pub struct Cell {
    pub nw: CellHash,
    pub ne: CellHash,
    pub sw: CellHash,
    pub se: CellHash,
}

impl Cell {
    /// Return the canonical "empty" cell
    pub const fn void() -> Self {
        Self {
            nw: LEAF_MASK,
            ne: 0,
            sw: 0,
            se: 0,
        }
    }

    /// Return an unset cell
    pub const fn unset() -> Self {
        Self {
            nw: 0,
            ne: 0,
            sw: 0,
            se: 0,
        }
    }

    /// For a cell of sidelength `2^k`, this returns a cell of sidelength `2^{k - 1}`, the result
    /// after `2^{k - 2}` iterations
    pub fn next(&mut self, next: &[u16], buf: &[Cell]) -> Cell {
        self.compute_res(next, buf)
    }

    // WARNING: A leaf check would fail after this. It's
    // important to remask the leaf as early as possible.
    pub fn unmask_leaf(&mut self) {
        assert!(self.is_leaf());

        self.nw ^= LEAF_MASK;
    }

    pub fn mask_leaf(&mut self) {
        // We mask leaves so that we have a way to differentiate between non-leaf cells and leaf
        // cells
        self.nw &= LEAF_MASK;
    }

    /// A cell is a parent if it is not a leaf.
    fn is_parent(&self) -> bool {
        !self.is_leaf()
    }

    /// A cell is a grandparent if all of its children are parents
    fn is_grandparent(&self, buf: &[Cell]) -> bool {
        self.is_parent()
            && buf[self.nw].is_parent()
            && buf[self.ne].is_parent()
            && buf[self.sw].is_parent()
            && buf[self.se].is_parent()
    }

    /// Check if the cell is a leaf.
    ///
    /// Leaves are 8 cells
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
    fn compute_res(&mut self, next: &[u16], buf: &[Cell]) -> Cell {
        if self.is_leaf() {
            self.compute_leaf_res(next);

            todo!()
        } else {
            self.compute_node_res16(buf, next)
        }

        // TODO: Add general `compute_node_res` code here
    }

    /// For a leaf cell, this computes its result.
    ///
    ///   t00 t01 t02
    ///   t10 t11 t12
    ///   t20 t21 t22
    ///
    /// Remember that a leaf cell is composed entirely of u16s, each 4 squares on a side. This
    /// makes leaves 8 cells, and their result 4 cells.
    ///
    /// Here, `next` is a ruleset array, where `next[rule] = result(rule)`
    #[rustfmt::skip]
    fn compute_leaf_res(&mut self, next: &[u16]) -> u16 {
        assert!(self.is_leaf());

        let res;

        self.unmask_leaf();
        {
            let t00 =   self.nw as u16 & 0b0000_0110_0110_0000;

            let t01 = ((self.nw as u16 & 0b0000_0001_0001_0000) << 2)
                    | ((self.ne as u16 & 0b0000_1000_1000_0000) >> 2);

            let t02 =   self.ne as u16 & 0b0000_0110_0110_0000;

            let t10 = ((self.nw as u16 & 0b0000_0000_0000_0110) << 8)
                    | ((self.sw as u16 & 0b0110_0000_0000_0000) >> 8);

            let t11 = ((self.nw as u16 & 0b0000_0000_0000_0001) << 10)
                    | ((self.ne as u16 & 0b0000_0000_0000_1000) << 6)
                    | ((self.sw as u16 & 0b0001_0000_0000_0000) >> 6)
                    | ((self.se as u16 & 0b1000_0000_0000_0000) >> 10);

            let t12 = ((self.ne as u16 & 0b0000_0000_0000_0110) << 8)
                    | ((self.se as u16 & 0b0110_0000_0000_0000) >> 8);

            let t20 =   self.sw as u16 & 0b0000_0110_0110_0000;

            let t21 = ((self.sw as u16 & 0b0000_0001_0001_0000) << 2)
                    | ((self.se as u16 & 0b0000_1000_1000_0000) >> 2);

            let t22 = self.se as u16 & 0b0000_0110_0110_0000;

            trace!("nw:  {:016b}", self.nw);
            trace!("ne:  {:016b}", self.ne);
            trace!("sw:  {:016b}", self.sw);
            trace!("se:  {:016b}", self.se);

            trace!("t00: {t00:016b}");
            trace!("t01: {t01:016b}");
            trace!("t02: {t02:016b}");
            trace!("t10: {t10:016b}");
            trace!("t11: {t11:016b}");
            trace!("t12: {t12:016b}");
            trace!("t20: {t20:016b}");
            trace!("t21: {t21:016b}");
            trace!("t22: {t22:016b}");

            let tl = (t00 << 5) | (t01 << 3) | (t10 >> 3) | (t11 >> 5);
            let tr = (t01 << 5) | (t02 << 3) | (t11 >> 3) | (t12 >> 5);
            let bl = (t10 << 5) | (t11 << 3) | (t20 >> 3) | (t21 >> 5);
            let br = (t11 << 5) | (t12 << 3) | (t21 >> 3) | (t22 >> 5);

            trace!("tl:  {tl:016b}");
            trace!("tr:  {tr:016b}");
            trace!("bl:  {bl:016b}");
            trace!("br:  {br:016b}");

            res = (next[tl as usize] << 5)
                | (next[tr as usize] << 3)
                | (next[bl as usize] >> 3)
                | (next[br as usize] >> 5);

            trace!("res: {res:016b}");
        }
        self.mask_leaf();

        res
    }

    /// Computes the result of a 16 cell
    ///
    ///     n00 n01 n02
    ///     n10 n11 n12
    ///     n20 n21 n22
    ///
    /// Returns an 8 cell
    fn compute_node_res16(&self, buf: &[Cell], next: &[u16]) -> Cell {
        trace!("nw: {}", self.nw);
        trace!("ne: {}", self.ne);
        trace!("sw: {}", self.sw);
        trace!("se: {}", self.se);

        // these are leaves
        let mut nw = buf[self.nw];
        let mut ne = buf[self.ne];
        let mut sw = buf[self.sw];
        let mut se = buf[self.se];

        assert!(nw.is_leaf());
        assert!(ne.is_leaf());
        assert!(sw.is_leaf());
        assert!(se.is_leaf());

        trace!("nw: {:?}", nw);
        trace!("ne: {:?}", ne);
        trace!("sw: {:?}", sw);
        trace!("se: {:?}", se);

        // cardinal pseudo-leaves
        let mut n = cell_utils::h_center8(nw, ne);
        let mut s = cell_utils::h_center8(sw, se);
        let mut e = cell_utils::v_center8(ne, se);
        let mut w = cell_utils::v_center8(nw, sw);

        // center 8 leaf of 16 cell
        let mut c = cell_utils::center16(*self, buf);

        // All of these are rules
        let n00 = nw.compute_leaf_res(next);
        let n01 = n.compute_leaf_res(next);
        let n02 = ne.compute_leaf_res(next);
        let n10 = w.compute_leaf_res(next);
        let n11 = c.compute_leaf_res(next);
        let n12 = e.compute_leaf_res(next);
        let n20 = sw.compute_leaf_res(next);
        let n21 = s.compute_leaf_res(next);
        let n22 = se.compute_leaf_res(next);

        let mut tl = Cell {
            nw: (n00 as usize) | LEAF_MASK,
            ne: n01 as usize,
            sw: n10 as usize,
            se: n11 as usize,
        };

        let mut tr = Cell {
            nw: (n01 as usize) | LEAF_MASK,
            ne: n02 as usize,
            sw: n11 as usize,
            se: n12 as usize,
        };

        let mut bl = Cell {
            nw: (n10 as usize) | LEAF_MASK,
            ne: n11 as usize,
            sw: n20 as usize,
            se: n21 as usize,
        };

        let mut br = Cell {
            nw: (n11 as usize) | LEAF_MASK,
            ne: n12 as usize,
            sw: n21 as usize,
            se: n22 as usize,
        };

        // Since the 4 cells above are 8x8 (i.e. leaves), these are rules
        let tl_res = tl.compute_leaf_res(next);
        let tr_res = tr.compute_leaf_res(next);
        let bl_res = bl.compute_leaf_res(next);
        let br_res = br.compute_leaf_res(next);

        Cell {
            nw: tl_res as usize | LEAF_MASK,
            ne: tr_res as usize,
            sw: bl_res as usize,
            se: br_res as usize,
        }
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
pub mod cell_utils {
    use crate::cell::LEAF_MASK;

    use super::Cell;
    use tracing::trace;

    /// Given an n-cell, returns the n/2 cell at its center
    pub fn center(c: Cell, buf: &[Cell]) -> Cell {
        Cell {
            nw: buf[c.nw].se,
            ne: buf[c.ne].sw,
            sw: buf[c.sw].ne,
            se: buf[c.se].nw,
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
        }
    }

    /// On an n > 16 cell, this is its n/4 center
    pub fn super_center(cell: Cell, buf: &[Cell]) -> Cell {
        Cell {
            nw: buf[buf[cell.nw].se].se,
            ne: buf[buf[cell.ne].sw].sw,
            sw: buf[buf[cell.sw].ne].ne,
            se: buf[buf[cell.se].nw].nw,
        }
    }

    /// Given two 8 cells `w` and `e`, returns the leaf at their center.
    /// Visually, if `w` is `-` and `e` is `+`, returns the area shaded `#`
    ///
    ///     ----########++++
    ///     ----########++++
    ///     ----########++++
    ///     ----########++++
    ///     ----########++++
    ///     ----########++++
    ///     ----########++++
    ///     ----########++++
    pub fn h_center8(w: Cell, e: Cell) -> Cell {
        assert!(w.is_leaf());
        assert!(e.is_leaf());

        trace!("w: {w:?}");
        trace!("e: {e:?}");

        let nw = w.ne as u16;
        let ne = (e.nw & !LEAF_MASK) as u16;
        let sw = w.se as u16;
        let se = e.sw as u16;

        Cell {
            nw: nw as usize | LEAF_MASK,
            ne: ne as usize,
            sw: sw as usize,
            se: se as usize,
        }
    }

    /// Given two 8 cells `n` and `s`, returns the leaf at their center.
    /// Visually, if `n` is `-` and `s` is `+`, returns the area shaded `#`
    ///
    ///     --------
    ///     --------
    ///     --------
    ///     --------
    ///     ########
    ///     ########
    ///     ########
    ///     ########
    ///     ########
    ///     ########
    ///     ########
    ///     ########
    ///     ++++++++
    ///     ++++++++
    ///     ++++++++
    ///     ++++++++
    pub fn v_center8(n: Cell, s: Cell) -> Cell {
        assert!(n.is_leaf());
        assert!(s.is_leaf());

        trace!("n: {n:?}");
        trace!("s: {s:?}");

        let nw = n.sw as u16;
        let ne = n.se as u16;
        let sw = (s.nw & !LEAF_MASK) as u16;
        let se = s.ne as u16;

        trace!("nw: {nw:016b}");
        trace!("ne: {ne:016b}");
        trace!("sw: {sw:016b}");
        trace!("se: {se:016b}");

        Cell {
            nw: nw as usize | LEAF_MASK,
            ne: ne as usize,
            sw: sw as usize,
            se: se as usize,
        }
    }

    /// On a 16 cell, this is its 8x8 center leaf
    /// Visually, returns the shaded area
    ///
    ///     ----------------
    ///     ----------------
    ///     ----------------
    ///     ----------------
    ///     ----########----
    ///     ----########----
    ///     ----########----
    ///     ----########----
    ///     ----########----
    ///     ----########----
    ///     ----########----
    ///     ----########----
    ///     ----------------
    ///     ----------------
    ///     ----------------
    ///     ----------------
    pub fn center16(cell: Cell, buf: &[Cell]) -> Cell {
        assert!(cell.is_parent());

        // leaves (i.e. 8 cells)
        let nw = buf[cell.nw];
        let ne = buf[cell.ne];
        let sw = buf[cell.sw];
        let se = buf[cell.se];

        assert!(nw.is_leaf());
        assert!(ne.is_leaf());
        assert!(sw.is_leaf());
        assert!(se.is_leaf());

        trace!("nw: {nw:?}");
        trace!("ne: {ne:?}");
        trace!("sw: {sw:?}");
        trace!("se: {se:?}");

        // These are rules, since the cell is not a grandparent
        let nw = nw.se as u16;
        let ne = ne.sw as u16;
        let sw = sw.ne as u16;
        let se = (se.nw & !LEAF_MASK) as u16;

        trace!("nw: {nw:016b}");
        trace!("ne: {ne:016b}");
        trace!("sw: {sw:016b}");
        trace!("se: {se:016b}");

        Cell {
            nw: nw as usize | LEAF_MASK,
            ne: ne as usize,
            sw: sw as usize,
            se: se as usize,
        }
    }
}
