pub mod camera;
pub mod cell;
pub mod rle_file;
pub mod rule_set;
pub mod world;

pub mod rle_data;

mod util_parse;

pub type ScreenSize = u16;
pub type CellOffset = i16;
pub type WorldOffset = i128;
