use hashlife::camera::Camera;
use hashlife::rule_set::B3S23;
use hashlife::world::World;

fn main() {
    let mut cam = Camera::new(10, 10);
    let mut world = World::new(B3S23);

    world.set(0, 0);
    cam.draw(&world);

    let s = cam.render();
    println!("{s}");
}
