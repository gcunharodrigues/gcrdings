pub mod commands;
pub mod metadata;
// Legacy public module path retained for compatibility with existing call sites.
#[allow(clippy::module_inception)]
pub mod ollama;

pub use ollama::*;
// Don't re-export commands to avoid conflicts - lib.rs will import directly
