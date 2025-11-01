use tracing::debug;
use tracing::trace;

use crate::camera::Camera;
use crate::CellOffset;

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

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Cell {
    pub nw: CellHash,
    pub ne: CellHash,
    pub sw: CellHash,
    pub se: CellHash,
}

impl Cell {
    /// Return the canonical "empty" cell. This is the same as an `uninit` cell, but has with
    /// different semantics.
    ///
    /// NOTE: A void cell is *not* tagged with the leaf mask. Cells of any size
    /// can point to a void cell if any of their quadrants happen to be empty
    pub const fn void() -> Self {
        Self::uninit()
    }

    /// Return an unset cell
    pub const fn uninit() -> Self {
        Self {
            nw: 0,
            ne: 0,
            sw: 0,
            se: 0,
        }
    }

    /// Create a new leaf node given 4 rules
    pub const fn leaf(nw: u16, ne: u16, sw: u16, se: u16) -> Self {
        Self {
            nw: nw as usize | LEAF_MASK,
            ne: ne as usize,
            sw: sw as usize,
            se: se as usize,
        }
    }

    pub const fn leaf_uninit() -> Self {
        Self::leaf(0, 0, 0, 0)
    }

    /// Create a new node given 4 indices. We assume the node has already been inserted
    pub const fn new(nw: usize, ne: usize, sw: usize, se: usize) -> Self {
        Self { nw, ne, sw, se }
    }

    /// Grow the current cell about its center by a factor of 2
    pub fn grow(&self, buf: &mut Vec<Cell>) -> Self {
        let mask = if self.is_leaf() { LEAF_MASK } else { 0 };

        let nw = Cell {
            nw: mask,
            ne: 0,
            sw: 0,
            se: self.nw & !mask,
        };

        let ne = Cell {
            nw: mask,
            ne: 0,
            sw: self.ne,
            se: 0,
        };

        let sw = Cell {
            nw: mask,
            ne: self.sw,
            sw: 0,
            se: 0,
        };

        let se = Cell {
            nw: self.se | mask,
            ne: 0,
            sw: 0,
            se: 0,
        };

        let n = buf.len();

        buf.push(nw);
        buf.push(ne);
        buf.push(sw);
        buf.push(se);

        Cell {
            nw: n,
            ne: n + 1,
            sw: n + 2,
            se: n + 3,
        }
    }

    /// For a cell of sidelength `2^k`, this returns a cell of sidelength `2^{k - 1}`, the result
    /// after `2^{k - 2}` iterations
    pub fn next(&mut self, next: &[u16], buf: &mut Vec<Cell>) -> usize {
        self.compute_res(next, buf)
    }

    pub fn children(&self) -> Option<[usize; 4]> {
        if self.is_leaf() {
            None
        } else {
            Some([self.nw, self.ne, self.sw, self.se])
        }
    }

    /// Check if the cell is void.
    ///
    /// Note that this is different from a cell being a leaf
    pub fn is_void(&self) -> bool {
        *self == Cell::void()
    }

    /// Check if the cell is a leaf.
    ///
    /// Leaves are 8 cells
    pub fn is_leaf(&self) -> bool {
        self.nw & LEAF_MASK == LEAF_MASK
    }

    /// Check if the cell is a 16 cell (i.e. a cell composed only of leaves)
    pub fn is_16(&self, buf: &[Cell]) -> bool {
        !self.is_leaf()
            && (buf[self.nw].is_leaf()
                || buf[self.ne].is_leaf()
                || buf[self.sw].is_leaf()
                || buf[self.se].is_leaf())
    }

    // WARNING: A leaf check would fail after this. It's
    // important to remask the leaf as early as possible.
    fn unmask_leaf(&mut self) {
        assert!(self.is_leaf());

        self.nw &= !LEAF_MASK;
    }

    fn mask_leaf(&mut self) {
        // We mask leaves so that we have a way to differentiate between non-leaf cells and leaf
        // cells
        self.nw &= LEAF_MASK;
    }

