use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

mod auto_fix;
mod aws;
mod aws_config_validator;
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
mod commands;
mod vscode;

use config::Config;
use error::NimbusError;
use error_recovery::{ContextualError, ErrorContext, ErrorRecoveryManager, RecoveryConfig};
use logging::{LoggingConfig, StructuredLogger};
#[cfg(feature = "performance-monitoring")]
use monitor::DefaultSessionMonitor;
#[cfg(feature = "multi-session")]
use multi_session::{MultiSessionManager, ResourceThresholds};
#[cfg(feature = "multi-session")]
use multi_session_ui::MultiSessionUi;
#[cfg(feature = "persistence")]
use persistence::{PersistenceManager, SqlitePersistenceManager};
use targets::TargetsConfig;
use user_messages::UserMessageSystem;
use commands::*;

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

#[derive(Subcommand)]
enum ConfigCommands {
    /// Validate current configuration
    Validate,

    /// Show current configuration
    Show,

    /// Generate example configuration file
    Generate {
        /// Output file path (defaults to standard config directory)
        #[arg(short, long)]
        output: Option<String>,

        /// Configuration format (json, toml)
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Show environment variable help
    EnvHelp,

    /// Test configuration with environment overrides
    Test,
}

#[derive(Subcommand)]
enum VsCodeCommands {
    /// Check VS Code integration status
    Status,

    /// Test VS Code integration
    Test {
        /// Session ID to test with (optional, creates test session if not provided)
        session_id: Option<String>,
    },

    /// Configure VS Code integration
    Setup,

