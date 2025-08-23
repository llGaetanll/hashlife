use std::fs::read_to_string;
use std::path::Path;

use hashlife::camera::Camera;
use hashlife::rle::read_rle;
use hashlife::world::World;

const WIDTH: u16 = HEIGHT * 2;
const HEIGHT: u16 = 50;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(Path::new).expect("A .rle file is required");
    let data = read_to_string(path).expect("Failed to open .rle file");

    println!("```");
    print!("{data}");
    println!("```");

    let data = data.as_bytes();

    let mut world = World::new("b3s23").unwrap();
    let mut cam = Camera::new(WIDTH, HEIGHT);

    world.grow(5);

    read_rle(data, |x, y| world.set(x, y)).expect("Failed to read rle file");

    cam.draw(&world);
}
