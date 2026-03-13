// commands/ module - split from monolithic commands.rs
// Each submodule handles a group of related subcommands.

// Re-export command types from parent (main.rs) for submodules
pub(crate) use super::{ConfigCommands, DatabaseCommands, DiagnosticCommands, DiagnosticSettingsCommands, VsCodeCommands};

mod connect;
mod config;
#[cfg(feature = "persistence")]
mod database;
mod diagnose;
mod diagnostic_settings;
mod fix;
mod monitoring;
#[cfg(feature = "multi-session")]
mod multi_session;
mod tui;
mod vscode;

pub use connect::*;
pub use config::*;
#[cfg(feature = "persistence")]
pub use database::*;
pub use diagnose::*;
pub use diagnostic_settings::*;
pub use fix::*;
pub use monitoring::*;
#[cfg(feature = "multi-session")]
pub use multi_session::*;
pub use tui::*;
pub use vscode::*;
