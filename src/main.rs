mod cell;
mod render;
mod rules;
mod world;

use std::u16;

use cell::Cell;
use cell::CellHash;
use cell::LEAF_MASK;
use cell::RES_UNSET_MASK;
use render::Camera;
use world::World;

struct CellBuf {
    /// The index of the root cell in `buf`
    root: usize,

    /// The list of all cells. This is where all of the memory allocated by the program goes
    buf: Vec<Cell>,

    /// The size of the world where n means a world sidelength of 2^n
    size: usize,
}

const C1: usize = 1;
const C2: usize = 1;

impl CellBuf {
    pub fn new() -> Self {
        Self {
            root: 0,
            buf: vec![Cell::unset(); next_prime(10_000)],
            size: 0,
        }
    }

    /// Inserts a cell into the buffer, returning its index
    pub fn insert(&mut self, cell: Cell) -> usize {
        // If the hashmap is more than 80% full, grow it
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

    /// Grow the hashmap
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
                return index;
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
const DEPTH: u8 = 0;

// Makes the smallest possible cell, 8 on a side
fn make_a_cell() -> Cell {
    Cell {
        nw: 0b0000_0000_0011_0011 | LEAF_MASK,
        ne: 0b0000_0000_1100_1100,
        sw: 0b0011_0011_0000_0000,
        se: 0b1100_1100_0000_0000,
        res: RES_UNSET_MASK,
    }
}

/// Assumes the cell is a leaf (hence the leaf mask)
fn draw_cell(cam: &mut Camera, cell: Cell) {
    let Cell { nw, ne, sw, se, .. } = cell;

    draw_rule(cam, (nw - LEAF_MASK) as u16, 0, 0);
    draw_rule(cam, ne as u16, 4, 0);
    draw_rule(cam, sw as u16, 0, 4);
    draw_rule(cam, se as u16, 4, 4);
}

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

fn main() {
    env_logger::init();

    let cell = make_a_cell();

    let mut world = World::new(DEPTH, LIFE_RULES).unwrap();
    world.buf[0] = cell;

    let mut cam = Camera::new(8, 8);
    draw_cell(&mut cam, cell);
    let s = cam.render();
    print!("{s}");

    let res = world.next();

    cam.reset();

    draw_rule(&mut cam, res, 2, 2);

    let s = cam.render();
    print!("{s}");
}
