use crate::cell::CellHash;
use crate::cell::LEAF_MASK;
use crate::rules::RuleSet;
use crate::rules::RuleSetError;

use crate::cell::Cell;
use crate::WorldOffset;

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

#[allow(clippy::collapsible_else_if)]
fn set_bit(buf: &mut [Cell], cell_ptr: usize, x: WorldOffset, y: WorldOffset, depth: u8) {
    match depth {
        0..3 => unreachable!(),

        // Leaf
        3 => {
            let cell = &mut buf[cell_ptr];

            if x < 0 {
                if y < 0 {
                    cell.sw |= 1 << (3 - (x & 3) + 4 * (y & 3));
                } else {
                    cell.nw |= 1 << (3 - (x & 3) + 4 * (y & 3));
                }
            } else {
                if y < 0 {
                    cell.se |= 1 << (3 - (x & 3) + 4 * (y & 3));
                } else {
                    cell.ne |= 1 << (3 - (x & 3) + 4 * (y & 3));
                }
            }
        }

        // Non-leaf
        depth => {
            let cell = &mut buf[cell_ptr];

            let child_ptr = if x < 0 {
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
            };

            // We're pointing at nothing
            if *child_ptr == 0 {
                // Initialize the nodes
                if depth == 3 {
                    // Leaf
                    todo!()
                } else {
                    todo!()
                }
            }

            let w = 1 << depth;
            let x = (x & (w - 1)) - (w >> 1);
            let y = (y & (w - 1)) - (w >> 1);

            let child_ptr = *child_ptr;

            set_bit(buf, child_ptr, x, y, depth - 1)
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
