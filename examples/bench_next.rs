use std::time::Instant;

use hashlife::rle_data::RleBuffer;
use hashlife::rle_file;
use hashlife::world::World;

fn load_rle(path: &str) -> World {
    let bytes = std::fs::read(path).expect("Failed to read file");
    let mut buf = RleBuffer::new();
    let header = rle_file::read_rle(&bytes, &mut buf).expect("Failed to parse RLE");
    World::from_rle(header.set, buf)
}

fn main() {
    env_logger::init();

    let path = std::env::args().nth(1)
        .unwrap_or_else(|| "tests/rle_pats/ortholoopship_synth.rle".to_string());

    let t = Instant::now();
    let mut world = load_rle(&path);
    eprintln!("load_rle:    {:?}", t.elapsed());
    eprintln!("depth: {}, buf len: {}, hashpop: {}", world.depth, world.buf.len(), world.hashpop);

    let t = Instant::now();
    world.grow(2);
    eprintln!("grow(2):     {:?}", t.elapsed());
    eprintln!("depth: {}, buf len: {}, hashpop: {}", world.depth, world.buf.len(), world.hashpop);

    let t = Instant::now();
    world.gc();
    eprintln!("gc:          {:?}", t.elapsed());
    eprintln!("depth: {}, buf len: {}, hashpop: {}", world.depth, world.buf.len(), world.hashpop);

    let t = Instant::now();
    world.next();
    eprintln!("next(1):     {:?}", t.elapsed());
    eprintln!("depth: {}, buf len: {}, hashpop: {}", world.depth, world.buf.len(), world.hashpop);
}
