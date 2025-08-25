pub mod camera;
pub mod cell;
pub mod parse_rle;
pub mod rule_set;
pub mod world;

mod parse_util;

pub type ScreenSize = u16;
pub type CellOffset = i16;
pub type WorldOffset = i128;
