// #![deny(clippy::all)]
// #![deny(clippy::pedantic)]
// #![allow(clippy::uninlined_format_args)]

pub mod utils;
pub mod macros;
pub mod database;
pub mod crawler;
// pub mod process_manager;
pub mod net;
pub mod process_manager;

pub use utils::errors::Result;
pub use utils::errors::Error;