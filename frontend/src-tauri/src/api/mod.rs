// Legacy public module path retained for compatibility with existing call sites.
#[allow(clippy::module_inception)]
pub mod api;
pub mod commands;

pub use api::*;
// Don't re-export commands to avoid conflicts - lib.rs will import directly
