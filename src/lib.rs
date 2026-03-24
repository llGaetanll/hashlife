pub mod camera;
pub mod cell;
pub mod rle_file;
pub mod rule_set;
pub mod world;

pub mod rle_data;

mod util_parse;

/// Logging macro that compiles to nothing unless the `trace` feature is enabled.
#[macro_export]
macro_rules! trace_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "trace")]
        log::trace!($($arg)*);
    }
}

/// Like `trace_log!` but at info level.
#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "trace")]
        log::info!($($arg)*);
    }
}

pub type ScreenSize = u16;
pub type CellOffset = i16;
pub type WorldOffset = i128;
