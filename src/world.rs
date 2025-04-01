use crate::cell::CellHash;
use crate::cell::LEAF_MASK;
use crate::camera::Camera;
use crate::rules::RuleSet;
use crate::rules::RuleSetError;

use crate::cell::Cell;

pub struct World {
    /// Life rules
    ///
    /// Indexing into this array with rule `r` yields the result of `r`.
    pub rules: Vec<u16>,

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
    pub fn new(depth: u8, rule_set: &str) -> Result<Self, RuleSetError> {
        let rule_set: RuleSet = rule_set.parse()?;
        let rules = rule_set.compute_rules();

        let buf = vec![Cell::void(), Cell::void()];

        let root = 1;

        Ok(Self {
            rules,
            root,
            buf,
            depth,
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

    pub fn draw(&self, cam: &mut Camera) {
        let root = self.buf[self.root];
        let depth = self.depth;

        draw_cell(cam, root, &self.buf, depth, 0, 0);
    }

    /// Insert the cell at the given hash (turned into index)
    ///
    /// If there is a collision, resize the hash
    fn insert_cell(&mut self, hash: CellHash, cell: Cell) {
        let n = self.buf.len();
        let index = hash % n;

        // collision
        if self.buf[index] != Cell::void() {
            self.grow_buf();
            self.insert_cell(hash, cell);
        } else {
            self.buf[index] = cell;
        }
    }

    /// Doubles the size of the cell buffer, moving all the cells over to their new hash
    fn grow_buf(&mut self) {
        let n = Self::next_prime(2 * self.buf.len());

        assert!(n < LEAF_MASK, "Out of memory!");

        let mut buf = vec![Cell::unset(); n];
        self.copy_children(self.root, &mut buf);

        self.buf = buf;
    }

    fn copy_children(&self, index: usize, buf: &mut [Cell]) {
        let n = buf.len();

        let cell = self.buf[index];
        let h = cell.hash() % n;
        buf[h] = cell;

        if let Some(children) = cell.children() {
            for index in children {
                self.copy_children(index, buf);
            }
        }
    }

    fn next_prime(mut n: usize) -> usize {
        fn is_prime(n: usize) -> bool {
            let mut i = 3;
            while i * i < n {
                if i % n == 0 {
                    return false;
                }

                i += 2;
            }

            true
        }

        n |= 1;

        loop {
            if is_prime(n) {
                return n;
            }

            n += 2
        }
    }
}

impl ::std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        struct DebugWorld<'a> {
            root: &'a usize,
            depth: &'a u8,
            buf: &'a [Cell],
        }

        let Self {
            root, buf, depth, ..
        } = self;

        ::std::fmt::Debug::fmt(&DebugWorld { root, depth, buf }, f)
    }
}

/// Draws a 4 cell
fn draw_rule(cam: &mut Camera, rule: u16, dx: usize, dy: usize) {
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
fn draw_leaf(cam: &mut Camera, mut cell: Cell, dx: usize, dy: usize) {
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
fn draw_cell(cam: &mut Camera, cell: Cell, cells: &[Cell], depth: u8, dx: usize, dy: usize) {
    if cell.is_leaf() {
        draw_leaf(cam, cell, dx, dy);
    } else {
        let Cell { nw, ne, sw, se, .. } = cell;

        let d = 2usize.pow(2 + depth as u32);

        draw_cell(cam, cells[nw], cells, depth - 1, dx, dy);
        draw_cell(cam, cells[ne], cells, depth - 1, dx + d, dy);
        draw_cell(cam, cells[sw], cells, depth - 1, dx, dy + d);
        draw_cell(cam, cells[se], cells, depth - 1, dx + d, dy + d);
    }
}