    /// Compute the result of a cell
    ///
    /// The `usize` returned is either an index or a rule.
    ///
    /// A rule is just returned as a usize, but a cell is inserted into the buf and its index is
    /// returned
    fn compute_res(&mut self, next: &[u16], buf: &mut Vec<Cell>) -> usize {
        trace!("Compute res quadrants");
        trace!("nw: {}", self.nw);
        trace!("ne: {}", self.ne);
        trace!("sw: {}", self.sw);
        trace!("se: {}", self.se);

        if self.is_void() {
            0
        } else if self.is_leaf() {
            debug!("Computing leaf res");
            debug_draw(*self, buf, 0);

            // NOTE: We only get here if called from `next`
            self.compute_leaf_res(next) as usize
        } else if self.is_16(buf) {
            debug!("Computing 16 cell res");
            debug_draw(*self, buf, 1);

            let cell = self.compute_node_res16(next, buf);

            let n = buf.len();
            buf.push(cell);

            n
        } else {
            debug!("Computing node res");

            let cell = self.compute_node_res(next, buf); //

            let n = buf.len();
            buf.push(cell);

            n
        }
    }

    /// For a leaf cell, this computes its result.
    /// Remember that a leaf cell is composed entirely of u16s, each 4 squares on a side. This
    /// makes leaves 8 cells, and their result 4 cells.
    ///
    /// Here, `next` is a ruleset array, where `next[rule] = result(rule)`
    #[rustfmt::skip]
    fn compute_leaf_res(&mut self, next: &[u16]) -> u16 {
        assert!(self.is_leaf());

        let rule;

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

            let t22 =   self.se as u16 & 0b0000_0110_0110_0000;

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

            // t00 t01 t02
            // t10 t11 t12
            // t20 t21 t22
            let tl = (t00 << 5) | (t01 << 3) | (t10 >> 3) | (t11 >> 5);
            let tr = (t01 << 5) | (t02 << 3) | (t11 >> 3) | (t12 >> 5);
            let bl = (t10 << 5) | (t11 << 3) | (t20 >> 3) | (t21 >> 5);
            let br = (t11 << 5) | (t12 << 3) | (t21 >> 3) | (t22 >> 5);

            trace!("tl:  {tl:016b}");
            trace!("tr:  {tr:016b}");
            trace!("bl:  {bl:016b}");
            trace!("br:  {br:016b}");

            rule = (next[tl as usize] << 5)
                 | (next[tr as usize] << 3)
                 | (next[bl as usize] >> 3)
                 | (next[br as usize] >> 5);

            trace!("res: {rule:016b}");
        }
        self.mask_leaf();

