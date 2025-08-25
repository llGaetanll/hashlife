use hashlife::camera::Camera;
use hashlife::cell::Cell;
use hashlife::rule_set::B3S23;
use hashlife::world::World;

const DUMMY_LEAF: Cell = Cell::leaf(
    0b0010_0001_0111_0000,
    0b0000_0100_0101_0110,
    0b0110_1010_0010_0000,
    0b0000_1110_1000_0100,
);

fn setup_world(depth: u8) -> World {
    let mut world = World::new(B3S23);

    world.buf.pop();
    world.buf.push(DUMMY_LEAF);

    let n = world.buf.len();

    for i in 0..depth {
        let i = n + i as usize - 1;
        world.buf.push(Cell::new(i, i, i, i));
    }

    world.root = world.buf.len() - 1;
    world.depth = depth;

    world
}

fn main() {
    let mut cam = Camera::new(10, 10);
    let world = setup_world(6);

    cam.draw(&world);

    let s = cam.render();
    println!("{s}");
}
