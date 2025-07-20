use hashlife::camera::Camera;
use hashlife::world::World;

const LIFE_RULES: &str = "b3s23";

fn main() {
    let mut cam = Camera::new(10, 10);
    let mut world = World::new(LIFE_RULES).unwrap();

    world.set(0, 0);
    cam.draw(&world);

    let s = cam.render();
    println!("{s}");
}
