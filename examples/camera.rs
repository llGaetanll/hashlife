use hashlife::camera;
use hashlife::camera::Camera;
use hashlife::cell::Cell;
use hashlife::world::World;

fn setup_world() -> World {
    // See: https://conwaylife.com/wiki/Rulestring
    const LIFE_RULES: &str = "b3s23";

    let mut world = World::new(0, LIFE_RULES).unwrap();

    world.buf.pop();
    world.buf.extend([
        Cell::leaf(
            0b0010_0001_0111_0000,
            0b0000_0100_0101_0110,
            0b0110_1010_0010_0000,
            0b0000_1110_1000_0100,
        ),
        Cell::leaf(
            0b0010_0001_0111_0000,
            0b0000_0100_0101_0110,
            0b0110_1010_0010_0000,
            0b0000_1110_1000_0100,
        ),
        Cell::leaf(
            0b0010_0001_0111_0000,
            0b0000_0100_0101_0110,
            0b0110_1010_0010_0000,
            0b0000_1110_1000_0100,
        ),
        Cell::leaf(
            0b0010_0001_0111_0000,
            0b0000_0100_0101_0110,
            0b0110_1010_0010_0000,
            0b0000_1110_1000_0100,
        ),
        Cell::new(1, 2, 3, 4),
    ]);
    world.root = 5;

    world
}

fn main() {
    let mut cam = Camera::new(100, 100);
    let world = setup_world();
    let root = world.buf[5];

    let n = 4; // Leaf = 3
    let scale = 0;

    camera::draw_cell(&mut cam, &world.buf, root, 0, 0, n, scale);

    let s = cam.render();
    println!("{s}");
}
