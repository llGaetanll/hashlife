use std::fs::read_to_string;
use std::path::Path;

use hashlife::camera::Camera;
use hashlife::parse_rle::read_rle;
use hashlife::rule_set::B3S23;
use hashlife::world::World;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(Path::new).expect("A .rle file is required");
    let data = read_to_string(path).expect("Failed to open .rle file");

    let data = data.as_bytes();

    let mut cam = Camera::new(100, 100);
    let mut world = World::new(B3S23);
    world.grow(5);

    read_rle(data, |x, y| world.set(x, y)).expect("Failed to read RLE file");

    cam.draw(&world);
    let s = cam.render();

    print!("{s}");
}
