#[cfg(test)]
static COMPUTE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub fn reset_compute_count() {
    COMPUTE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
pub fn get_compute_count() -> usize {
    COMPUTE_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn bump_compute_count() {
    #[cfg(test)]
    COMPUTE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

const WORD_SIZE_BITS: usize = std::mem::size_of::<usize>() * 8;

/// MSB of `nw`: marks the cell as a leaf (8x8 grid with u16 rules in each quadrant).
///
/// We assume the memory buffer will never contain 2^63 (or 2^31) entries,
/// so the most significant bit is free to use as a tag.
pub const LEAF_MASK: usize = 1usize << (WORD_SIZE_BITS - 1);

/// MSB of `res`: marks the cached result as not yet computed.
pub const RES_UNSET_MASK: usize = LEAF_MASK;

/// Second-highest bit of `res`: marks a cell as void (contains no live cells).
/// A leaf is void when all 4 rules are zero. A node is void when all 4 children are void.
pub const VOID_MASK: usize = 1usize << (WORD_SIZE_BITS - 2);

/// Sentinel value for "end of hash chain" — can never be a valid buf index
pub const HASH_CHAIN_END: usize = usize::MAX;

/// Sentinel for "cell reset by GC but not yet visited/reinserted"
pub const GC_UNREACHABLE: usize = usize::MAX - 1;

/// A `CellHash` is either an index into a list of `Cell`s, or 4 cell stored directly as a u16
pub type CellHash = usize;

#[derive(Clone, Copy)]
pub struct Cell {
    pub nw: CellHash,
    pub ne: CellHash,
    pub sw: CellHash,
    pub se: CellHash,

    /// Cached result index (RES_UNSET_MASK = not computed)
    pub res: CellHash,

    /// Hash chain link (HASH_CHAIN_END = end of chain)
    pub next_hash: CellHash,
}

impl Cell {
    /// Return the canonical "empty" cell (the base void node at buf index 0).
    pub const fn void() -> Self {
        Self {
            nw: 0,
            ne: 0,
            sw: 0,
            se: 0,
            res: RES_UNSET_MASK | VOID_MASK,
            next_hash: HASH_CHAIN_END,
        }
    }

    /// Create a new leaf node given 4 rules
    pub const fn leaf(nw: u16, ne: u16, sw: u16, se: u16) -> Self {
        let void_bit = if nw == 0 && ne == 0 && sw == 0 && se == 0 {
            VOID_MASK
        } else {
            0
        };
        Self {
            nw: nw as usize | LEAF_MASK,
            ne: ne as usize,
            sw: sw as usize,
            se: se as usize,
            res: RES_UNSET_MASK | void_bit,
            next_hash: HASH_CHAIN_END,
        }
    }

    pub const fn leaf_uninit() -> Self {
        Self::leaf(0, 0, 0, 0)
    }

    /// Create a new node given 4 indices. We assume the node has already been inserted
    pub const fn new(nw: usize, ne: usize, sw: usize, se: usize) -> Self {
        Self {
            nw,
            ne,
            sw,
            se,
            res: RES_UNSET_MASK,
            next_hash: HASH_CHAIN_END,
        }
    }

    pub fn children(&self) -> Option<[usize; 4]> {
        if self.is_leaf() {
            None
        } else {
            Some([self.nw, self.ne, self.sw, self.se])
        }
    }

    /// Check if the cell is void (contains no live cells).
    /// This works at any depth — leaves, nodes, and the base void cell.
    pub fn is_void(&self) -> bool {
        self.res & VOID_MASK != 0
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
    pub fn unmask_leaf(&mut self) {
        assert!(self.is_leaf());

        self.nw &= !LEAF_MASK;
    }

    pub fn mask_leaf(&mut self) {
        // We mask leaves so that we have a way to differentiate between non-leaf cells and leaf
        // cells
        self.nw &= LEAF_MASK;
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

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.nw == other.nw && self.ne == other.ne && self.sw == other.sw && self.se == other.se
    }
}

impl Eq for Cell {}

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

pub mod cell_utils {
    use crate::cell::Cell;
    use crate::cell::LEAF_MASK;

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
        Cell::new(w.ne, e.nw, w.se, e.sw)
    }

    /// Given two 16-cells `n` and `s`, returns the cell at their center.
    pub fn v_center(n: Cell, s: Cell) -> Cell {
        Cell::new(n.sw, n.se, s.nw, s.ne)
    }

    /// Given an n-cell, returns the n/2 cell at its center
    /// NOTE: Must be at least a 16 cell
    pub fn center(c: Cell, buf: &[Cell]) -> Cell {
        Cell::new(buf[c.nw].se, buf[c.ne].sw, buf[c.sw].ne, buf[c.se].nw)
    }

    /// Given two 8 cells `w` and `e`, returns the leaf at their center.
    pub fn h_center8(w: Cell, e: Cell) -> Cell {
        Cell::leaf(
            w.ne as u16,
            (e.nw & !LEAF_MASK) as u16,
            w.se as u16,
            e.sw as u16,
        )
    }

    /// Given two 8 cells `n` and `s`, returns the leaf at their center.
    pub fn v_center8(n: Cell, s: Cell) -> Cell {
        Cell::leaf(
            n.sw as u16,
            n.se as u16,
            (s.nw & !LEAF_MASK) as u16,
            s.ne as u16,
        )
    }

    /// On a 16 cell, this is its 8x8 center leaf
    pub fn center16(cell: Cell, buf: &[Cell]) -> Cell {
        assert!(cell.is_16(buf));

        Cell::leaf(
            buf[cell.nw].se as u16,
            buf[cell.ne].sw as u16,
            buf[cell.sw].ne as u16,
            (buf[cell.se].nw & !LEAF_MASK) as u16,
        )
    }
}

#[cfg(test)]
mod test_next {
    use crate::camera::Camera;
    use crate::cell::Cell;
    use crate::rule_set::B3S23;
    use crate::world::World;
    use crate::CellOffset;

    /// Draws a 4 cell
    fn draw_rule(cam: &mut impl Camera, rule: u16, dx: CellOffset, dy: CellOffset) {
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
    fn draw_leaf(cam: &mut impl Camera, mut cell: Cell, dx: CellOffset, dy: CellOffset) {
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
        cam: &mut impl Camera,
        cell: Cell,
        cells: &[Cell],
        depth: u8,
        dx: CellOffset,
        dy: CellOffset,
    ) {
        if cell.is_void() {
            /* No live cells */
        } else if cell.is_leaf() {
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

    #[test]
    fn test_glider_leaf() {
        // Glider in top-left of 8x8 leaf:
        // . . . . | . . . .
        // . . # . | . . . .
        // . . . # | . . . .
        // . # # # | . . . .
        // --------|--------
        // . . . . | . . . .
        // . . . . | . . . .
        // . . . . | . . . .
        // . . . . | . . . .
        let nw: u16 = 0b0000_0010_0001_0111;
        let leaf = Cell::leaf(nw, 0, 0, 0);

        let mut buf = vec![Cell::void()];
        let leaf_idx = buf.len();
        buf.push(leaf);
        let mut world = World::from_parts(B3S23, buf, Cell::new(leaf_idx, 0, 0, 0), 4);

        // After canonicalization, find the glider leaf via the root's nw child
        let leaf_idx = world.buf[world.root].nw;
        let result = world.compute_leaf(leaf_idx);

        // After 1 step, center 4x4 (rows 2-5, cols 2-5):
        // . # . .
        // # # . .
        // # . . .
        // . . . .
        let expected: u16 = 0b0100_1100_1000_0000;

        assert_eq!(
            result, expected,
            "\nExpected: {:016b}\n     Got: {:016b}",
            expected, result
        );
    }

    #[test]
    #[rustfmt::skip]
    fn test_glider_16cell() {
        let mut buf = vec![Cell::void()];

        // Glider near the center of a 16 cell
        let nw_leaf = Cell::leaf(0, 0, 0, 0b0010_0001_0111_0000);
        let empty_leaf = Cell::leaf(0, 0, 0, 0);

        let nw_idx = buf.len(); buf.push(nw_leaf);
        let ne_idx = buf.len(); buf.push(empty_leaf);
        let sw_idx = buf.len(); buf.push(empty_leaf);
        let se_idx = buf.len(); buf.push(empty_leaf);

        let cell16 = Cell::new(nw_idx, ne_idx, sw_idx, se_idx);
        let mut world = World::from_parts(B3S23, buf, cell16, 4);

        world.set_k(0);
        let result_idx = world.compute(world.root, 4);
        let result = world.buf[result_idx];

        let expected = Cell::leaf(0b0000_0001_0101_0011, 0, 0, 0);
        assert_eq!(
            result, expected,
            "\nExpected: {:?}\n     Got: {:?}",
            expected, result
        );
    }

    #[test]
    #[rustfmt::skip]
    fn test_glider_16cell_half() {
        let mut buf = vec![Cell::void()];

        let nw_leaf = Cell::leaf(0, 0, 0, 0b0010_0001_0111_0000);
        let empty_leaf = Cell::leaf(0, 0, 0, 0);

        let nw_idx = buf.len(); buf.push(nw_leaf);
        let ne_idx = buf.len(); buf.push(empty_leaf);
        let sw_idx = buf.len(); buf.push(empty_leaf);
        let se_idx = buf.len(); buf.push(empty_leaf);

        let cell16 = Cell::new(nw_idx, ne_idx, sw_idx, se_idx);

        // Full
        let mut world_full = World::from_parts(B3S23, buf.clone(), cell16, 4);
        world_full.set_k(0);
        let full_idx = world_full.compute(world_full.root, 4);
        let full_result = world_full.buf[full_idx];

        // Half
        let mut world_half = World::from_parts(B3S23, buf, cell16, 4);
        world_half.set_k(1);
        let half_idx = world_half.compute(world_half.root, 4);
        let result = world_half.buf[half_idx];

        // Visualize
        use crate::camera::CameraBlock;

        let mut cam = CameraBlock::new(16, 8);
        draw_cell(&mut cam, world_half.buf[world_half.root], &world_half.buf, 1, 0, 0);
        cam.invert();
        eprintln!("Input 16-cell (16x16):\n{}", cam.render());

        let mut cam = CameraBlock::new(8, 4);
        draw_leaf(&mut cam, result, 0, 0);
        cam.invert();
        eprintln!("half result (8x8, 1 iter):\n{}", cam.render());

        let mut cam = CameraBlock::new(8, 4);
        draw_leaf(&mut cam, full_result, 0, 0);
        cam.invert();
        eprintln!("full result (8x8, 2 iter):\n{}", cam.render());

        let expected = Cell::leaf(0b0000_0101_0011_0010, 0, 0, 0);
        assert_eq!(
            result, expected,
            "\nExpected: {:?}\n     Got: {:?}",
            expected, result
        );
    }

    #[test]
    #[rustfmt::skip]
    fn test_glider_32cell_half() {
        let mut buf = vec![Cell::void()];

        let empty_leaf = Cell::leaf(0, 0, 0, 0);
        let glider_leaf = Cell::leaf(0, 0, 0, 0b0010_0001_0111_0000);

        let e1_idx = buf.len(); buf.push(empty_leaf);
        let e2_idx = buf.len(); buf.push(empty_leaf);
        let e3_idx = buf.len(); buf.push(empty_leaf);
        let gl_idx = buf.len(); buf.push(glider_leaf);
        let nw16 = Cell::new(e1_idx, e2_idx, e3_idx, gl_idx);
        let nw16_idx = buf.len(); buf.push(nw16);

        let e4_idx = buf.len(); buf.push(empty_leaf);
        let e5_idx = buf.len(); buf.push(empty_leaf);
        let e6_idx = buf.len(); buf.push(empty_leaf);
        let e7_idx = buf.len(); buf.push(empty_leaf);
        let empty16 = Cell::new(e4_idx, e5_idx, e6_idx, e7_idx);
        let ne16_idx = buf.len(); buf.push(empty16);
        let sw16_idx = buf.len(); buf.push(empty16);
        let se16_idx = buf.len(); buf.push(empty16);

        let cell32 = Cell::new(nw16_idx, ne16_idx, sw16_idx, se16_idx);

        use crate::camera::CameraBlock;

        // Input visualization
        let mut cam = CameraBlock::new(32, 16);
        draw_cell(&mut cam, cell32, &buf, 2, 0, 0);
        cam.invert();
        eprintln!("Input 32-cell (32x32):\n{}", cam.render());

        // Full
        let mut world_full = World::from_parts(B3S23, buf.clone(), cell32, 5);
        world_full.set_k(0);
        let full_idx = world_full.compute(world_full.root, 5);
        let full_result = world_full.buf[full_idx];

        // Half
        let mut world_half = World::from_parts(B3S23, buf, cell32, 5);
        world_half.set_k(1);
        let half_idx = world_half.compute(world_half.root, 5);
        let result = world_half.buf[half_idx];

        let mut cam = CameraBlock::new(16, 8);
        draw_cell(&mut cam, result, &world_half.buf, 1, 0, 0);
        cam.invert();
        eprintln!("half result (16x16, 1 iter):\n{}", cam.render());

        let mut cam = CameraBlock::new(16, 8);
        draw_cell(&mut cam, full_result, &world_full.buf, 1, 0, 0);
        cam.invert();
        eprintln!("full result (16x16, 4 iter):\n{}", cam.render());

        assert!(world_half.buf[result.nw].is_leaf(), "result should be a 16-cell");

        let nw_leaf = world_half.buf[result.nw];
        let expected_nw = Cell::leaf(0, 0, 0, 0b0000_0101_0011_0010);
        assert_eq!(
            nw_leaf, expected_nw,
            "\nExpected nw: {:?}\n     Got nw: {:?}",
            expected_nw, nw_leaf
        );
    }

    /// Helper: build a 32-cell World with a glider near the center.
    #[rustfmt::skip]
    fn make_glider_32() -> World {
        let mut buf = vec![Cell::void()];
        let empty_leaf = Cell::leaf(0, 0, 0, 0);
        let glider_leaf = Cell::leaf(0, 0, 0, 0b0010_0001_0111_0000);

        let e1 = buf.len(); buf.push(empty_leaf);
        let e2 = buf.len(); buf.push(empty_leaf);
        let e3 = buf.len(); buf.push(empty_leaf);
        let gl = buf.len(); buf.push(glider_leaf);
        let nw16 = Cell::new(e1, e2, e3, gl);
        let nw16_idx = buf.len(); buf.push(nw16);

        let e4 = buf.len(); buf.push(empty_leaf);
        let e5 = buf.len(); buf.push(empty_leaf);
        let e6 = buf.len(); buf.push(empty_leaf);
        let e7 = buf.len(); buf.push(empty_leaf);
        let empty16 = Cell::new(e4, e5, e6, e7);
        let ne16_idx = buf.len(); buf.push(empty16);
        let sw16_idx = buf.len(); buf.push(empty16);
        let se16_idx = buf.len(); buf.push(empty16);

        let cell32 = Cell::new(nw16_idx, ne16_idx, sw16_idx, se16_idx);
        World::from_parts(B3S23, buf, cell32, 5)
    }

    /// Render a cell to a string for comparison.
    fn render_cell(cell: Cell, buf: &[Cell], depth: u8) -> String {
        use crate::camera::CameraBlock;
        let side = 1u16 << depth;
        let mut cam = CameraBlock::new(side, side / 2);
        draw_cell(&mut cam, cell, buf, depth - 3, 0, 0);
        cam.invert();
        cam.render().to_string()
    }

    /// Step forward by `n` single steps, returning the raw compute result index.
    /// Each step does compute(k=1) then grow, except the last which skips the grow.
    fn step_n_times(world: &mut World, n: usize) {
        world.set_k(1);
        for i in 0..n {
            let idx = world.compute(world.root, world.depth);
            world.depth -= 1;
            if i < n - 1 {
                world.root = idx;
                world.grow(1);
            } else {
                world.root = idx;
            }
        }
    }

    #[test]
    #[rustfmt::skip]
    fn test_next_k_32cell_k2_is_2_steps() {
        // next_k(2, 5) should give 2 steps
        let mut world_k = make_glider_32();
        world_k.set_k(2);
        let k2_idx = world_k.compute(world_k.root, 5);
        let k2_render = render_cell(world_k.buf[k2_idx], &world_k.buf, 4);

        // 2x single steps for ground truth
        let mut world_step = make_glider_32();
        step_n_times(&mut world_step, 2);
        let step2_render = render_cell(world_step.buf[world_step.root], &world_step.buf, world_step.depth);

        eprintln!("next_k(2) (2 steps):\n{}", k2_render);
        eprintln!("2x next_k(1) (2 steps):\n{}", step2_render);

        assert_eq!(k2_render, step2_render,
            "\n32-cell: next_k(2) should equal 2 single steps");
    }

    #[test]
    #[rustfmt::skip]
    fn test_next_k_32cell_k3_matches_k0() {
        // On a 32-cell (depth 5), k=3 means d <= k+2 = 5 for all levels,
        // so all levels run phase 2 — same as k=0
        let mut w1 = make_glider_32();
        let mut w2 = make_glider_32();

        w1.set_k(3);
        let k3 = w1.compute(w1.root, 5);
        w2.set_k(0);
        let k0 = w2.compute(w2.root, 5);

        assert_eq!(w1.buf[k3], w2.buf[k0],
            "\n32-cell: next_k(3) should equal next_k(0) (both full)\n  k3: {:?}\n  k0: {:?}",
            w1.buf[k3], w2.buf[k0]);
    }

    // ---- 64-cell tests ----

    /// Helper: build a 64-cell World with a glider near the center.
    #[rustfmt::skip]
    fn make_glider_64() -> World {
        let w32 = make_glider_32();
        let mut buf = w32.buf;

        // Wrap: glider is in the SE of the 32-cell's NW 16-cell,
        // so place the 32-cell in the NW quadrant of the 64-cell
        let nw32_idx = w32.root;

        // Build 3 empty 32-cells
        let empty_leaf = Cell::leaf(0, 0, 0, 0);
        let e1 = buf.len(); buf.push(empty_leaf);
        let e2 = buf.len(); buf.push(empty_leaf);
        let e3 = buf.len(); buf.push(empty_leaf);
        let e4 = buf.len(); buf.push(empty_leaf);
        let empty16 = Cell::new(e1, e2, e3, e4);
        let e16a = buf.len(); buf.push(empty16);
        let e16b = buf.len(); buf.push(empty16);
        let e16c = buf.len(); buf.push(empty16);
        let e16d = buf.len(); buf.push(empty16);
        let empty32 = Cell::new(e16a, e16b, e16c, e16d);
        let ne32_idx = buf.len(); buf.push(empty32);
        let sw32_idx = buf.len(); buf.push(empty32);
        let se32_idx = buf.len(); buf.push(empty32);

        let cell64 = Cell::new(nw32_idx, ne32_idx, sw32_idx, se32_idx);
        World::from_parts(B3S23, buf, cell64, 6)
    }

    #[test]
    #[rustfmt::skip]
    fn test_next_k_64cell_k2_is_2_steps() {
        // next_k(2, 6) should give 2 steps
        let mut world_k = make_glider_64();
        world_k.set_k(2);
        let k2_idx = world_k.compute(world_k.root, 6);
        let k2_render = render_cell(world_k.buf[k2_idx], &world_k.buf, 5);

        // 2x single steps for ground truth
        let mut world_step = make_glider_64();
        step_n_times(&mut world_step, 2);
        let step2_render = render_cell(world_step.buf[world_step.root], &world_step.buf, world_step.depth);

        eprintln!("64-cell next_k(2) (2 steps):\n{}", k2_render);
        eprintln!("64-cell 2x next_k(1) (2 steps):\n{}", step2_render);

        assert_eq!(k2_render, step2_render,
            "\n64-cell: next_k(2) should equal 2 single steps");
    }
}

#[cfg(test)]
mod test_hash {
    use crate::cell::{get_compute_count, reset_compute_count, Cell};
    use crate::rule_set::B3S23;
    use crate::world::World;

    /// Build a World with a glider near the center at the given depth
    #[rustfmt::skip]
    fn make_glider(depth: u8) -> World {
        assert!(depth >= 4, "need at least a 16-cell for a glider");

        let mut buf = vec![Cell::void()];

        let empty_leaf = Cell::leaf(0, 0, 0, 0);
        // Glider in the se quadrant of this leaf
        let glider_leaf = Cell::leaf(0, 0, 0, 0b0010_0001_0111_0000);

        let el = buf.len(); buf.push(empty_leaf);
        let gl = buf.len(); buf.push(glider_leaf);

        // Depth 4: 16-cell with glider in nw's se corner
        let nw16 = Cell::new(el, el, el, gl);
        let empty16 = Cell::new(el, el, el, el);
        let nw16_idx = buf.len(); buf.push(nw16);
        let empty16_idx = buf.len(); buf.push(empty16);

        let mut root = Cell::new(nw16_idx, empty16_idx, empty16_idx, empty16_idx);

        // Wrap in progressively larger nodes up to target depth
        for _ in 5..=depth {
            let root_idx = buf.len(); buf.push(root);
            root = Cell::new(root_idx, 0, 0, 0);
        }

        World::from_parts(B3S23, buf, root, depth)
    }

    #[test]
    fn test_compute_count_scaling() {
        let mut counts = Vec::new();

        for depth in 4..=8 {
            let mut world = make_glider(depth);

            reset_compute_count();
            world.set_k(0);
            world.next();
            let count = get_compute_count();

            eprintln!(
                "depth {depth} ({}x{} cell): {count} compute calls",
                1u64 << depth,
                1u64 << depth
            );
            counts.push(count);
        }

        // Without memoization, compute calls grow exponentially.
        // Log the ratios so we can see the blowup and later confirm
        // memoization flattens it.
        for i in 1..counts.len() {
            let ratio = counts[i] as f64 / counts[i - 1] as f64;
            eprintln!("depth {} -> {}: {:.1}x", i + 4, i + 5, ratio);
        }
    }
}
