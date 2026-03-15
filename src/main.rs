use anyhow::Result;
use clap::Parser;
use tracing::info;

mod auto_fix;
mod aws;
mod aws_config_validator;
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
#[allow(dead_code)]
mod monitor;
#[cfg(feature = "multi-session")]
#[allow(dead_code)]
mod multi_session;
#[cfg(feature = "multi-session")]
#[allow(dead_code)]
mod multi_session_ui;
mod network_diagnostics;
#[cfg(feature = "performance-monitoring")]
#[allow(dead_code)]
mod performance;
#[cfg(feature = "persistence")]
#[allow(dead_code)]
mod persistence;
mod port_diagnostics;
mod preventive_check;
mod realtime_feedback;
#[cfg(feature = "auto-reconnect")]
#[allow(dead_code)]
mod reconnect;
mod resource;
mod session;
mod ssm_agent_diagnostics;
mod targets;
mod ui;
mod user_messages;
mod vscode;

use commands::*;
use config::Config;
use error::NimbusError;
use error_recovery::{ContextualError, ErrorContext, ErrorRecoveryManager, RecoveryConfig};
use logging::{LoggingConfig, StructuredLogger};
use targets::TargetsConfig;
use user_messages::UserMessageSystem;

#[derive(Parser)]
#[command(name = "nimbus")]
#[command(about = "High-performance EC2 SSM connection manager with automatic session management")]
#[command(version = "3.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging with enhanced configuration
    let logging_config = LoggingConfig {
        level: if cli.verbose {
            "debug".to_string()
        } else {
            "info".to_string()
        },
        performance_tracing: cli.verbose,
        ..LoggingConfig::default()
    };

    let _guard = match crate::logging::init_logging(&logging_config) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            return Err(anyhow::anyhow!("Failed to initialize logging: {}", e));
        }
    };

    info!("Starting Nimbus v3.0.0");

    // Initialize error recovery manager
    let recovery_manager = ErrorRecoveryManager::new(RecoveryConfig::default());

    // Initialize user message system
    let message_system = UserMessageSystem::new();

    // Load configuration with error handling
    let config = match Config::load(cli.config.as_deref()).await {
        Ok(config) => config,
        Err(e) => {
            let ec2_error = NimbusError::from(e);
            let context = ErrorContext::new("load_config", "main")
                .with_info("config_path", cli.config.as_deref().unwrap_or("default"));
            let contextual_error = ContextualError::new(ec2_error, context);

            StructuredLogger::log_error(&contextual_error);

            let user_message = message_system.get_error_message(&contextual_error.error);
            eprintln!("{}", user_message.format_for_display());

            return Err(contextual_error.error.into());
        }
    };

    // Execute command with error handling
    let result = match cli.command {
        Commands::Connect {
            target,
            targets_file,
            instance_id,
            local_port,
            remote_port,
            remote_host,
            profile,
            region,
            priority,
            precheck,
        } => {
            let mut effective_config = config.clone();

            // If a target is specified, load targets file and apply target defaults.
            let target_config = if let Some(target_name) = target.as_deref() {
                let (targets_cfg, targets_path) =
                    TargetsConfig::load(targets_file.as_deref()).await?;
                let t = targets_cfg.get(target_name).ok_or_else(|| {
                    anyhow::anyhow!("Target '{}' not found in {:?}", target_name, targets_path)
                })?;
                Some(t.clone())
            } else {
                None
            };

            let resolved_instance_id = match (instance_id, target_config.as_ref()) {
                (Some(id), _) => id,
                (None, Some(t)) => t.instance_id.clone(),
                (None, None) => {
                    anyhow::bail!("Either --instance-id or --target must be specified")
                }
            };

            // Preserve historical defaults when neither CLI nor target specifies ports.
            let resolved_local_port = local_port
                .or_else(|| target_config.as_ref().and_then(|t| t.local_port))
                .unwrap_or(8080);
            let resolved_remote_port = remote_port
                .or_else(|| target_config.as_ref().and_then(|t| t.remote_port))
                .unwrap_or(80);

            // Resolve remote_host from CLI or target config
            let resolved_remote_host =
                remote_host.or_else(|| target_config.as_ref().and_then(|t| t.remote_host.clone()));

            let resolved_profile =
                profile.or_else(|| target_config.as_ref().and_then(|t| t.profile.clone()));
            let resolved_region =
                region.or_else(|| target_config.as_ref().and_then(|t| t.region.clone()));

            // Apply SSH settings for generated ~/.ssh/config entry.
            if let Some(t) = &target_config {
                if let Some(user) = &t.ssh_user {
                    effective_config.vscode.ssh_user = Some(user.clone());
                }
                if let Some(identity_file) = &t.ssh_identity_file {
                    effective_config.vscode.ssh_identity_file = Some(identity_file.clone());
                }
                if let Some(identities_only) = t.ssh_identities_only {
                    effective_config.vscode.ssh_identities_only = identities_only;
                }
            }

            info!(
                "Connecting to instance {} on port {}:{}",
                resolved_instance_id, resolved_local_port, resolved_remote_port
            );

            handle_connect_with_recovery(
                resolved_instance_id,
                resolved_local_port,
                resolved_remote_port,
                resolved_remote_host,
                resolved_profile,
                resolved_region,
                priority,
                precheck,
                &effective_config,
                &recovery_manager,
                &message_system,
            )
            .await
        }
        Commands::List => {
            handle_list_with_recovery(&config, &recovery_manager, &message_system).await
        }
        Commands::Terminate { session_id } => {
            handle_terminate_with_recovery(session_id, &config, &recovery_manager, &message_system)
                .await
        }
        Commands::Status { session_id } => handle_status(session_id, &config).await,
        Commands::Tui => handle_tui(&config).await,
        Commands::MultiSession => {
            #[cfg(feature = "multi-session")]
            {
                handle_multi_session(&config).await
            }
            #[cfg(not(feature = "multi-session"))]
            {
                eprintln!("❌ Multi-session functionality is not available. Enable the 'multi-session' feature to use this command.");
                Err(anyhow::anyhow!("Multi-session functionality not available"))
            }
        }
        Commands::Metrics => handle_metrics(&config).await,
        Commands::Resources => handle_resources(&config).await,
        Commands::Health {
            session_id,
            comprehensive,
        } => handle_health(session_id, comprehensive, &config).await,
        Commands::Database { action } => {
            #[cfg(feature = "persistence")]
            {
                handle_database(action, &config).await
            }
            #[cfg(not(feature = "persistence"))]
            {
                let _ = action; // Suppress unused warning
                eprintln!("❌ Database functionality is not available. Enable the 'persistence' feature to use this command.");
                Err(anyhow::anyhow!("Database functionality not available"))
            }
        }
        Commands::Config { action } => handle_config(action, &config).await,
        Commands::VsCode { action } => handle_vscode(action, &config).await,
        Commands::Diagnose { action } => handle_diagnose(action, &config).await,
        Commands::Precheck {
            instance_id,
            local_port,
            profile,
            region,
            timeout,
            format,
            output,
        } => {
            handle_precheck(
                instance_id,
                local_port,
                profile,
                region,
                timeout,
                format,
                output,
                &config,
            )
            .await
        }
        Commands::Fix {
            instance_id,
            local_port,
            remote_port,
            profile,
            region,
            auto_fix,
            safe_only,
            dry_run,
            timeout,
            format,
            output,
        } => {
            handle_fix(
                instance_id,
                local_port,
                remote_port,
                profile,
                region,
                auto_fix,
                safe_only,
                dry_run,
                timeout,
                format,
                output,
                &config,
            )
            .await
        }
    };

    // Handle any errors that occurred during command execution
    if let Err(e) = result {
        let ec2_error = match e.downcast::<NimbusError>() {
            Ok(ec2_err) => ec2_err,
            Err(other_err) => NimbusError::System(other_err.to_string()),
        };

        let context = ErrorContext::new("command_execution", "main");
        let contextual_error = ContextualError::new(ec2_error, context);

        StructuredLogger::log_error(&contextual_error);

        let user_message = message_system.get_error_message(&contextual_error.error);
        eprintln!("{}", user_message.format_for_display());

        return Err(contextual_error.error.into());
    }

    Ok(())
}