    /// Clean up SSH configuration
    Cleanup {
        /// Session ID to clean up (optional, cleans all if not provided)
        session_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum DiagnosticCommands {
    /// Run comprehensive SSM connection diagnostics
    Full {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// Local port for port forwarding (optional)
        #[arg(long)]
        local_port: Option<u16>,

        /// Remote port on the instance (optional)
        #[arg(long)]
        remote_port: Option<u16>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Run diagnostics in parallel
        #[arg(long, default_value = "true")]
        parallel: bool,

        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Run pre-connection checks
    Precheck {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// Local port for port forwarding (optional)
        #[arg(long)]
        local_port: Option<u16>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,
    },

    /// Run preventive checks before connection attempt
    Preventive {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// Local port for port forwarding (optional)
        #[arg(long)]
        local_port: Option<u16>,

        /// Remote port on the instance (optional)
        #[arg(long)]
        remote_port: Option<u16>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Abort connection on critical issues
        #[arg(long, default_value = "true")]
        abort_on_critical: bool,

        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Run specific diagnostic item
    Item {
        /// Diagnostic item name
        #[arg(short = 't', long)]
        item: String,

        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// Local port for port forwarding (optional)
        #[arg(long)]
        local_port: Option<u16>,

        /// Remote port on the instance (optional)
        #[arg(long)]
        remote_port: Option<u16>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,
    },

    /// List available diagnostic items
    List,

    /// Run comprehensive AWS configuration validation
    AwsConfig {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Include credential validation
        #[arg(long, default_value = "true")]
        include_credentials: bool,

        /// Include IAM permission validation
        #[arg(long, default_value = "true")]
        include_iam: bool,

        /// Include VPC configuration validation
        #[arg(long, default_value = "true")]
        include_vpc: bool,

        /// Include security group validation
        #[arg(long, default_value = "true")]
        include_security_groups: bool,

        /// Minimum compliance score (0-100)
        #[arg(long, default_value = "75.0")]
        minimum_score: f64,
    },

    /// Run integrated AWS configuration validation with cross-validation and caching
    AwsConfigIntegrated {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Include credential validation
        #[arg(long, default_value = "true")]
        include_credentials: bool,

        /// Include IAM permission validation
        #[arg(long, default_value = "true")]
        include_iam: bool,

        /// Include VPC configuration validation
        #[arg(long, default_value = "true")]
        include_vpc: bool,

        /// Include security group validation
        #[arg(long, default_value = "true")]
        include_security_groups: bool,

        /// Minimum compliance score (0-100)
        #[arg(long, default_value = "75.0")]
        minimum_score: f64,

        /// Clear cache before validation
        #[arg(long, default_value = "false")]
        clear_cache: bool,
    },

    /// Run diagnostics with real-time feedback UI
    Interactive {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// Local port for port forwarding (optional)
        #[arg(long)]
        local_port: Option<u16>,

        /// Remote port on the instance (optional)
        #[arg(long)]
        remote_port: Option<u16>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Run diagnostics in parallel
        #[arg(long, default_value = "true")]
        parallel: bool,

        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,

        /// Disable color coding
        #[arg(long)]
        no_color: bool,

        /// Refresh interval in milliseconds
        #[arg(long, default_value = "100")]
        refresh_interval: u64,
    },

    /// Manage diagnostic settings
    Settings {
        #[command(subcommand)]
        action: DiagnosticSettingsCommands,
    },
}

#[derive(Subcommand)]
enum DiagnosticSettingsCommands {
    /// Show current diagnostic settings
    Show,

    /// Enable a diagnostic check
    Enable {
        /// Diagnostic check name
        check_name: String,
    },

    /// Disable a diagnostic check
    Disable {
        /// Diagnostic check name
        check_name: String,
    },

    /// Set auto-fix mode
    AutoFix {
        /// Enable auto-fix
        #[arg(long)]
        enable: bool,

        /// Safe fixes only
        #[arg(long)]
        safe_only: bool,
    },

    /// Set parallel execution mode
    Parallel {
        /// Enable parallel execution
        enable: bool,
    },

    /// Set default timeout
    Timeout {
        /// Timeout in seconds
        seconds: u64,
    },

    /// Set report format
    Format {
        /// Report format (text, json, yaml)
        format: String,
    },

    /// Reset to default settings
    Reset,
}

#[derive(Subcommand)]
enum DatabaseCommands {
    /// Initialize database
    Init,

    /// Show database information
    Info,

    /// List stored sessions
    Sessions,

    /// Show performance statistics
    Stats {
        /// Session ID (optional, shows all if not specified)
        session_id: Option<String>,
    },

    /// Clean up old data
    Cleanup {
        /// Retention period in days
        #[arg(short, long, default_value = "30")]
        days: u32,
    },

    /// Export data
    Export {
        /// Output file path
        #[arg(short, long)]
        output: String,

        /// Export format (json, csv)
        #[arg(short, long, default_value = "json")]
        format: String,
    },
}

#[derive(Subcommand)]
enum Commands {
    /// Connect to an EC2 instance
    Connect {
        /// Target name from targets file (optional)
        #[arg(long)]
        target: Option<String>,

        /// Targets file path (optional; defaults to ~/.config/nimbus/targets.json)
        #[arg(long)]
        targets_file: Option<String>,

        /// EC2 instance ID (required if --target is not specified)
        #[arg(short, long)]
        instance_id: Option<String>,

        /// Local port for port forwarding
        #[arg(short, long)]
        local_port: Option<u16>,

        /// Remote port on the instance
        #[arg(short, long)]
        remote_port: Option<u16>,

        /// Remote host for port forwarding through the instance (uses AWS-StartPortForwardingSessionToRemoteHost)
        #[arg(long)]
        remote_host: Option<String>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Session priority (low, normal, high, critical)
        #[arg(long, default_value = "normal")]
        priority: String,

        /// Run preventive checks before connection
        #[arg(long)]
        precheck: bool,
    },

    /// List active sessions
    List,

    /// Terminate a session
    Terminate {
        /// Session ID to terminate
        session_id: String,
    },

    /// Show session status
    Status {
        /// Session ID (optional, shows all if not specified)
        session_id: Option<String>,
    },

    /// Launch interactive terminal UI
    Tui,

    /// Launch multi-session management UI
    MultiSession,

    /// Show performance metrics
    Metrics,

    /// Show resource usage and efficiency
    Resources,

    /// Perform health check
    Health {
        /// Session ID to check (optional, checks system health if not specified)
        session_id: Option<String>,

        /// Perform comprehensive health check
        #[arg(short, long)]
        comprehensive: bool,
    },

    /// Database management commands
    Database {
        #[command(subcommand)]
        action: DatabaseCommands,
    },

    /// Configuration management commands
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },

    /// VS Code integration commands
    VsCode {
        #[command(subcommand)]
        action: VsCodeCommands,
    },

    /// SSM connection diagnostics
    Diagnose {
        #[command(subcommand)]
        action: DiagnosticCommands,
    },

    /// Run pre-connection checks
    Precheck {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// Local port for port forwarding (optional)
        #[arg(long)]
        local_port: Option<u16>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Timeout in seconds
        #[arg(long, default_value = "15")]
        timeout: u64,

        /// Output format (text, json, yaml)
        #[arg(long, default_value = "text")]
        format: String,

        /// Output file path (optional)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Run automatic fixes for detected issues
    Fix {
        /// EC2 instance ID
        #[arg(short, long)]
        instance_id: String,

        /// Local port for port forwarding (optional)
        #[arg(long)]
        local_port: Option<u16>,

        /// Remote port on the instance (optional)
        #[arg(long)]
        remote_port: Option<u16>,

        /// AWS profile to use
        #[arg(short, long)]
        profile: Option<String>,

        /// AWS region
        #[arg(long)]
        region: Option<String>,

        /// Execute confirmation-required fixes automatically (non-interactive mode)
        #[arg(long)]
        auto_fix: bool,

        /// Only run safe fixes (low risk)
        #[arg(long)]
        safe_only: bool,

        /// Dry run - show what would be fixed without executing
        #[arg(long)]
        dry_run: bool,

        /// Timeout in seconds
        #[arg(long, default_value = "60")]
        timeout: u64,

        /// Output format (text, json, yaml)
        #[arg(long, default_value = "text")]
        format: String,

        /// Output file path (optional)
        #[arg(short, long)]
        output: Option<String>,
    },
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
        console_enabled: true,
        file_enabled: true,
        log_dir: std::path::PathBuf::from("logs"),
        file_prefix: "nimbus".to_string(),
        rotation: "daily".to_string(),
        max_files: 7,
        json_format: false,
        performance_tracing: cli.verbose,
    };

    if let Err(e) = crate::logging::init_logging(&logging_config) {
        eprintln!("Failed to initialize logging: {}", e);
        return Err(anyhow::anyhow!("Failed to initialize logging: {}", e));
    }

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
            let resolved_remote_host = remote_host
                .or_else(|| target_config.as_ref().and_then(|t| t.remote_host.clone()));

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

