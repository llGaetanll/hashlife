pub mod camera;
pub mod cell;
pub mod events;
pub mod io;
pub mod rules;
pub mod world;

pub type ScreenSize = u16;
pub type CellOffset = i16;
pub type WorldOffset = i128;

use cell::Cell;

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

fn build_cell(nw: Cell, ne: Cell, sw: Cell, se: Cell, cells: &mut Vec<Cell>) -> usize {
    cells.pop(); // Pop the old root

    let n = cells.len();

    cells.extend([nw, ne, sw, se]);

    let cell = Cell {
        nw: n,
        ne: n + 1,
        sw: n + 2,
        se: n + 3,
    };

    let n = cells.len();

    cells.push(cell);

    n
}

fn build_8_cell() -> Cell {
    Cell::leaf(
        0b0000_0000_0011_0011,
        0b0000_0000_1100_1100,
        0b0011_0011_0000_0000,
        0b1100_1100_0000_0000,
    )
}

fn build_8_cell_checker() -> Cell {
    let c = 0b0101_1010_0101_1010;

    Cell::leaf(c, c, c, c)
}

fn build_8_cell_full() -> Cell {
    Cell::leaf(u16::MAX, u16::MAX, u16::MAX, u16::MAX)
}

fn build_8_glider() -> Cell {
    Cell::leaf(
        0b0010_0001_0111_0000,
        0b0000_0100_0101_0110,
        0b0110_1010_0010_0000,
        0b0000_1110_1000_0100,
    )
}

fn build_16_cell(cells: &mut Vec<Cell>) -> usize {
    cells.pop(); // Pop the old root

    let nw = build_8_glider();
    let ne = build_8_glider();
    let sw = build_8_glider();
    let se = build_8_glider();

    cells.push(nw);
    cells.push(ne);
    cells.push(sw);
    cells.push(se);

    let n = cells.len();

    let root = Cell {
        nw: 1,
        ne: 2,
        sw: 3,
        se: 4,
    };

    cells.push(root);

    n
}
