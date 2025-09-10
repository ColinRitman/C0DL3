// C0DL3 zkSync Implementation Library
// Production-grade zkSync Hyperchains with elite-level privacy

pub mod privacy;
pub mod mining;
// Remove fuego_daemon from lib to avoid referencing bin-only types
// pub mod fuego_daemon;

#[cfg(feature = "cli-ui")]
pub mod unified_cli;
#[cfg(feature = "cli-ui")]
pub mod enhanced_cli;
#[cfg(feature = "cli-ui")]
pub mod visual_cli;
#[cfg(feature = "cli-ui")]
pub mod simple_visual_cli;
#[cfg(feature = "cli-ui")]
pub mod cli_interface;

// Expose security utilities to library users
pub mod security;

// Re-export main privacy components
pub use privacy::*;
