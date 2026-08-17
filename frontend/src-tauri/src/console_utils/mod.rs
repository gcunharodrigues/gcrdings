pub mod commands;
// Legacy public module path retained for compatibility with existing call sites.
#[allow(clippy::module_inception)]
pub mod console_utils;

pub use console_utils::*;
// Don't re-export commands to avoid conflicts - lib.rs will import directly
