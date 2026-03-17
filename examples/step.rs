use std::error::Error;

use hashlife::rle_data::RleBuffer;
use hashlife::rle_file;
use hashlife::world::World;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("Usage: step <file.rle>");
    let bytes = std::fs::read(path)?;
    let mut buf = RleBuffer::new();
    let header = rle_file::read_rle(&bytes, &mut buf)?;
    let mut world = World::from_rle(header.set, buf);

    eprintln!("Loaded world: depth={}", world.depth);
    world.dump_tree();
    world.next();
    eprintln!("\nAfter 1 step: depth={}", world.depth);
    world.dump_tree();

    Ok(())
}
