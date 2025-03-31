mod cell;
mod render;
mod rules;
mod world;

use cell::Cell;
use cell::CellHash;
use cell::LEAF_MASK;
use cell::RES_UNSET_MASK;
use render::Camera;
use std::io::Write;
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

fn setup_logging() {
    // Initialize the tracing subscriber with custom formatting
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(true) // Keep the target (module path)
        .with_ansi(true) // Enable colors
        .without_time()
        .init();
}

// See: https://conwaylife.com/wiki/Rulestring
const LIFE_RULES: &str = "b3s23";
const DEPTH: u8 = 0;

/// Build this leaf cell:
///
///    00000000
///    00000000
///    00111100
///    00111100
///    00111100
///    00111100
///    00000000
///    00000000
///
///
fn build_8_cell() -> Cell {
    Cell {
        nw: 0b0000_0000_0011_0011 | LEAF_MASK,
        ne: 0b0000_0000_1100_1100,
        sw: 0b0011_0011_0000_0000,
        se: 0b1100_1100_0000_0000,
        res: RES_UNSET_MASK,
    }
}

fn build_full_8_cell() -> Cell {
    Cell {
        nw: u16::MAX as usize | LEAF_MASK,
        ne: u16::MAX as usize,
        sw: u16::MAX as usize,
        se: u16::MAX as usize,
        res: RES_UNSET_MASK,
    }
}

fn build_16_cell(cells: &mut [Cell]) -> Cell {
    let nw = build_8_cell();
    let ne = build_8_cell();
    let sw = build_8_cell();
    let se = build_8_cell();

    cells[1] = nw;
    cells[2] = ne;
    cells[3] = sw;
    cells[4] = se;

    let cell = Cell {
        nw: 1,
        ne: 2,
        sw: 3,
        se: 4,
        res: RES_UNSET_MASK,
    };

    cells[0] = cell;

    cell
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

/// Assumes the cell is a leaf (hence the leaf mask)
fn draw_leaf_cell(cam: &mut Camera, mut cell: Cell, dx: usize, dy: usize) {
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

fn draw_cell(cam: &mut Camera, cell: Cell, cells: &[Cell], depth: u8, dx: usize, dy: usize) {
    if cell.is_leaf() {
        assert_eq!(depth, 0, "Wrong depth, expected 0, got {depth}");

        draw_leaf_cell(cam, cell, dx, dy);
    } else {
        let Cell { nw, ne, sw, se, .. } = cell;

        let d = 2usize.pow(2 + depth as u32);

        let depth = depth - 1;

        draw_cell(cam, cells[nw], cells, depth, dx, dy);
        draw_cell(cam, cells[ne], cells, depth, dx + d, dy);
        draw_cell(cam, cells[sw], cells, depth, dx, dy + d);
        draw_cell(cam, cells[se], cells, depth, dx + d, dy + d);
    }
}

fn main() {
    setup_logging();

    let mut cam = Camera::new(16, 16);

    let mut world = World::new(DEPTH, LIFE_RULES).unwrap();

    let cell = build_16_cell(&mut world.buf);

    draw_cell(&mut cam, cell, &world.buf, 1, 0, 0);
    let s = cam.render();
    print!("{s}");

    // cam.reset();
    //
    // let leaf = cell_utils::center16(cell, &world.buf);
    // draw_leaf_cell(&mut cam, leaf, 4, 4);
    // let s = cam.render();
    // print!("{s}");
    //
    // cam.reset();
    //
    // let w = build_8_cell();
    // let e = build_8_cell();
    // let leaf = cell_utils::h_center16(w, e);
    // draw_leaf_cell(&mut cam, leaf, 4, 4);
    // let s = cam.render();
    // print!("{s}");
    //
    // cam.reset();
    //
    // let n = build_8_cell();
    // let s = build_8_cell();
    // let leaf = cell_utils::v_center16(n, s);
    // draw_leaf_cell(&mut cam, leaf, 4, 4);
    // let s = cam.render();
    // print!("{s}");
    //
    // cam.reset();
    //
    // let cell = build_16_cell(&mut world.buf);
    // let rule = cell_utils::super_center16(cell, &world.buf);
    // draw_rule(&mut cam, rule, 8, 8);
    // let s = cam.render();
    // print!("{s}");

    world.next();
}