        rule
    }

    /// Computes the result of a 16 cell
    /// Returns an 8 cell
    #[rustfmt::skip]
    fn compute_node_res16(&self, next: &[u16], buf: &mut Vec<Cell>) -> Cell {
        // these are leaves
        let mut nw = buf[self.nw];
        let mut ne = buf[self.ne];
        let mut sw = buf[self.sw];
        let mut se = buf[self.se];

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

        // NOTE: This downcast is safe. The only way down from here is either void or leaf
        // All of these are rules
        let n00 = nw.compute_res(next, buf) as u16;
        let n01 =  n.compute_res(next, buf) as u16;
        let n02 = ne.compute_res(next, buf) as u16;
        let n10 =  w.compute_res(next, buf) as u16;
        let n11 =  c.compute_res(next, buf) as u16;
        let n12 =  e.compute_res(next, buf) as u16;
        let n20 = sw.compute_res(next, buf) as u16;
        let n21 =  s.compute_res(next, buf) as u16;
        let n22 = se.compute_res(next, buf) as u16;

        // n00 n01 n02
        // n10 n11 n12
        // n20 n21 n22
        let mut tl = Cell::leaf(n00, n01, n10, n11);
        let mut tr = Cell::leaf(n01, n02, n11, n12);
        let mut bl = Cell::leaf(n10, n11, n20, n21);
        let mut br = Cell::leaf(n11, n12, n21, n22);

        // NOTE: This downcast is safe for the same reason as the one above
        let tl_res = tl.compute_res(next, buf) as u16;
        let tr_res = tr.compute_res(next, buf) as u16;
        let bl_res = bl.compute_res(next, buf) as u16;
        let br_res = br.compute_res(next, buf) as u16;

        Cell::leaf(tl_res, tr_res, bl_res, br_res)
    }

    /// Computes the result of a 2^k cell for k > 4 (i.e. at least 32 cells)
    #[rustfmt::skip]
    fn compute_node_res(&mut self, next: &[u16], buf: &mut Vec<Cell>) -> Cell {
        // at least 16 cells
        let mut nw = buf[self.nw];
        let mut ne = buf[self.ne];
        let mut sw = buf[self.sw];
        let mut se = buf[self.se];

        debug!("nw:");
        debug_draw(nw, buf, 1);
        trace!("nw: {:?}", nw);

        debug!("ne:");
        debug_draw(ne, buf, 1);
        trace!("ne: {:?}", ne);

        debug!("sw:");
        debug_draw(sw, buf, 1);
        trace!("sw: {:?}", sw);

        debug!("se:");
        debug_draw(se, buf, 1);
        trace!("se: {:?}", se);

        // cardinal pseudo-cells
        let mut n = cell_utils::h_center(nw, ne);
        let mut s = cell_utils::h_center(sw, se);
        let mut e = cell_utils::v_center(ne, se);
        let mut w = cell_utils::v_center(nw, sw);

        // center n/2 cell of n cell
        let mut c = cell_utils::center(*self, buf);

        // All of these are cells
        let n00 = nw.compute_res(next, buf);
        debug!("n00");
        debug_draw(buf[n00], buf, 1);

        let n01 =  n.compute_res(next, buf);
        debug!("n01");
        debug_draw(buf[n01], buf, 1);

        let n02 = ne.compute_res(next, buf);
        debug!("n02");
        debug_draw(buf[n02], buf, 1);

        let n10 =  w.compute_res(next, buf);
        debug!("n10");
        debug_draw(buf[n10], buf, 1);

        let n11 =  c.compute_res(next, buf);
        debug!("n11");
        debug_draw(buf[n11], buf, 1);

        let n12 =  e.compute_res(next, buf);
        debug!("n12");
        debug_draw(buf[n12], buf, 1);

        let n20 = sw.compute_res(next, buf);
        debug!("n20");
        debug_draw(buf[n20], buf, 1);

        let n21 =  s.compute_res(next, buf);
        debug!("n21");
        debug_draw(buf[n21], buf, 1);

        let n22 = se.compute_res(next, buf);
        debug!("n22");
        debug_draw(buf[n22], buf, 1);

        // n00 n01 n02
        // n10 n11 n12
        // n20 n21 n22
        let mut tl = Cell::new(n00, n01, n10, n11);
        let mut tr = Cell::new(n01, n02, n11, n12);
        let mut bl = Cell::new(n10, n11, n20, n21);
        let mut br = Cell::new(n11, n12, n21, n22);

        debug!("tl:");
        debug_draw(tl, buf, 1);

        debug!("tr:");
        debug_draw(tr, buf, 1);

        debug!("bl:");
        debug_draw(bl, buf, 1);

        debug!("br:");
        debug_draw(br, buf, 1);

        let nw = tl.compute_res(next, buf);
        let ne = tr.compute_res(next, buf);
        let sw = bl.compute_res(next, buf);
        let se = br.compute_res(next, buf);

        debug!("tl res:");
        debug_draw(buf[nw], buf, 0);

        debug!("tr res:");
        debug_draw(buf[ne], buf, 0);

        debug!("bl res:");
        debug_draw(buf[sw], buf, 0);

        debug!("br res:");
        debug_draw(buf[se], buf, 0);

        let res = Cell {
            nw,
            ne,
            sw,
            se,
        };

        debug!("Final res:");
        debug_draw(res, buf, 1);

        res
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

impl std::fmt::Debug for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_leaf() {
            // Unmask leaf
            // NOTE: We don't use `.unmask_leaf` because it takes `&mut self`. Frankly it should
            // just return a copy of the leaf, that whole system is stupid.
            let nw = (self.nw & !LEAF_MASK) as u16;

            f.debug_struct("Leaf")
                .field("nw", &format!("{:b}", nw))
                .field("ne", &format!("{:b}", self.ne))
                .field("sw", &format!("{:b}", self.sw))
                .field("se", &format!("{:b}", self.se))
                .finish()
        } else {
            f.debug_struct("Cell")
                .field("nw", &self.nw)
                .field("ne", &self.ne)
                .field("sw", &self.sw)
                .field("se", &self.se)
                .finish()
        }
    }
}

mod cell_utils {
    use crate::cell::Cell;
    use crate::cell::LEAF_MASK;

    use tracing::trace;

