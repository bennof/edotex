#[path = "server.rs"]
mod core;
pub mod embeddedhandler;
mod filehandler;
pub mod mime;
pub mod statichandler;

pub use core::{Server, write_json};
pub use embeddedhandler::handle_embedded;
pub use statichandler::{handle_static, serve_static};
