#[macro_use]
extern crate log;

pub mod cli;
pub mod client;
pub mod config;
pub mod function;
pub mod mcp;
pub mod rag;
pub mod render;
pub mod repl;
pub mod serve;

#[macro_use]
pub mod utils;

// Many modules historically refer to helpers as `crate::<name>`.
// The binary crate root pulls these into scope via `use crate::utils::*;`.
// Mirror that behavior for the library crate so integration tests can link.
pub use utils::*;
