use anyhow::Result;

mod auto_fix;
mod aws;
mod aws_config_validator;
mod cli;
mod commands;
mod config;
mod diagnostic;
mod error;
mod error_recovery;
mod health;
mod iam_diagnostics;
mod instance_diagnostics;
mod logging;
mod manager;
#[cfg(feature = "performance-monitoring")]
mod monitor;
#[cfg(feature = "multi-session")]
mod multi_session;
#[cfg(feature = "multi-session")]
mod multi_session_ui;
mod network_diagnostics;
#[cfg(feature = "performance-monitoring")]
mod performance;
#[cfg(feature = "persistence")]
mod persistence;
mod port_diagnostics;
mod preventive_check;
mod realtime_feedback;
#[cfg(feature = "auto-reconnect")]
mod reconnect;
mod resource;
mod session;
mod ssm_agent_diagnostics;
mod targets;
mod ui;
mod user_messages;
mod vscode;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
