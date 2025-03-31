use crate::cell::CellHash;
use crate::cell::LEAF_MASK;
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

    /// Index of the void (empty) [`Cell`] in `buf`
    pub void: usize,

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

        const MIN_BUF_SIZE: usize = 10_000;
        let mut buf = vec![Cell::unset(); Self::next_prime(MIN_BUF_SIZE)];
        buf[0] = Cell::void();

        let void = 0;
        let root = 0;

        Ok(Self {
            rules,
            root,
            void,
            buf,
            depth,
        })
    }

    pub fn next(&mut self) -> Cell {
        // todo

        let mut root_cell = self.buf[self.root];

        root_cell.next(&self.rules, &self.buf)
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
