mod rules;
mod cell;
mod world;

use cell::Cell;
use cell::CellHash;

struct CellBuf {
    root: usize,
    buf: Vec<Cell>,
    size: usize
}

const C1: usize = 1;
const C2: usize = 1;

impl CellBuf {
    pub fn new() -> Self {
        Self {
            root: 0,
            buf: vec![Cell::unset(); next_prime(10_000)],
            size: 0
        }
    }

    /// Inserts a cell into the buffer, returning its index
    pub fn insert(&mut self, cell: Cell) -> usize {
        if self.size as f64 / self.buf.len() as f64 > 0.8 {
            self.grow()
        }

        self.size += 1;

        Self::insert_buf(cell, &mut self.buf)
    }

    pub fn get(&self, index: usize) -> Option<Cell> {
        let cell = self.buf[index];
        if cell == Cell::unset() {
            None
        } else {
            Some(cell)
        }
    }

    fn grow(&mut self) {
        let n = next_prime(2 * self.buf.len());
        let mut buf = vec![Cell::unset(); n];

        self.root = self.move_cell(self.root, &mut buf);
    }

    fn move_cell(&self, index: usize, buf: &mut [Cell]) -> usize {
        let mut cell = self.buf[index];

        if !cell.is_leaf() {
            cell.nw = self.move_cell(cell.nw, buf);
            cell.ne = self.move_cell(cell.ne, buf);
            cell.sw = self.move_cell(cell.sw, buf);
            cell.se = self.move_cell(cell.se, buf);
            cell.res = self.move_cell(cell.res, buf);
        }

        Self::insert_buf(cell, buf)
    }

    fn insert_buf(cell: Cell, buf: &mut [Cell]) -> usize {
        let n = buf.len();
        let h: CellHash = cell.hash();

        for i in 0usize.. {
            let index = (h + C1 * i + C2 * i * i) % n;

            if buf[index] == Cell::unset() {
                buf[index] = cell;
                return index
            }
        }

        unreachable!()
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

// See: https://conwaylife.com/wiki/Rulestring
const LIFE_RULES: &str = "b3s23";
const DEPTH: u8 = 5;

fn main() {

}
