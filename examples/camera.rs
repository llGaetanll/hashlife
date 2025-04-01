use hashlife::camera::Camera;
use hashlife::cell::Cell;
use hashlife::world::World;

fn setup_simple_world() -> World {
    // See: https://conwaylife.com/wiki/Rulestring
    const LIFE_RULES: &str = "b3s23";

    let mut world = World::new(0, LIFE_RULES).unwrap();

    world.buf.pop();
    world.buf.push(Cell::leaf(
        0b0010_0001_0111_0000,
        0b0000_0100_0101_0110,
        0b0110_1010_0010_0000,
        0b0000_1110_1000_0100,
    ));

    world
}

/// A simple example to set up ray casting with the camera
fn main() {
    let mut cam = Camera::new(100, 100);
    let world = setup_simple_world();

    let s = cam.render();

    println!("{s}");

    cam.reset();
}
