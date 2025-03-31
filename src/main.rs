mod cell;
mod render;
mod rules;
mod world;

use cell::Cell;
use cell::LEAF_MASK;
use render::Camera;
use world::World;

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
    }
}

fn build_full_8_cell() -> Cell {
    Cell {
        nw: u16::MAX as usize | LEAF_MASK,
        ne: u16::MAX as usize,
        sw: u16::MAX as usize,
        se: u16::MAX as usize,
    }
}

fn main() {
    setup_logging();
    let k = 2;
    let sl = 2usize.pow(3 + k as u32);

    let mut cam = Camera::new(sl, sl);

    let mut world = World::new(0, LIFE_RULES).unwrap();
    let cell = build_full_8_cell();
    world.buf[1] = cell;

    world.draw(&mut cam);
    let s = cam.render();
    print!("{s}");

    world.grow(k);

    cam.reset();

    world.draw(&mut cam);
    let s = cam.render();
    print!("{s}");

    world.next();
}
