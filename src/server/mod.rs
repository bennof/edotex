#[path = "server.rs"]
mod core;
pub mod mime;
pub mod statichandler;

pub use core::{Server, write_json};
pub use statichandler::{handle_static, serve_static};