    /// Takes as input a rule return a `Cell` with that rule about its center
    pub fn rule_to_leaf(rule: u16) -> Cell {
        let nw = (rule & 0b1100_1100_0000_0000) >> 10;
        let ne = (rule & 0b0011_0011_0000_0000) >> 6;
        let sw = (rule & 0b0000_0000_1100_1100) << 6;
        let se = (rule & 0b0000_0000_0011_0011) << 10;

        Cell::leaf(nw, ne, sw, se)
    }

    /// Given two cells `w` and `e`, returns the cell at their center.
    pub fn h_center(w: Cell, e: Cell) -> Cell {
        Cell {
            nw: w.ne,
            ne: e.nw,
            sw: w.se,
            se: e.sw,
        }
    }

    /// Given two 16-cells `n` and `s`, returns the cell at their center.
    pub fn v_center(n: Cell, s: Cell) -> Cell {
        Cell {
            nw: n.sw,
            ne: n.se,
            sw: s.nw,
            se: s.ne,
        }
    }

    /// Given an n-cell, returns the n/2 cell at its center
    /// NOTE: Must be at least a 16 cell
    pub fn center(c: Cell, buf: &[Cell]) -> Cell {
        Cell {
            nw: buf[c.nw].se,
            ne: buf[c.ne].sw,
            sw: buf[c.sw].ne,
            se: buf[c.se].nw,
        }
    }

    /// Given two 8 cells `w` and `e`, returns the leaf at their center.
    pub fn h_center8(w: Cell, e: Cell) -> Cell {
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
    pub fn v_center8(n: Cell, s: Cell) -> Cell {
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
    pub fn center16(cell: Cell, buf: &[Cell]) -> Cell {
        assert!(cell.is_16(buf));

        // leaves (i.e. 8 cells)
        let nw = buf[cell.nw];
        let ne = buf[cell.ne];
        let sw = buf[cell.sw];
        let se = buf[cell.se];

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

/// Draws a 4 cell
fn draw_rule(cam: &mut Camera, rule: u16, dx: CellOffset, dy: CellOffset) {
    let mut mask = 1 << 0xF;

    let (mut x, mut y) = (0, 0);
    while mask > 0 {
        if rule & mask == mask {
            cam.draw_pixel(x + dx, y + dy);
        }

        x = (x + 1) % 4;

        if x == 0 {
            y += 1;
        }

        mask >>= 1;
    }
}

/// Draws an 8 cell
fn draw_leaf(cam: &mut Camera, mut cell: Cell, dx: CellOffset, dy: CellOffset) {
    assert!(cell.is_leaf());

    cell.unmask_leaf();
    {
        let Cell { nw, ne, sw, se, .. } = cell;

        draw_rule(cam, nw as u16, dx, dy);
        draw_rule(cam, ne as u16, dx + 4, dy);
        draw_rule(cam, sw as u16, dx, dy + 4);
        draw_rule(cam, se as u16, dx + 4, dy + 4);
    }
    cell.mask_leaf();
}

/// Draws a 2^k cell for k > 3
fn draw_cell(
    cam: &mut Camera,
    cell: Cell,
    cells: &[Cell],
    depth: u8,
    dx: CellOffset,
    dy: CellOffset,
) {
    if cell.is_leaf() {
        draw_leaf(cam, cell, dx, dy);
    } else {
        assert!(depth > 0, "Expected non-zero depth for non-leaf node");

        let Cell { nw, ne, sw, se, .. } = cell;

        let d = 2usize.pow(2 + depth as u32) as CellOffset;

        draw_cell(cam, cells[nw], cells, depth - 1, dx, dy);
        draw_cell(cam, cells[ne], cells, depth - 1, dx + d, dy);
        draw_cell(cam, cells[sw], cells, depth - 1, dx, dy + d);
        draw_cell(cam, cells[se], cells, depth - 1, dx + d, dy + d);
    }
}

fn debug_draw(cell: Cell, cells: &[Cell], depth: u8) {
    // if enabled!(Level::DEBUG) {
    //     let sl = 2usize.pow(depth as u32 + 3);
    //     let mut cam = Camera::new(sl, sl);
    //
    //     draw_cell(&mut cam, cell, cells, depth, 0, 0);
    //
    //     let s = cam.render();
    //
    //     debug!("\n{s}");
    // }
}
