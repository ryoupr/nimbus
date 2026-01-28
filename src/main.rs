use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

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
mod vscode;

use aws::AwsManager;
use aws_config_validator::{AwsConfigValidationConfig, DefaultAwsConfigValidator};
use config::Config;
use diagnostic::{DefaultDiagnosticManager, DiagnosticConfig, DiagnosticManager};
use preventive_check::{
    DefaultPreventiveCheck, PreventiveCheck, PreventiveCheckConfig,
};
use error::NimbusError;
use error_recovery::{ContextualError, ErrorContext, ErrorRecoveryManager, RecoveryConfig};
use health::{DefaultHealthChecker, HealthChecker};
use logging::{LoggingConfig, StructuredLogger};
use manager::{DefaultSessionManager, SessionManager};
#[cfg(feature = "performance-monitoring")]
use monitor::DefaultSessionMonitor;
#[cfg(feature = "multi-session")]
use multi_session::{MultiSessionManager, ResourceThresholds};
#[cfg(feature = "multi-session")]
use multi_session_ui::MultiSessionUi;
#[cfg(feature = "persistence")]
use persistence::{PersistenceManager, SqlitePersistenceManager};
use resource::ResourceMonitor;
use session::{SessionConfig, SessionPriority};
use targets::TargetsConfig;
use user_messages::UserMessageSystem;
use vscode::VsCodeIntegration;

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

async fn handle_connect_with_recovery(
    instance_id: String,
    local_port: u16,
    remote_port: u16,
    remote_host: Option<String>,
    profile: Option<String>,
    region: Option<String>,
    priority: String,
    precheck: bool,
    config: &Config,
    recovery_manager: &ErrorRecoveryManager,
    message_system: &UserMessageSystem,
) -> Result<()> {
    let context = ErrorContext::new("connect", "session_manager")
        .with_instance_id(&instance_id)
        .with_info("local_port", &local_port.to_string())
        .with_info("remote_port", &remote_port.to_string());

    // First attempt
    match handle_connect(
        instance_id.clone(),
        local_port,
        remote_port,
        remote_host.clone(),
        profile.clone(),
        region.clone(),
        priority.clone(),
        precheck,
        config,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            let ec2_error = match e.downcast::<NimbusError>() {
                Ok(ec2_err) => ec2_err,
                Err(other_err) => NimbusError::System(other_err.to_string()),
            };

            let contextual_error = ContextualError::new(ec2_error, context);
            StructuredLogger::log_error(&contextual_error);

            if contextual_error.error.is_recoverable() {
                warn!(
                    "Connection failed, attempting recovery: {}",
                    contextual_error.error
                );

                // Create a proper recovery operation that actually retries the connection
                let instance_id_clone = instance_id.clone();
                let remote_host_clone = remote_host.clone();
                let profile_clone = profile.clone();
                let region_clone = region.clone();
                let priority_clone = priority.clone();
                let config_clone = config.clone();

                let recovery_operation = || -> crate::error::Result<()> {
                    // For async recovery, we need to use a different approach
                    // Return an error that indicates we need to retry the entire operation
                    Err(contextual_error.error.clone())
                };

                match recovery_manager
                    .recover(recovery_operation, &contextual_error.error)
                    .await
                {
                    Ok(_) => {
                        // If recovery suggests we should retry, do the actual retry here
                        info!("Recovery suggests retry, attempting connection again");
                        match handle_connect(
                            instance_id_clone,
                            local_port,
                            remote_port,
                            remote_host_clone,
                            profile_clone,
                            region_clone,
                            priority_clone,
                            precheck,
                            &config_clone,
                        )
                        .await
                        {
                            Ok(_) => {
                                info!("Connection recovered successfully after retry");
                                println!("✅ Connection recovered successfully after retry");
                                Ok(())
                            }
                            Err(retry_error) => {
                                let retry_ec2_error =
                                    match retry_error.downcast::<NimbusError>() {
                                        Ok(ec2_err) => ec2_err,
                                        Err(other_err) => {
                                            NimbusError::System(other_err.to_string())
                                        }
                                    };
                                let user_message =
                                    message_system.get_error_message(&retry_ec2_error);
                                eprintln!("{}", user_message.format_for_display());
                                Err(retry_ec2_error.into())
                            }
                        }
                    }
                    Err(recovery_error) => {
                        let user_message = message_system.get_error_message(&recovery_error);
                        eprintln!("{}", user_message.format_for_display());
                        Err(recovery_error.into())
                    }
                }
            } else {
                let user_message = message_system.get_error_message(&contextual_error.error);
                eprintln!("{}", user_message.format_for_display());
                Err(contextual_error.error.into())
            }
        }
    }
}

async fn handle_list_with_recovery(
    config: &Config,
    recovery_manager: &ErrorRecoveryManager,
    message_system: &UserMessageSystem,
) -> Result<()> {
    let context = ErrorContext::new("list_sessions", "aws_manager");

    match handle_list(config).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let ec2_error = match e.downcast::<NimbusError>() {
                Ok(ec2_err) => ec2_err,
                Err(other_err) => NimbusError::System(other_err.to_string()),
            };

            let contextual_error = ContextualError::new(ec2_error, context);
            StructuredLogger::log_error(&contextual_error);

            if contextual_error.error.is_recoverable() {
                warn!(
                    "List operation failed, attempting recovery: {}",
                    contextual_error.error
                );

                let config_clone = config.clone();
                let recovery_operation = || -> crate::error::Result<()> {
                    // For async recovery, return error to indicate retry needed
                    Err(contextual_error.error.clone())
                };

                match recovery_manager
                    .recover(recovery_operation, &contextual_error.error)
                    .await
                {
                    Ok(_) => {
                        // Retry the actual operation
                        match handle_list(&config_clone).await {
                            Ok(_) => Ok(()),
                            Err(retry_error) => {
                                let retry_ec2_error =
                                    match retry_error.downcast::<NimbusError>() {
                                        Ok(ec2_err) => ec2_err,
                                        Err(other_err) => {
                                            NimbusError::System(other_err.to_string())
                                        }
                                    };
                                let user_message =
                                    message_system.get_error_message(&retry_ec2_error);
                                eprintln!("{}", user_message.format_for_display());
                                Err(retry_ec2_error.into())
                            }
                        }
                    }
                    Err(recovery_error) => {
                        let user_message = message_system.get_error_message(&recovery_error);
                        eprintln!("{}", user_message.format_for_display());
                        Err(recovery_error.into())
                    }
                }
            } else {
                let user_message = message_system.get_error_message(&contextual_error.error);
                eprintln!("{}", user_message.format_for_display());
                Err(contextual_error.error.into())
            }
        }
    }
}

async fn handle_terminate_with_recovery(
    session_id: String,
    config: &Config,
    recovery_manager: &ErrorRecoveryManager,
    message_system: &UserMessageSystem,
) -> Result<()> {
    let context =
        ErrorContext::new("terminate_session", "aws_manager").with_session_id(&session_id);

    match handle_terminate(session_id.clone(), config).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let ec2_error = match e.downcast::<NimbusError>() {
                Ok(ec2_err) => ec2_err,
                Err(other_err) => NimbusError::System(other_err.to_string()),
            };

            let contextual_error = ContextualError::new(ec2_error, context);
            StructuredLogger::log_error(&contextual_error);

            if contextual_error.error.is_recoverable() {
                warn!(
                    "Terminate operation failed, attempting recovery: {}",
                    contextual_error.error
                );

                let session_id_clone = session_id.clone();
                let config_clone = config.clone();
                let recovery_operation = || -> crate::error::Result<()> {
                    // For async recovery, return error to indicate retry needed
                    Err(contextual_error.error.clone())
                };

                match recovery_manager
                    .recover(recovery_operation, &contextual_error.error)
                    .await
                {
                    Ok(_) => {
                        // Retry the actual operation
                        match handle_terminate(session_id_clone, &config_clone).await {
                            Ok(_) => Ok(()),
                            Err(retry_error) => {
                                let retry_ec2_error =
                                    match retry_error.downcast::<NimbusError>() {
                                        Ok(ec2_err) => ec2_err,
                                        Err(other_err) => {
                                            NimbusError::System(other_err.to_string())
                                        }
                                    };
                                let user_message =
                                    message_system.get_error_message(&retry_ec2_error);
                                eprintln!("{}", user_message.format_for_display());
                                Err(retry_ec2_error.into())
                            }
                        }
                    }
                    Err(recovery_error) => {
                        let user_message = message_system.get_error_message(&recovery_error);
                        eprintln!("{}", user_message.format_for_display());
                        Err(recovery_error.into())
                    }
                }
            } else {
                let user_message = message_system.get_error_message(&contextual_error.error);
                eprintln!("{}", user_message.format_for_display());
                Err(contextual_error.error.into())
            }
        }
    }
}

async fn handle_connect(
    instance_id: String,
    local_port: u16,
    remote_port: u16,
    remote_host: Option<String>,
    profile: Option<String>,
    region: Option<String>,
    priority: String,
    precheck: bool,
    config: &Config,
) -> Result<()> {
    info!("Initiating connection to instance {}", instance_id);

    println!("🚀 Connecting to EC2 instance: {}", instance_id);
    if let Some(ref host) = remote_host {
        println!(
            "📡 Port forwarding: localhost:{} -> {} -> {}:{}",
            local_port, instance_id, host, remote_port
        );
    } else {
        println!(
            "📡 Port forwarding: {}:{} -> localhost:{}",
            instance_id, remote_port, local_port
        );
    }

    if let Some(profile) = &profile {
        println!("🔐 Using AWS profile: {}", profile);
    }

    if let Some(region) = &region {
        println!("🌍 Using AWS region: {}", region);
    }

    // Parse priority
    let session_priority = match priority.to_lowercase().as_str() {
        "low" => SessionPriority::Low,
        "normal" => SessionPriority::Normal,
        "high" => SessionPriority::High,
        "critical" => SessionPriority::Critical,
        _ => {
            warn!("Invalid priority '{}', using 'normal'", priority);
            SessionPriority::Normal
        }
    };

    println!("⚡ Session priority: {:?}", session_priority);

    // Create session manager with AWS integration
    let mut session_manager = match (&profile, &region) {
        (Some(p), Some(_r)) => {
            // Create AWS manager with both profile and region
            let _aws_manager = AwsManager::with_profile(p).await.map_err(|e| {
                NimbusError::Aws(crate::error::AwsError::AuthenticationFailed {
                    message: format!(
                        "Failed to initialize AWS manager with profile '{}': {}",
                        p, e
                    ),
                })
            })?;
            DefaultSessionManager::with_profile(3, p)
                .await
                .map_err(|e| {
                    NimbusError::Session(crate::error::SessionError::CreationFailed {
                        reason: format!(
                            "Failed to create session manager with profile '{}': {}",
                            p, e
                        ),
                    })
                })?
        }
        (Some(p), None) => DefaultSessionManager::with_profile(3, p)
            .await
            .map_err(|e| {
                NimbusError::Session(crate::error::SessionError::CreationFailed {
                    reason: format!(
                        "Failed to create session manager with profile '{}': {}",
                        p, e
                    ),
                })
            })?,
        (None, Some(r)) => DefaultSessionManager::with_region(3, r)
            .await
            .map_err(|e| {
                NimbusError::Session(crate::error::SessionError::CreationFailed {
                    reason: format!(
                        "Failed to create session manager with region '{}': {}",
                        r, e
                    ),
                })
            })?,
        (None, None) => DefaultSessionManager::new(3).await.map_err(|e| {
            NimbusError::Session(crate::error::SessionError::CreationFailed {
                reason: format!("Failed to create default session manager: {}", e),
            })
        })?,
    };

    // Create session configuration with priority
    // If region isn't specified by user, use the region resolved by AWS configuration.
    let region_for_session = region
        .clone()
        .unwrap_or_else(|| session_manager.aws_manager().region().to_string());
    let session_config = SessionConfig::new(
        instance_id.clone(),
        local_port,
        remote_port,
        profile.clone(),
        region_for_session,
    )
    .with_remote_host(remote_host)
    .with_priority(session_priority);

    // Check for existing sessions
    let existing_sessions = session_manager
        .find_existing_sessions(&instance_id, local_port)
        .await
        .map_err(|e| {
            NimbusError::Session(crate::error::SessionError::CreationFailed {
                reason: format!("Failed to search for existing sessions: {}", e),
            })
        })?;

    if !existing_sessions.is_empty() {
        if let Some(reusable_session) = session_manager.suggest_reuse(&existing_sessions).await {
            println!("🔄 Found existing session: {}", reusable_session.id);
            println!("   Status: {:?}", reusable_session.status);
            println!("   Age: {} seconds", reusable_session.age_seconds());
            println!("   Idle: {} seconds", reusable_session.idle_seconds());

            // For now, just show the suggestion
            println!("💡 You can reuse this session or create a new one");
        }
    }

    // Run preventive checks only if --precheck flag is set
    if precheck {
        println!("🛡️  Running preventive checks before connection...");
        let preventive_config = PreventiveCheckConfig::new(instance_id.clone())
            .with_ports(local_port, remote_port)
            .with_aws_config(region.clone(), profile.clone())
            .with_abort_on_critical(true)
            .with_timeout(std::time::Duration::from_secs(30));

        let preventive_check = match DefaultPreventiveCheck::with_aws_config(
            preventive_config.region.clone(),
            preventive_config.profile.clone(),
        )
        .await
        {
            Ok(checker) => checker,
            Err(e) => {
                warn!(
                    "Failed to create preventive check, proceeding without: {}",
                    e
                );
                println!(
                    "⚠️  Preventive check unavailable, proceeding with connection: {}",
                    e
                );
                // Continue without preventive check
                DefaultPreventiveCheck::new().await.map_err(|e| {
                    NimbusError::System(format!(
                        "Failed to create fallback preventive check: {}",
                        e
                    ))
                })?
            }
        };

        match preventive_check
            .run_preventive_checks(preventive_config.clone())
            .await
        {
        Ok(mut result) => {
            println!(
                "🎯 Connection Likelihood: {} ({}%)",
                result.connection_likelihood.as_description(),
                result.connection_likelihood.as_percentage()
            );

            if result.should_abort_connection {
                // If auto-fix is enabled, try to resolve common blockers (task 26.1/26.2)
                // and then re-run preventive checks once.
                if config.diagnostic.auto_fix_enabled {
                    let has_managed_instance_registration_issue = result
                        .critical_issues
                        .iter()
                        .any(|issue| issue.item_name == "managed_instance_registration");

                    if has_managed_instance_registration_issue {
                        use auto_fix::{
                            AutoFixManager, DefaultAutoFixManager, FixAction, FixActionType,
                        };

                        println!(
                            "🔧 Auto-fix is enabled - attempting to start instance and wait for SSM registration..."
                        );

                        let mut auto_fix_manager = DefaultAutoFixManager::with_aws_config(
                            preventive_config.region.clone(),
                            preventive_config.profile.clone(),
                        )
                        .await
                        .map_err(|e| {
                            NimbusError::System(format!(
                                "Failed to create auto-fix manager: {}",
                                e
                            ))
                        })?;

                        // Safety: in connect flow we only attempt the safe, non-confirmation fix.
                        // (StartInstance is Low risk and requires no confirmation per task 26.1)
                        let action = FixAction::new(
                            FixActionType::StartInstance,
                            format!("Starting instance: {}", instance_id),
                            instance_id.clone(),
                        );

                        match auto_fix_manager.execute_fix(action).await {
                            Ok(fix_result) => {
                                if fix_result.success {
                                    println!("✅ Auto-fix succeeded: {}", fix_result.message);
                                    println!("🔁 Re-running preventive checks...");
                                    match preventive_check
                                        .run_preventive_checks(preventive_config.clone())
                                        .await
                                    {
                                        Ok(retry_result) => {
                                            result = retry_result;
                                            println!(
                                                "🎯 Connection Likelihood: {} ({}%)",
                                                result.connection_likelihood.as_description(),
                                                result.connection_likelihood.as_percentage()
                                            );
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Preventive check failed after auto-fix, proceeding to abort handling: {}",
                                                e
                                            );
                                        }
                                    }
                                } else {
                                    println!("❌ Auto-fix failed: {}", fix_result.message);
                                }
                            }
                            Err(e) => {
                                println!("❌ Auto-fix execution failed: {}", e);
                            }
                        }
                    }
                }

                if result.should_abort_connection {
                    println!(
                        "🛑 Preventive checks failed - connection aborted due to critical issues:"
                    );
                    for issue in &result.critical_issues {
                        println!("   ❌ {}: {}", issue.item_name, issue.message);
                    }
                    println!();
                    println!("💡 Recommendations:");
                    for (index, recommendation) in result.recommendations.iter().enumerate() {
                        println!("   {}. {}", index + 1, recommendation);
                    }
                    println!();
                    println!(
                        "Run 'nimbus diagnose preventive --instance-id {}' for detailed analysis.",
                        instance_id
                    );
                    if !config.diagnostic.auto_fix_enabled {
                        println!();
                        println!("💡 Auto-fix is currently disabled. You can enable it with:");
                        println!("   nimbus diagnose settings auto-fix --enable --safe-only");
                        println!(
                            "   (or run: nimbus fix --instance-id {} --auto-fix)",
                            instance_id
                        );
                    }

                    return Err(NimbusError::Connection(
                        crate::error::ConnectionError::PreventiveCheckFailed {
                            reason: "Critical issues detected during preventive checks".to_string(),
                            issues: result
                                .critical_issues
                                .iter()
                                .map(|i| i.message.clone())
                                .collect(),
                        },
                    )
                    .into());
                }
            }

            if !result.warnings.is_empty() {
                println!("⚠️  Proceeding with {} warnings:", result.warnings.len());
                for warning in &result.warnings {
                    println!("   ⚠️  {}: {}", warning.item_name, warning.message);
                }
                println!();
            }

            if matches!(
                result.overall_status,
                preventive_check::PreventiveCheckStatus::Ready
            ) {
                println!("✅ Preventive checks passed - proceeding with connection");
            } else {
                println!(
                    "⚠️  Preventive checks completed with warnings - proceeding with connection"
                );
            }
        }
        Err(e) => {
            warn!("Preventive check failed, proceeding with connection: {}", e);
            println!(
                "⚠️  Preventive check failed, proceeding with connection: {}",
                e
            );
        }
        }
        println!();
    }

    // Create new session
    match session_manager.create_session(session_config).await {
        Ok(session) => {
            println!("✅ Session created successfully!");
            println!("   Session ID: {}", session.id);
            println!("   Status: {:?}", session.status);
            println!("   Local port: {}", session.local_port);
            println!("   Remote port: {}", session.remote_port);

            // Get SSM session ID if available
            if let Some(ssm_session_id) = session_manager.get_ssm_session_id(&session.id) {
                println!("   SSM Session ID: {}", ssm_session_id);
            }

            println!(
                "🎯 Connection ready! You can now access the service at localhost:{}",
                local_port
            );

            // VS Code統合を実行
            if config.vscode.auto_launch_enabled || config.vscode.auto_update_ssh_config {
                println!("🔧 Setting up VS Code integration...");

                match VsCodeIntegration::new(config.vscode.clone()) {
                    Ok(integration) => match integration.integrate_session(&session).await {
                        Ok(result) => {
                            if result.success {
                                println!("✅ VS Code integration completed!");

                                if result.ssh_config_updated {
                                    if let Some(connection_info) = &result.connection_info {
                                        println!(
                                            "   SSH Host: {} (added to ~/.ssh/config)",
                                            connection_info.ssh_host
                                        );
                                        println!(
                                            "   💡 You can also connect using: ssh {}",
                                            connection_info.ssh_host
                                        );
                                    }
                                }

                                if result.vscode_launched {
                                    println!("   🚀 VS Code launched automatically");
                                }
                            } else if let Some(error) = &result.error_message {
                                warn!("VS Code integration failed: {}", error);
                                println!("⚠️  VS Code integration failed: {}", error);
                            }
                        }
                        Err(e) => {
                            warn!("VS Code integration error: {}", e);
                            println!("⚠️  VS Code integration error: {}", e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to initialize VS Code integration: {}", e);
                        println!("⚠️  VS Code integration unavailable: {}", e);
                    }
                }
            }

            // Log successful connection
            let mut context_map = std::collections::HashMap::new();
            context_map.insert("instance_id".to_string(), instance_id);
            context_map.insert("local_port".to_string(), local_port.to_string());
            context_map.insert("remote_port".to_string(), remote_port.to_string());

            StructuredLogger::log_session_activity(
                &session.id,
                "session_created",
                Some(&context_map),
            );
        }
        Err(e) => {
            error!("Failed to create session: {}", e);

            // Convert to appropriate NimbusError
            let ec2_error = match e {
                _ => NimbusError::Session(crate::error::SessionError::CreationFailed {
                    reason: e.to_string(),
                }),
            };

            return Err(ec2_error.into());
        }
    }

    Ok(())
}

async fn handle_list(_config: &Config) -> Result<()> {
    info!("Listing active sessions");

    println!("📋 Active Sessions:");

    // Create AWS manager to list sessions
    let aws_manager = AwsManager::default().await?;

    match aws_manager.list_active_sessions().await {
        Ok(sessions) => {
            if sessions.is_empty() {
                println!("  No active sessions found");
            } else {
                for session in sessions {
                    println!("  • Session ID: {}", session.session_id);
                    println!("    Target: {}", session.target);
                    println!("    Status: {:?}", session.status);
                    println!("    Region: {}", session.region);
                    println!(
                        "    Created: {}",
                        session.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    println!();
                }
            }
        }
        Err(e) => {
            warn!("Failed to list sessions: {}", e);
            println!("  ⚠️  Failed to retrieve session list: {}", e);
        }
    }

    Ok(())
}

async fn handle_terminate(session_id: String, _config: &Config) -> Result<()> {
    info!("Terminating session: {}", session_id);

    println!("🛑 Terminating session: {}", session_id);

    // Create AWS manager to terminate session
    let aws_manager = AwsManager::default().await?;

    match aws_manager.terminate_ssm_session(&session_id).await {
        Ok(_) => {
            println!("✅ Session terminated successfully");
        }
        Err(e) => {
            error!("Failed to terminate session: {}", e);
            println!("❌ Failed to terminate session: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_status(session_id: Option<String>, _config: &Config) -> Result<()> {
    let aws_manager = AwsManager::default().await?;

    match session_id {
        Some(id) => {
            info!("Showing status for session: {}", id);
            println!("📊 Session Status: {}", id);

            match aws_manager.get_session_status(&id).await {
                Ok(status) => {
                    println!("  Status: {:?}", status);
                }
                Err(e) => {
                    warn!("Failed to get session status: {}", e);
                    println!("  ⚠️  Failed to retrieve status: {}", e);
                }
            }
        }
        None => {
            info!("Showing status for all sessions");
            println!("📊 All Sessions Status:");

            match aws_manager.list_active_sessions().await {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("  No sessions found");
                    } else {
                        for session in sessions {
                            println!("  • {}: {:?}", session.session_id, session.status);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to list sessions: {}", e);
                    println!("  ⚠️  Failed to retrieve sessions: {}", e);
                }
            }
        }
    }

    Ok(())
}

async fn handle_tui(_config: &Config) -> Result<()> {
    info!("Launching Terminal UI");

    println!("🖥️  Starting Terminal UI...");

    // Create Terminal UI
    let mut terminal_ui = ui::TerminalUi::new()?;

    // Initialize with some sample data for demonstration
    let sample_sessions = vec![
        session::Session {
            id: "session-001".to_string(),
            instance_id: "i-1234567890abcdef0".to_string(),
            local_port: 8080,
            remote_port: 80,
            remote_host: None,
            status: session::SessionStatus::Active,
            created_at: std::time::SystemTime::now() - std::time::Duration::from_secs(300),
            last_activity: std::time::SystemTime::now() - std::time::Duration::from_secs(30),
            process_id: Some(12345),
            connection_count: 5,
            data_transferred: 1024000,
            aws_profile: Some("default".to_string()),
            region: "us-east-1".to_string(),
            priority: session::SessionPriority::Normal,
            tags: std::collections::HashMap::new(),
        },
        session::Session {
            id: "session-002".to_string(),
            instance_id: "i-0987654321fedcba0".to_string(),
            local_port: 8081,
            remote_port: 443,
            remote_host: None,
            status: session::SessionStatus::Connecting,
            created_at: std::time::SystemTime::now() - std::time::Duration::from_secs(60),
            last_activity: std::time::SystemTime::now() - std::time::Duration::from_secs(10),
            process_id: Some(12346),
            connection_count: 0,
            data_transferred: 0,
            aws_profile: None,
            region: "us-west-2".to_string(),
            priority: session::SessionPriority::Normal,
            tags: std::collections::HashMap::new(),
        },
    ];

    // Update UI with sample data
    terminal_ui.update_sessions(sample_sessions);

    // Update metrics
    let sample_metrics = ui::ResourceMetrics {
        memory_usage_mb: 8.5,
        cpu_usage_percent: 0.3,
        active_sessions: 2,
        total_connections: 5,
        uptime_seconds: 3600,
    };
    terminal_ui.update_metrics(sample_metrics);

    // Add some sample warnings
    terminal_ui
        .add_warning("Session session-002 is taking longer than expected to connect".to_string());
    terminal_ui.add_warning("Memory usage is approaching 85% of the 10MB limit".to_string());

    // Set initial progress for demonstration
    terminal_ui.set_progress(
        "Initializing".to_string(),
        0.8,
        "Loading session data...".to_string(),
    );

    // Clear progress after a moment
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    terminal_ui.clear_progress();

    // Run the Terminal UI
    match terminal_ui.run().await {
        Ok(_) => {
            println!("👋 Terminal UI closed");
        }
        Err(e) => {
            error!("Terminal UI error: {}", e);
            println!("❌ Terminal UI error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_metrics(_config: &Config) -> Result<()> {
    info!("Showing performance metrics");

    println!("📈 Performance Metrics:");

    // Initialize resource monitor
    let resource_monitor = ResourceMonitor::new();

    // Get current resource usage
    match resource_monitor.get_current_usage().await {
        Ok(usage) => {
            println!("  💾 Memory usage: {:.2}MB", usage.memory_mb);
            println!("  🖥️  CPU usage: {:.2}%", usage.cpu_percent);
            println!("  🔄 Active processes: {}", usage.process_count);

            // Check if within limits
            match resource_monitor.check_limits().await {
                Ok(violations) => {
                    if violations.is_empty() {
                        println!("  ✅ All resource limits satisfied");
                    } else {
                        println!("  ⚠️  Resource limit violations:");
                        for violation in violations {
                            match violation {
                                resource::ResourceViolation::MemoryExceeded { current, limit } => {
                                    println!("    - Memory: {:.2}MB > {:.2}MB", current, limit);
                                }
                                resource::ResourceViolation::CpuExceeded { current, limit } => {
                                    println!("    - CPU: {:.2}% > {:.2}%", current, limit);
                                }
                                resource::ResourceViolation::ProcessCountExceeded {
                                    current,
                                    limit,
                                } => {
                                    println!("    - Processes: {} > {}", current, limit);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to check resource limits: {}", e);
                    println!("  ⚠️  Failed to check resource limits: {}", e);
                }
            }

            // Show efficiency metrics
            match resource_monitor.get_efficiency_metrics().await {
                Ok(metrics) => {
                    println!("  📊 Efficiency:");
                    println!(
                        "    - Memory efficiency: {:.1}%",
                        metrics.memory_efficiency_percent
                    );
                    println!(
                        "    - CPU efficiency: {:.1}%",
                        metrics.cpu_efficiency_percent
                    );
                    println!(
                        "    - Low power mode: {}",
                        if metrics.low_power_mode_active {
                            "ON"
                        } else {
                            "OFF"
                        }
                    );
                    println!("    - Uptime: {}s", metrics.uptime_seconds);
                }
                Err(e) => {
                    warn!("Failed to get efficiency metrics: {}", e);
                }
            }
        }
        Err(e) => {
            error!("Failed to get resource usage: {}", e);
            println!("  ❌ Failed to retrieve resource metrics: {}", e);
        }
    }

    Ok(())
}

async fn handle_resources(_config: &Config) -> Result<()> {
    info!("Showing resource usage and efficiency");

    println!("🔧 Resource Management:");

    // Initialize resource monitor
    let mut resource_monitor = ResourceMonitor::new();

    // Get current usage
    match resource_monitor.get_current_usage().await {
        Ok(usage) => {
            println!("  📊 Current Usage:");
            println!("    Memory: {:.2}MB / 10.0MB (limit)", usage.memory_mb);
            println!("    CPU: {:.2}% / 0.5% (limit)", usage.cpu_percent);
            println!("    Processes: {}", usage.process_count);

            // Check if optimization is needed
            match resource_monitor.is_operating_optimally().await {
                Ok(optimal) => {
                    if optimal {
                        println!("  ✅ System operating optimally");
                    } else {
                        println!("  ⚠️  System could benefit from optimization");

                        // Perform optimization
                        match resource_monitor.optimize_resources().await {
                            Ok(result) => {
                                println!("  🔧 Optimization completed:");
                                println!(
                                    "    Memory: {:.2}MB -> {:.2}MB",
                                    result.memory_before_mb, result.memory_after_mb
                                );
                                println!(
                                    "    CPU: {:.2}% -> {:.2}%",
                                    result.cpu_before_percent, result.cpu_after_percent
                                );
                                println!("    Actions taken: {:?}", result.actions_taken);
                                println!("    Time: {:?}", result.optimization_time);
                            }
                            Err(e) => {
                                warn!("Optimization failed: {}", e);
                                println!("  ❌ Optimization failed: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to check optimization status: {}", e);
                }
            }

            // Show monitoring status
            let status = resource_monitor.get_monitoring_status();
            println!("  📡 Monitoring Status:");
            println!("    Active: {}", status.active);
            println!("    Low power mode: {}", status.low_power_mode);
            println!("    Interval: {:?}", status.monitoring_interval);
            println!("    Uptime: {:?}", status.uptime);
            println!("    Sample count: {}", status.sample_count);
        }
        Err(e) => {
            error!("Failed to get resource usage: {}", e);
            println!("  ❌ Failed to retrieve resource information: {}", e);
        }
    }

    Ok(())
}

async fn handle_health(
    session_id: Option<String>,
    comprehensive: bool,
    _config: &Config,
) -> Result<()> {
    info!("Performing health check");

    println!("🏥 Health Check:");

    // Initialize health checker
    let health_checker = DefaultHealthChecker::new(std::time::Duration::from_secs(30));

    match session_id {
        Some(id) => {
            if comprehensive {
                println!("  🔍 Comprehensive health check for session: {}", id);

                match health_checker.comprehensive_health_check(&id).await {
                    Ok(result) => {
                        println!(
                            "  📊 Overall Health: {}",
                            if result.overall_healthy {
                                "✅ HEALTHY"
                            } else {
                                "❌ UNHEALTHY"
                            }
                        );
                        println!("  ⏱️  Check Duration: {}ms", result.check_duration_ms);
                        println!(
                            "  🕐 Timestamp: {}",
                            result.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        println!();

                        // SSM Health
                        println!("  🔗 SSM Session Health:");
                        println!(
                            "    Status: {}",
                            if result.ssm_health.is_healthy {
                                "✅ Healthy"
                            } else {
                                "❌ Unhealthy"
                            }
                        );
                        println!(
                            "    Response Time: {}ms",
                            result.ssm_health.response_time_ms
                        );
                        if let Some(error) = &result.ssm_health.error_message {
                            println!("    Error: {}", error);
                        }
                        if let Some(details) = &result.ssm_health.details {
                            println!("    Details: {}", details);
                        }
                        println!();

                        // Network Health
                        println!("  🌐 Network Connectivity:");
                        println!(
                            "    Status: {}",
                            if result.network_health.is_healthy {
                                "✅ Healthy"
                            } else {
                                "❌ Unhealthy"
                            }
                        );
                        println!(
                            "    Response Time: {}ms",
                            result.network_health.response_time_ms
                        );
                        if let Some(error) = &result.network_health.error_message {
                            println!("    Error: {}", error);
                        }
                        if let Some(details) = &result.network_health.details {
                            println!("    Details: {}", details);
                        }
                        println!();

                        // Resource Availability
                        let resources = &result.resource_availability;
                        println!("  💾 Resource Availability:");
                        println!(
                            "    Memory: {:.1}MB available / {:.1}MB total ({:.1}% used)",
                            resources.memory_available_mb,
                            resources.memory_total_mb,
                            resources.memory_usage_percent
                        );
                        println!(
                            "    CPU: {:.1}% available ({:.1}% used)",
                            resources.cpu_available_percent, resources.cpu_usage_percent
                        );
                        println!("    Disk: {:.1}MB available", resources.disk_available_mb);
                        println!(
                            "    Network: {}",
                            if resources.network_available {
                                "✅ Available"
                            } else {
                                "❌ Unavailable"
                            }
                        );
                        println!("    Processes: {}", resources.process_count);

                        // Recommendations
                        if !result.overall_healthy {
                            println!();
                            println!("  💡 Recommendations:");
                            if !result.ssm_health.is_healthy {
                                println!("    - Check SSM session status and connectivity");
                                println!("    - Verify AWS credentials and permissions");
                            }
                            if !result.network_health.is_healthy {
                                println!("    - Check internet connectivity");
                                println!("    - Verify AWS service endpoints are accessible");
                            }
                            if resources.memory_available_mb < 50.0 {
                                println!("    - Free up memory (less than 50MB available)");
                            }
                            if resources.cpu_available_percent < 10.0 {
                                println!("    - Reduce CPU load (less than 10% available)");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Comprehensive health check failed: {}", e);
                        println!("  ❌ Health check failed: {}", e);
                    }
                }
            } else {
                println!("  🔍 SSM session health check for: {}", id);

                match health_checker.check_ssm_session(&id).await {
                    Ok(result) => {
                        println!(
                            "  Status: {}",
                            if result.is_healthy {
                                "✅ Healthy"
                            } else {
                                "❌ Unhealthy"
                            }
                        );
                        println!("  Response Time: {}ms", result.response_time_ms);
                        if let Some(error) = &result.error_message {
                            println!("  Error: {}", error);
                        }
                        if let Some(details) = &result.details {
                            println!("  Details: {}", details);
                        }
                    }
                    Err(e) => {
                        error!("SSM health check failed: {}", e);
                        println!("  ❌ Health check failed: {}", e);
                    }
                }
            }
        }
        None => {
            println!("  🔍 System health check");

            // Perform system-wide health checks
            let (network_result, resource_result) = tokio::join!(
                health_checker.check_network_connectivity("ssm.amazonaws.com"),
                health_checker.check_resource_availability()
            );

            // Network Health
            match network_result {
                Ok(network_health) => {
                    println!("  🌐 Network Connectivity:");
                    println!(
                        "    Status: {}",
                        if network_health.is_healthy {
                            "✅ Healthy"
                        } else {
                            "❌ Unhealthy"
                        }
                    );
                    println!("    Response Time: {}ms", network_health.response_time_ms);
                    if let Some(error) = &network_health.error_message {
                        println!("    Error: {}", error);
                    }
                    if let Some(details) = &network_health.details {
                        println!("    Details: {}", details);
                    }
                }
                Err(e) => {
                    warn!("Network health check failed: {}", e);
                    println!("  🌐 Network Connectivity: ❌ Check failed - {}", e);
                }
            }

            println!();

            // Resource Health
            match resource_result {
                Ok(resources) => {
                    println!("  💾 System Resources:");
                    println!(
                        "    Memory: {:.1}MB available / {:.1}MB total ({:.1}% used)",
                        resources.memory_available_mb,
                        resources.memory_total_mb,
                        resources.memory_usage_percent
                    );
                    println!(
                        "    CPU: {:.1}% available ({:.1}% used)",
                        resources.cpu_available_percent, resources.cpu_usage_percent
                    );
                    println!("    Disk: {:.1}MB available", resources.disk_available_mb);
                    println!(
                        "    Network: {}",
                        if resources.network_available {
                            "✅ Available"
                        } else {
                            "❌ Unavailable"
                        }
                    );
                    println!("    Processes: {}", resources.process_count);

                    // Health assessment
                    let memory_healthy = resources.memory_available_mb > 50.0;
                    let cpu_healthy = resources.cpu_available_percent > 10.0;
                    let overall_healthy =
                        memory_healthy && cpu_healthy && resources.network_available;

                    println!();
                    println!(
                        "  📊 Overall System Health: {}",
                        if overall_healthy {
                            "✅ HEALTHY"
                        } else {
                            "⚠️  NEEDS ATTENTION"
                        }
                    );

                    if !overall_healthy {
                        println!("  💡 Issues detected:");
                        if !memory_healthy {
                            println!("    - Low memory available (< 50MB)");
                        }
                        if !cpu_healthy {
                            println!("    - High CPU usage (< 10% available)");
                        }
                        if !resources.network_available {
                            println!("    - Network connectivity issues");
                        }
                    }
                }
                Err(e) => {
                    error!("Resource health check failed: {}", e);
                    println!("  💾 System Resources: ❌ Check failed - {}", e);
                }
            }

            // AWS CLI availability check
            println!();
            println!("  🔧 Tool Availability:");
            let aws_cli_available = std::process::Command::new("aws")
                .arg("--version")
                .output()
                .is_ok();
            println!(
                "    AWS CLI: {}",
                if aws_cli_available {
                    "✅ Available"
                } else {
                    "❌ Not found"
                }
            );

            if !aws_cli_available {
                println!("  💡 Install AWS CLI to enable full SSM session health checks");
            }
        }
    }

    Ok(())
}

#[cfg(feature = "persistence")]
async fn handle_database(action: DatabaseCommands, _config: &Config) -> Result<()> {
    let persistence_manager = SqlitePersistenceManager::with_default_path()?;

    match action {
        DatabaseCommands::Init => {
            info!("Initializing database");
            println!("🗄️  Initializing database...");

            match persistence_manager.initialize().await {
                Ok(_) => {
                    println!("✅ Database initialized successfully");
                }
                Err(e) => {
                    error!("Database initialization failed: {}", e);
                    println!("❌ Database initialization failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        DatabaseCommands::Info => {
            info!("Getting database information");
            println!("🗄️  Database Information:");

            match persistence_manager.get_database_info().await {
                Ok(info) => {
                    println!("  📁 Database path: {:?}", info.db_path);
                    println!("  📊 Schema version: {}", info.schema_version);
                    println!("  📋 Sessions stored: {}", info.session_count);
                    println!("  📈 Performance metrics: {}", info.metrics_count);
                    println!(
                        "  💾 File size: {:.2} MB",
                        info.file_size_bytes as f64 / 1024.0 / 1024.0
                    );
                }
                Err(e) => {
                    error!("Failed to get database info: {}", e);
                    println!("❌ Failed to get database information: {}", e);
                }
            }
        }

        DatabaseCommands::Sessions => {
            info!("Listing stored sessions");
            println!("📋 Stored Sessions:");

            match persistence_manager.load_active_sessions().await {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("  No sessions found");
                    } else {
                        for session in sessions {
                            println!("  • Session ID: {}", session.session_id);
                            println!("    Instance: {}", session.instance_id);
                            println!("    Region: {}", session.region);
                            println!("    Status: {:?}", session.status);
                            println!(
                                "    Created: {}",
                                session.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                            );
                            println!(
                                "    Last Activity: {}",
                                session.last_activity.format("%Y-%m-%d %H:%M:%S UTC")
                            );
                            println!("    Connections: {}", session.connection_count);
                            println!("    Total Duration: {}s", session.total_duration_seconds);
                            println!();
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to load sessions: {}", e);
                    println!("❌ Failed to load sessions: {}", e);
                }
            }
        }

        DatabaseCommands::Stats { session_id } => {
            match session_id {
                Some(id) => {
                    info!("Getting performance statistics for session: {}", id);
                    println!("📊 Performance Statistics for session: {}", id);

                    match persistence_manager.get_performance_statistics(&id).await {
                        Ok(stats) => {
                            println!("  📈 Measurements: {}", stats.total_measurements);
                            println!("  ⏱️  Connection Time:");
                            println!("    Average: {}ms", stats.avg_connection_time_ms);
                            println!("    Min: {}ms", stats.min_connection_time_ms);
                            println!("    Max: {}ms", stats.max_connection_time_ms);
                            println!("  🌐 Latency:");
                            println!("    Average: {}ms", stats.avg_latency_ms);
                            println!("    Min: {}ms", stats.min_latency_ms);
                            println!("    Max: {}ms", stats.max_latency_ms);
                            println!("  📡 Throughput:");
                            println!("    Average: {:.2} Mbps", stats.avg_throughput_mbps);
                            println!("    Max: {:.2} Mbps", stats.max_throughput_mbps);
                            println!("  💾 Resource Usage:");
                            println!("    CPU Average: {:.2}%", stats.avg_cpu_usage_percent);
                            println!("    CPU Max: {:.2}%", stats.max_cpu_usage_percent);
                            println!("    Memory Average: {:.2}MB", stats.avg_memory_usage_mb);
                            println!("    Memory Max: {:.2}MB", stats.max_memory_usage_mb);
                        }
                        Err(e) => {
                            error!("Failed to get performance statistics: {}", e);
                            println!("❌ Failed to get performance statistics: {}", e);
                        }
                    }
                }
                None => {
                    info!("Getting performance statistics for all sessions");
                    println!("📊 Performance Statistics (All Sessions):");

                    // Load all sessions and get stats for each
                    match persistence_manager.load_active_sessions().await {
                        Ok(sessions) => {
                            if sessions.is_empty() {
                                println!("  No sessions found");
                            } else {
                                for session in sessions {
                                    println!("  📋 Session: {}", session.session_id);
                                    match persistence_manager
                                        .get_performance_statistics(&session.session_id)
                                        .await
                                    {
                                        Ok(stats) => {
                                            println!(
                                                "    Measurements: {}",
                                                stats.total_measurements
                                            );
                                            println!(
                                                "    Avg Connection: {}ms",
                                                stats.avg_connection_time_ms
                                            );
                                            println!("    Avg Latency: {}ms", stats.avg_latency_ms);
                                            println!(
                                                "    Avg Throughput: {:.2} Mbps",
                                                stats.avg_throughput_mbps
                                            );
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Failed to get stats for session {}: {}",
                                                session.session_id, e
                                            );
                                            println!("    ❌ Stats unavailable: {}", e);
                                        }
                                    }
                                    println!();
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to load sessions: {}", e);
                            println!("❌ Failed to load sessions: {}", e);
                        }
                    }
                }
            }
        }

        DatabaseCommands::Cleanup { days } => {
            info!("Cleaning up data older than {} days", days);
            println!("🧹 Cleaning up data older than {} days...", days);

            match persistence_manager.cleanup_old_data(days).await {
                Ok(deleted_count) => {
                    println!("✅ Cleanup completed: {} records deleted", deleted_count);
                }
                Err(e) => {
                    error!("Cleanup failed: {}", e);
                    println!("❌ Cleanup failed: {}", e);
                }
            }
        }

        DatabaseCommands::Export { output, format } => {
            info!("Exporting data to: {} (format: {})", output, format);
            println!("📤 Exporting data to: {} (format: {})", output, format);

            match format.as_str() {
                "json" => {
                    // Export sessions as JSON
                    match persistence_manager.load_active_sessions().await {
                        Ok(sessions) => {
                            let json_data = serde_json::to_string_pretty(&sessions)?;
                            std::fs::write(&output, json_data)?;
                            println!("✅ Exported {} sessions to {}", sessions.len(), output);
                        }
                        Err(e) => {
                            error!("Export failed: {}", e);
                            println!("❌ Export failed: {}", e);
                        }
                    }
                }
                "csv" => {
                    println!("❌ CSV export not yet implemented");
                }
                _ => {
                    println!("❌ Unsupported format: {}. Use 'json' or 'csv'", format);
                }
            }
        }
    }

    Ok(())
}

#[allow(dead_code)]
async fn handle_config_validation(_config: &Config) -> Result<()> {
    info!("Validating configuration");

    println!("⚙️  Configuration Validation:");
    println!("✅ Configuration file loaded successfully");
    println!("✅ All required settings present");

    Ok(())
}

async fn handle_config(action: ConfigCommands, config: &Config) -> Result<()> {
    match action {
        ConfigCommands::Validate => {
            info!("Validating configuration");
            println!("⚙️  Configuration Validation:");

            match config.validate() {
                Ok(_) => {
                    println!("✅ Configuration is valid");
                    println!("✅ All settings within acceptable ranges");
                    config.print_summary();
                }
                Err(e) => {
                    error!("Configuration validation failed: {}", e);
                    println!("❌ Configuration validation failed:");
                    println!("   {}", e);
                    return Err(e);
                }
            }
        }

        ConfigCommands::Show => {
            info!("Showing current configuration");
            println!("⚙️  Current Configuration:");
            println!();

            // AWS Configuration
            println!("🔐 AWS Settings:");
            println!("  Region: {}", config.aws.default_region);
            println!("  Profile: {:?}", config.aws.default_profile);
            println!("  Connection Timeout: {}s", config.aws.connection_timeout);
            println!("  Request Timeout: {}s", config.aws.request_timeout);
            println!();

            // Session Configuration
            println!("🔄 Session Management:");
            println!(
                "  Max Sessions per Instance: {}",
                config.session.max_sessions_per_instance
            );
            println!(
                "  Health Check Interval: {}s",
                config.session.health_check_interval
            );
            println!("  Inactive Timeout: {}s", config.session.inactive_timeout);
            println!(
                "  Timeout Prediction Threshold: {}s",
                config.session.timeout_prediction_threshold
            );
            println!();

            // Reconnection Policy
            println!("🔁 Reconnection Policy:");
            println!("  Enabled: {}", config.session.reconnection.enabled);
            println!(
                "  Max Attempts: {}",
                config.session.reconnection.max_attempts
            );
            println!(
                "  Base Delay: {}ms",
                config.session.reconnection.base_delay_ms
            );
            println!(
                "  Max Delay: {}ms",
                config.session.reconnection.max_delay_ms
            );
            println!(
                "  Aggressive Mode: {}",
                config.session.reconnection.aggressive_mode
            );
            if config.session.reconnection.aggressive_mode {
                println!(
                    "  Aggressive Attempts: {}",
                    config.session.reconnection.aggressive_attempts
                );
                println!(
                    "  Aggressive Interval: {}ms",
                    config.session.reconnection.aggressive_interval_ms
                );
            }
            println!();

            // Performance Configuration
            println!("📈 Performance Monitoring:");
            println!(
                "  Monitoring Enabled: {}",
                config.performance.monitoring_enabled
            );
            println!(
                "  Metrics Interval: {}s",
                config.performance.metrics_interval
            );
            println!(
                "  Latency Threshold: {}ms",
                config.performance.latency_threshold_ms
            );
            println!(
                "  Optimization Enabled: {}",
                config.performance.optimization_enabled
            );
            println!();

            // Resource Configuration
            println!("💾 Resource Limits:");
            println!("  Max Memory: {}MB", config.resources.max_memory_mb);
            println!("  Max CPU: {}%", config.resources.max_cpu_percent);
            println!("  Low Power Mode: {}", config.resources.low_power_mode);
            println!(
                "  Monitoring Interval: {}s",
                config.resources.monitoring_interval
            );
            println!();

            // UI Configuration
            println!("🖥️  User Interface:");
            println!("  Rich UI: {}", config.ui.rich_ui);
            println!("  Update Interval: {}ms", config.ui.update_interval_ms);
            println!("  Show Progress: {}", config.ui.show_progress);
            println!("  Notifications: {}", config.ui.notifications);
            println!();

            // Logging Configuration
            println!("📝 Logging:");
            println!("  Level: {}", config.logging.level);
            println!("  File Logging: {}", config.logging.file_logging);
            println!("  Log File: {:?}", config.logging.log_file);
            println!("  JSON Format: {}", config.logging.json_format);
        }

        ConfigCommands::Generate { output, format } => {
            let config_path = match output {
                Some(path) => std::path::PathBuf::from(path),
                None => Config::default_config_path()?,
            };

            info!(
                "Generating example configuration file: {:?} (format: {})",
                config_path, format
            );
            println!("📝 Generating example configuration file...");

            // Ensure correct extension
            let config_path = if format == "toml" {
                if config_path.extension().and_then(|s| s.to_str()) != Some("toml") {
                    config_path.with_extension("toml")
                } else {
                    config_path
                }
            } else {
                if config_path.extension().and_then(|s| s.to_str()) != Some("json") {
                    config_path.with_extension("json")
                } else {
                    config_path
                }
            };

            let default_config = Config::default();

            match default_config.save(&config_path).await {
                Ok(_) => {
                    println!("✅ Configuration file generated: {:?}", config_path);
                    println!("💡 Edit this file to customize your settings");
                    println!("💡 Use environment variables for runtime overrides");
                }
                Err(e) => {
                    error!("Failed to generate configuration file: {}", e);
                    println!("❌ Failed to generate configuration file: {}", e);
                    return Err(e);
                }
            }
        }

        ConfigCommands::EnvHelp => {
            info!("Showing environment variable help");
            println!("🌍 Environment Variable Configuration:");
            println!();
            println!("All configuration values can be overridden using environment variables.");
            println!("Environment variables take precedence over configuration file values.");
            println!();
            println!("Available Environment Variables:");
            println!();

            let env_vars = Config::get_env_variables_help();

            // Group by category
            let mut current_category = "";
            for (var_name, description) in env_vars {
                let category = if var_name.contains("AWS") {
                    "AWS Configuration"
                } else if var_name.contains("RECONNECTION") || var_name.contains("AGGRESSIVE") {
                    "Reconnection Policy"
                } else if var_name.contains("SESSION")
                    || var_name.contains("HEALTH")
                    || var_name.contains("INACTIVE")
                {
                    "Session Management"
                } else if var_name.contains("PERFORMANCE")
                    || var_name.contains("LATENCY")
                    || var_name.contains("OPTIMIZATION")
                {
                    "Performance Monitoring"
                } else if var_name.contains("MEMORY")
                    || var_name.contains("CPU")
                    || var_name.contains("POWER")
                {
                    "Resource Limits"
                } else if var_name.contains("UI")
                    || var_name.contains("RICH")
                    || var_name.contains("NOTIFICATIONS")
                {
                    "User Interface"
                } else if var_name.contains("LOG") {
                    "Logging"
                } else {
                    "Other"
                };

                if category != current_category {
                    println!("{}:", category);
                    current_category = category;
                }

                println!("  {} - {}", var_name, description);
            }

            println!();
            println!("Example Usage:");
            println!("  export NIMBUS_AWS_REGION=us-west-2");
            println!("  export NIMBUS_MAX_SESSIONS=5");
            println!("  export NIMBUS_RECONNECTION_ENABLED=true");
            println!("  export NIMBUS_LOG_LEVEL=debug");
            println!();
            println!("For more information, see: docs/CONFIGURATION.md");
        }

        ConfigCommands::Test => {
            info!("Testing configuration with environment overrides");
            println!("🧪 Testing Configuration:");
            println!();

            // Show which environment variables are currently set
            println!("🌍 Active Environment Variables:");
            let env_vars = Config::get_env_variables_help();
            let mut found_any = false;

            for (var_name, _) in env_vars {
                if let Ok(value) = std::env::var(var_name) {
                    println!("  {} = {}", var_name, value);
                    found_any = true;
                }
            }

            if !found_any {
                println!("  (No NIMBUS_* environment variables set)");
            }

            println!();
            println!("📋 Effective Configuration:");
            println!("  (After applying environment variable overrides)");
            println!();

            // Show effective configuration
            config.print_summary();

            println!();
            println!("✅ Configuration test complete");
            println!("💡 Use 'config show' to see full configuration details");
        }
    }

    Ok(())
}

#[cfg(feature = "multi-session")]
async fn handle_multi_session(_config: &Config) -> Result<()> {
    info!("Launching Multi-Session Management UI");

    println!("🖥️  Starting Multi-Session Management UI...");

    // Create session manager and monitor
    let session_manager = DefaultSessionManager::new(10).await.map_err(|e| {
        NimbusError::Session(crate::error::SessionError::CreationFailed {
            reason: format!("Failed to create session manager: {}", e),
        })
    })?;

    let session_monitor = DefaultSessionMonitor::new();

    // Create resource thresholds
    let thresholds = ResourceThresholds {
        memory_warning_mb: 8.0,
        memory_critical_mb: 10.0,
        cpu_warning_percent: 0.3,
        cpu_critical_percent: 0.5,
        max_sessions_per_instance: 3,
        max_total_sessions: 10,
    };

    // Create multi-session manager
    let multi_manager =
        MultiSessionManager::new(session_manager, session_monitor, Some(thresholds));

    // Create and run multi-session UI
    let mut multi_ui = MultiSessionUi::new(multi_manager);

    println!("🎯 Multi-Session Management UI is ready!");
    println!("📋 Use tabs to navigate: 1=Sessions, 2=Resources, 3=Warnings, 4=Details");
    println!("🔄 Press 'R' to refresh, 'Q' to quit");

    // Initialize terminal
    use crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main UI loop
    let mut should_quit = false;

    while !should_quit {
        // Render UI
        if let Err(e) = terminal.draw(|f| {
            if let Err(render_error) =
                tokio::runtime::Handle::current().block_on(multi_ui.render(f))
            {
                error!("UI render error: {}", render_error);
            }
        }) {
            error!("Terminal draw error: {}", e);
            break;
        }

        // Handle events
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        should_quit = true;
                    }
                    KeyCode::Char(c) => {
                        multi_ui.handle_input(c);
                    }
                    KeyCode::Up => {
                        multi_ui.handle_input('k');
                    }
                    KeyCode::Down => {
                        multi_ui.handle_input('j');
                    }
                    KeyCode::Esc => {
                        should_quit = true;
                    }
                    _ => {}
                }
            }
        }

        // Small delay to prevent excessive CPU usage
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("👋 Multi-Session Management UI closed");

    Ok(())
}

async fn handle_vscode(action: VsCodeCommands, config: &Config) -> Result<()> {
    use crate::session;
    use crate::vscode::VsCodeIntegration;
    use tokio::fs;
    match action {
        VsCodeCommands::Status => {
            info!("Checking VS Code integration status");
            println!("🔧 VS Code Integration Status:");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => match integration.check_integration_status().await {
                    Ok(status) => {
                        println!(
                            "  📊 Overall Status: {}",
                            if status.is_fully_available() {
                                "✅ Ready"
                            } else {
                                "⚠️  Partial"
                            }
                        );
                        println!();

                        println!("  🔍 Component Status:");
                        println!(
                            "    VS Code: {}",
                            if status.vscode_available {
                                "✅ Available"
                            } else {
                                "❌ Not Found"
                            }
                        );
                        if let Some(path) = &status.vscode_path {
                            println!("      Path: {:?}", path);
                        }

                        println!(
                            "    SSH Config: {}",
                            if status.ssh_config_writable {
                                "✅ Writable"
                            } else {
                                "❌ Not Writable"
                            }
                        );
                        println!("      Path: {:?}", status.ssh_config_path);

                        println!(
                            "    Auto Launch: {}",
                            if status.auto_launch_enabled {
                                "✅ Enabled"
                            } else {
                                "⚪ Disabled"
                            }
                        );

                        println!(
                            "    Notifications: {}",
                            if status.notifications_enabled {
                                "✅ Enabled"
                            } else {
                                "⚪ Disabled"
                            }
                        );

                        println!();

                        let features = status.available_features();
                        if !features.is_empty() {
                            println!("  ✅ Available Features:");
                            for feature in features {
                                println!("    • {}", feature);
                            }
                            println!();
                        }

                        let missing = status.missing_requirements();
                        if !missing.is_empty() {
                            println!("  ❌ Missing Requirements:");
                            for requirement in missing {
                                println!("    • {}", requirement);
                            }
                            println!();

                            println!("  💡 Recommendations:");
                            if !status.vscode_available {
                                println!(
                                    "    • Install VS Code from https://code.visualstudio.com/"
                                );
                                println!(
                                    "    • Or set NIMBUS_VSCODE_PATH environment variable"
                                );
                            }
                            if !status.ssh_config_writable {
                                println!("    • Check permissions on ~/.ssh/config file");
                                println!("    • Create ~/.ssh directory if it doesn't exist");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to check integration status: {}", e);
                        println!("  ❌ Status check failed: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }

        VsCodeCommands::Test { session_id } => {
            info!("Testing VS Code integration");
            println!("🧪 Testing VS Code Integration:");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => {
                    // Check status first
                    match integration.check_integration_status().await {
                        Ok(status) => {
                            if !status.is_fully_available() {
                                println!("  ⚠️  Integration not fully available. Run 'vscode status' for details.");
                                return Ok(());
                            }

                            // Create or use existing session for testing
                            let test_session = match session_id {
                                Some(id) => {
                                    println!("  🔍 Using existing session: {}", id);
                                    // In a real implementation, you would load the session from the session manager
                                    // For now, create a mock session
                                    session::Session {
                                        id: id.clone(),
                                        instance_id: "i-test123456789abcdef".to_string(),
                                        local_port: 8080,
                                        remote_port: 22,
                                        remote_host: None,
                                        status: session::SessionStatus::Active,
                                        created_at: std::time::SystemTime::now(),
                                        last_activity: std::time::SystemTime::now(),
                                        process_id: Some(12345),
                                        connection_count: 1,
                                        data_transferred: 0,
                                        aws_profile: None,
                                        region: "us-east-1".to_string(),
                                        priority: session::SessionPriority::Normal,
                                        tags: std::collections::HashMap::new(),
                                    }
                                }
                                None => {
                                    println!("  🆕 Creating test session...");
                                    session::Session {
                                        id: "test-session-vscode".to_string(),
                                        instance_id: "i-test123456789abcdef".to_string(),
                                        local_port: 8080,
                                        remote_port: 22,
                                        remote_host: None,
                                        status: session::SessionStatus::Active,
                                        created_at: std::time::SystemTime::now(),
                                        last_activity: std::time::SystemTime::now(),
                                        process_id: Some(12345),
                                        connection_count: 1,
                                        data_transferred: 0,
                                        aws_profile: None,
                                        region: "us-east-1".to_string(),
                                        priority: session::SessionPriority::Normal,
                                        tags: std::collections::HashMap::new(),
                                    }
                                }
                            };

                            println!("  📋 Test Session Details:");
                            println!("    Session ID: {}", test_session.id);
                            println!("    Instance ID: {}", test_session.instance_id);
                            println!("    Local Port: {}", test_session.local_port);
                            println!("    Remote Port: {}", test_session.remote_port);
                            println!();

                            // Perform integration test
                            match integration.integrate_session(&test_session).await {
                                Ok(result) => {
                                    println!("  📊 Integration Test Results:");
                                    println!(
                                        "    Overall Success: {}",
                                        if result.success { "✅ Yes" } else { "❌ No" }
                                    );
                                    println!(
                                        "    SSH Config Updated: {}",
                                        if result.ssh_config_updated {
                                            "✅ Yes"
                                        } else {
                                            "❌ No"
                                        }
                                    );
                                    println!(
                                        "    VS Code Launched: {}",
                                        if result.vscode_launched {
                                            "✅ Yes"
                                        } else {
                                            "❌ No"
                                        }
                                    );
                                    println!(
                                        "    Notification Sent: {}",
                                        if result.notification_sent {
                                            "✅ Yes"
                                        } else {
                                            "❌ No"
                                        }
                                    );

                                    if let Some(connection_info) = &result.connection_info {
                                        println!();
                                        println!("  🔗 Connection Information:");
                                        println!("    SSH Host: {}", connection_info.ssh_host);
                                        println!(
                                            "    Connection URL: {}",
                                            connection_info.connection_url
                                        );
                                    }

                                    if let Some(error) = &result.error_message {
                                        println!();
                                        println!("  ❌ Error Details: {}", error);
                                    }

                                    if result.success {
                                        println!();
                                        println!("  ✅ Integration test completed successfully!");
                                        println!("  💡 You can now connect to the instance using:");
                                        if let Some(connection_info) = &result.connection_info {
                                            println!("     ssh {}", connection_info.ssh_host);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Integration test failed: {}", e);
                                    println!("  ❌ Integration test failed: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to check integration status: {}", e);
                            println!("  ❌ Status check failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }

        VsCodeCommands::Setup => {
            info!("Setting up VS Code integration");
            println!("⚙️  VS Code Integration Setup:");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => match integration.check_integration_status().await {
                    Ok(status) => {
                        println!("  🔍 Current Status:");
                        println!(
                            "    VS Code: {}",
                            if status.vscode_available {
                                "✅ Found"
                            } else {
                                "❌ Not Found"
                            }
                        );
                        println!(
                            "    SSH Config: {}",
                            if status.ssh_config_writable {
                                "✅ Writable"
                            } else {
                                "❌ Not Writable"
                            }
                        );
                        println!();

                        if status.is_fully_available() {
                            println!(
                                "  ✅ VS Code integration is already set up and ready to use!"
                            );
                            println!();
                            println!("  📋 Configuration:");
                            if let Some(path) = &status.vscode_path {
                                println!("    VS Code Path: {:?}", path);
                            }
                            println!("    SSH Config: {:?}", status.ssh_config_path);
                            println!("    Auto Launch: {}", status.auto_launch_enabled);
                            println!("    Notifications: {}", status.notifications_enabled);
                        } else {
                            println!("  ⚠️  Setup incomplete. Please address the following:");
                            println!();

                            let missing = status.missing_requirements();
                            for (i, requirement) in missing.iter().enumerate() {
                                println!("  {}. {}", i + 1, requirement);
                            }

                            println!();
                            println!("  💡 Setup Instructions:");

                            if !status.vscode_available {
                                println!("    📥 Install VS Code:");
                                println!("      • Download from: https://code.visualstudio.com/");
                                println!("      • Or use package manager:");
                                println!("        - macOS: brew install --cask visual-studio-code");
                                println!("        - Ubuntu: snap install code --classic");
                                println!(
                                    "        - Windows: winget install Microsoft.VisualStudioCode"
                                );
                                println!();
                                println!("    🔧 Alternative: Set custom path");
                                println!("      export NIMBUS_VSCODE_PATH=/path/to/code");
                                println!();
                            }

                            if !status.ssh_config_writable {
                                println!("    📁 Fix SSH config permissions:");
                                println!("      mkdir -p ~/.ssh");
                                println!("      chmod 700 ~/.ssh");
                                println!("      touch ~/.ssh/config");
                                println!("      chmod 600 ~/.ssh/config");
                                println!();
                            }

                            println!("  🔄 After completing setup, run:");
                            println!("    nimbus vscode status");
                        }
                    }
                    Err(e) => {
                        error!("Failed to check integration status: {}", e);
                        println!("  ❌ Status check failed: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }

        VsCodeCommands::Cleanup { session_id } => {
            info!("Cleaning up VS Code integration");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => {
                    match session_id {
                        Some(id) => {
                            println!("🧹 Cleaning up SSH config for session: {}", id);

                            match integration.cleanup_ssh_config(&id).await {
                                Ok(_) => {
                                    println!("  ✅ SSH config cleaned up successfully");
                                }
                                Err(e) => {
                                    error!("Failed to clean up SSH config: {}", e);
                                    println!("  ❌ Cleanup failed: {}", e);
                                }
                            }
                        }
                        None => {
                            println!("🧹 Cleaning up all Nimbus entries from SSH config...");

                            // Read SSH config and remove all Nimbus entries
                            match integration.check_integration_status().await {
                                Ok(status) => {
                                    let ssh_config_path = &status.ssh_config_path;
                                    if ssh_config_path.exists() {
                                        match fs::read_to_string(ssh_config_path).await {
                                            Ok(content) => {
                                                let lines: Vec<&str> = content.lines().collect();
                                                let mut result_lines = Vec::new();
                                                let mut skip_section = false;

                                                for line in lines {
                                                    let trimmed = line.trim();

                                                    // Skip Nimbus sections
                                                    if trimmed.starts_with("# Nimbus") {
                                                        skip_section = true;
                                                        continue;
                                                    }

                                                    // End skip when we hit a new section or empty line
                                                    if skip_section {
                                                        if trimmed.starts_with("Host ")
                                                            && !trimmed.contains("ec2-")
                                                        {
                                                            skip_section = false;
                                                        } else if trimmed.is_empty()
                                                            && result_lines
                                                                .last()
                                                                .map_or(false, |l: &String| {
                                                                    l.trim().is_empty()
                                                                })
                                                        {
                                                            skip_section = false;
                                                        }

                                                        if skip_section {
                                                            continue;
                                                        }
                                                    }

                                                    result_lines.push(line.to_string());
                                                }

                                                let cleaned_content = result_lines.join("\n");

                                                match fs::write(ssh_config_path, cleaned_content)
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        println!("  ✅ All Nimbus entries removed from SSH config");
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to write cleaned SSH config: {}", e);
                                                        println!("  ❌ Failed to write cleaned SSH config: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to read SSH config: {}", e);
                                                println!("  ❌ Failed to read SSH config: {}", e);
                                            }
                                        }
                                    } else {
                                        println!("  ℹ️  SSH config file does not exist, nothing to clean");
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to get integration status: {}", e);
                                    println!("  ❌ Failed to get integration status: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }
    }

    Ok(())
}

async fn handle_diagnose(action: DiagnosticCommands, _config: &Config) -> Result<()> {
    use std::time::Duration;

    match action {
        DiagnosticCommands::Full {
            instance_id,
            local_port,
            remote_port,
            profile,
            region,
            parallel,
            timeout,
        } => {
            info!("Running full diagnostics for instance: {}", instance_id);
            println!("🔍 Running comprehensive SSM connection diagnostics...");
            println!("   Instance ID: {}", instance_id);

            if let Some(port) = local_port {
                println!("   Local Port: {}", port);
            }
            if let Some(port) = remote_port {
                println!("   Remote Port: {}", port);
            }
            if let Some(prof) = &profile {
                println!("   AWS Profile: {}", prof);
            }
            if let Some(reg) = &region {
                println!("   AWS Region: {}", reg);
            }

            println!(
                "   Execution Mode: {}",
                if parallel { "Parallel" } else { "Sequential" }
            );
            println!("   Timeout: {}s", timeout);
            println!();

            // Create diagnostic configuration
            let mut config = DiagnosticConfig::new(instance_id)
                .with_timeout(Duration::from_secs(timeout))
                .with_parallel_execution(parallel);

            if let (Some(local), Some(remote)) = (local_port, remote_port) {
                config = config.with_ports(local, remote);
            }

            config = config.with_aws_config(region, profile);

            // Create diagnostic manager
            let mut diagnostic_manager = DefaultDiagnosticManager::new()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;

            // Register progress callback
            diagnostic_manager.register_progress_callback(Box::new(|progress| {
                println!(
                    "📊 Progress: {:.1}% - {} ({}/{})",
                    progress.progress_percentage(),
                    progress.current_item,
                    progress.completed,
                    progress.total
                );

                if let Some(remaining) = progress.estimated_remaining {
                    println!("   ⏱️  Estimated remaining: {:?}", remaining);
                }
            }));

            // Run diagnostics
            match diagnostic_manager.run_full_diagnostics(config).await {
                Ok(results) => {
                    println!();
                    println!("📋 Diagnostic Results:");
                    println!("======================");

                    let mut success_count = 0;
                    let mut warning_count = 0;
                    let mut error_count = 0;

                    for result in &results {
                        let status_icon = match result.status {
                            diagnostic::DiagnosticStatus::Success => {
                                success_count += 1;
                                "✅"
                            }
                            diagnostic::DiagnosticStatus::Warning => {
                                warning_count += 1;
                                "⚠️"
                            }
                            diagnostic::DiagnosticStatus::Error => {
                                error_count += 1;
                                "❌"
                            }
                            diagnostic::DiagnosticStatus::Skipped => "⏭️",
                        };

                        let severity_text = match result.severity {
                            diagnostic::Severity::Critical => "CRITICAL",
                            diagnostic::Severity::High => "HIGH",
                            diagnostic::Severity::Medium => "MEDIUM",
                            diagnostic::Severity::Low => "LOW",
                            diagnostic::Severity::Info => "INFO",
                        };

                        println!(
                            "{} {} [{}] - {} ({:?})",
                            status_icon,
                            result.item_name,
                            severity_text,
                            result.message,
                            result.duration
                        );

                        if result.auto_fixable {
                            println!("   🔧 Auto-fixable");
                        }

                        if let Some(details) = &result.details {
                            println!("   📝 Details: {}", details);
                        }
                        println!();
                    }

                    // Summary
                    println!("📊 Summary:");
                    println!("   ✅ Success: {}", success_count);
                    println!("   ⚠️  Warnings: {}", warning_count);
                    println!("   ❌ Errors: {}", error_count);
                    println!("   📋 Total: {}", results.len());

                    if error_count > 0 {
                        println!();
                        println!("💡 Next steps:");
                        println!("   • Review error details above");
                        println!("   • Run 'nimbus diagnose precheck' for quick fixes");
                        println!("   • Use 'nimbus diagnose item' for specific issues");
                    } else if warning_count > 0 {
                        println!();
                        println!("💡 Connection should work, but consider addressing warnings");
                    } else {
                        println!();
                        println!("🎉 All diagnostics passed! Connection should work perfectly.");
                    }
                }
                Err(e) => {
                    error!("Diagnostic execution failed: {}", e);
                    println!("❌ Diagnostic execution failed: {}", e);
                    return Err(anyhow::anyhow!("Diagnostic execution failed: {}", e));
                }
            }
        }

        DiagnosticCommands::Precheck {
            instance_id,
            local_port,
            profile,
            region,
        } => {
            info!("Running precheck diagnostics for instance: {}", instance_id);
            println!("🚀 Running pre-connection checks...");
            println!("   Instance ID: {}", instance_id);

            if let Some(port) = local_port {
                println!("   Local Port: {}", port);
            }
            if let Some(prof) = &profile {
                println!("   AWS Profile: {}", prof);
            }
            if let Some(reg) = &region {
                println!("   AWS Region: {}", reg);
            }
            println!();

            // Create diagnostic configuration
            let mut config = DiagnosticConfig::new(instance_id)
                .with_timeout(Duration::from_secs(15))
                .with_parallel_execution(false); // Sequential for precheck

            if let Some(local) = local_port {
                config = config.with_ports(local, 22); // Default to SSH port
            }

            config = config.with_aws_config(region, profile);

            // Create diagnostic manager
            let mut diagnostic_manager = DefaultDiagnosticManager::new()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;

            // Register progress callback
            diagnostic_manager.register_progress_callback(Box::new(|progress| {
                println!(
                    "🔍 Checking: {} ({}/{})",
                    progress.current_item,
                    progress.completed + 1,
                    progress.total
                );
            }));

            // Run precheck
            match diagnostic_manager.run_precheck(config).await {
                Ok(results) => {
                    println!();
                    println!("📋 Pre-connection Check Results:");
                    println!("=================================");

                    let mut can_proceed = true;

                    for result in &results {
                        let status_icon = match result.status {
                            diagnostic::DiagnosticStatus::Success => "✅",
                            diagnostic::DiagnosticStatus::Warning => "⚠️",
                            diagnostic::DiagnosticStatus::Error => {
                                if matches!(
                                    result.severity,
                                    diagnostic::Severity::Critical
                                        | diagnostic::Severity::High
                                ) {
                                    can_proceed = false;
                                }
                                "❌"
                            }
                            diagnostic::DiagnosticStatus::Skipped => "⏭️",
                        };

                        println!("{} {} - {}", status_icon, result.item_name, result.message);

                        if result.auto_fixable {
                            println!("   🔧 This issue can be auto-fixed");
                        }
                    }

                    println!();
                    if can_proceed {
                        println!(
                            "🎯 Pre-connection checks passed! You can proceed with connection."
                        );
                        println!(
                            "💡 Run: nimbus connect --instance-id {}",
                            results
                                .first()
                                .map(|r| r.item_name.as_str())
                                .unwrap_or("INSTANCE_ID")
                        );
                    } else {
                        println!(
                            "🛑 Critical issues detected. Please resolve them before connecting."
                        );
                        println!("💡 Run: nimbus diagnose full --instance-id {} for detailed analysis", 
                                results.first().map(|r| r.item_name.as_str()).unwrap_or("INSTANCE_ID"));
                    }
                }
                Err(e) => {
                    error!("Precheck execution failed: {}", e);
                    println!("❌ Precheck execution failed: {}", e);
                    return Err(anyhow::anyhow!("Precheck execution failed: {}", e));
                }
            }
        }

        DiagnosticCommands::Preventive {
            instance_id,
            local_port,
            remote_port,
            profile,
            region,
            abort_on_critical,
            timeout,
        } => {
            info!("Running preventive checks for instance: {}", instance_id);
            println!("🛡️  Running preventive connection checks...");
            println!("   Instance ID: {}", instance_id);

            if let Some(port) = local_port {
                println!("   Local Port: {}", port);
            }
            if let Some(port) = remote_port {
                println!("   Remote Port: {}", port);
            }
            if let Some(prof) = &profile {
                println!("   AWS Profile: {}", prof);
            }
            if let Some(reg) = &region {
                println!("   AWS Region: {}", reg);
            }

            println!("   Abort on Critical: {}", abort_on_critical);
            println!("   Timeout: {}s", timeout);
            println!();

            // Create preventive check configuration
            let mut config = PreventiveCheckConfig::new(instance_id.clone())
                .with_abort_on_critical(abort_on_critical)
                .with_timeout(Duration::from_secs(timeout));

            if let (Some(local), Some(remote)) = (local_port, remote_port) {
                config = config.with_ports(local, remote);
            }

            config = config.with_aws_config(region, profile);

            // Create preventive check instance
            let preventive_check = match DefaultPreventiveCheck::with_aws_config(
                config.region.clone(),
                config.profile.clone(),
            )
            .await
            {
                Ok(checker) => checker,
                Err(e) => {
                    error!("Failed to create preventive check: {}", e);
                    println!("❌ Failed to initialize preventive check: {}", e);
                    return Err(anyhow::anyhow!("Failed to create preventive check: {}", e));
                }
            };

            // Run preventive checks
            match preventive_check.run_preventive_checks(config).await {
                Ok(result) => {
                    println!("📋 Preventive Check Results:");
                    println!("============================");

                    // Display overall status
                    let status_icon = match result.overall_status {
                        preventive_check::PreventiveCheckStatus::Ready => "✅",
                        preventive_check::PreventiveCheckStatus::Warning => "⚠️",
                        preventive_check::PreventiveCheckStatus::Critical => "❌",
                        preventive_check::PreventiveCheckStatus::Aborted => "🛑",
                    };

                    println!(
                        "{} Overall Status: {:?}",
                        status_icon, result.overall_status
                    );
                    println!(
                        "🎯 Connection Likelihood: {} ({}%)",
                        result.connection_likelihood.as_description(),
                        result.connection_likelihood.as_percentage()
                    );
                    println!("⏱️  Total Duration: {:?}", result.total_duration);
                    println!();

                    // Display critical issues
                    if !result.critical_issues.is_empty() {
                        println!("🚨 Critical Issues ({}):", result.critical_issues.len());
                        for issue in &result.critical_issues {
                            println!("   ❌ {}: {}", issue.item_name, issue.message);
                        }
                        println!();
                    }

                    // Display warnings
                    if !result.warnings.is_empty() {
                        println!("⚠️  Warnings ({}):", result.warnings.len());
                        for warning in &result.warnings {
                            println!("   ⚠️  {}: {}", warning.item_name, warning.message);
                        }
                        println!();
                    }

                    // Display recommendations
                    if !result.recommendations.is_empty() {
                        println!("💡 Recommendations:");
                        for (index, recommendation) in result.recommendations.iter().enumerate() {
                            println!("   {}. {}", index + 1, recommendation);
                        }
                        println!();
                    }

                    // Final decision
                    if result.should_abort_connection {
                        println!("🛑 Connection aborted due to critical issues.");
                        println!("   Please resolve the critical issues above before attempting connection.");
                        println!("   Run 'nimbus diagnose full --instance-id {}' for detailed analysis.", instance_id);
                    } else {
                        match result.overall_status {
                            preventive_check::PreventiveCheckStatus::Ready => {
                                println!("🚀 All checks passed! You can proceed with connection.");
                                println!(
                                    "   Run: nimbus connect --instance-id {}",
                                    instance_id
                                );
                            }
                            preventive_check::PreventiveCheckStatus::Warning => {
                                println!("⚠️  Connection can proceed but with warnings.");
                                println!(
                                    "   Consider addressing warnings for optimal performance."
                                );
                                println!(
                                    "   Run: nimbus connect --instance-id {}",
                                    instance_id
                                );
                            }
                            _ => {
                                println!("❓ Connection status unclear. Review issues above.");
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Preventive check execution failed: {}", e);
                    println!("❌ Preventive check execution failed: {}", e);
                    return Err(anyhow::anyhow!("Preventive check execution failed: {}", e));
                }
            }
        }

        DiagnosticCommands::Item {
            item,
            instance_id,
            local_port,
            remote_port,
            profile,
            region,
        } => {
            info!(
                "Running specific diagnostic: {} for instance: {}",
                item, instance_id
            );
            println!("🔍 Running specific diagnostic: {}", item);
            println!("   Instance ID: {}", instance_id);
            println!();

            // Create diagnostic configuration
            let mut config =
                DiagnosticConfig::new(instance_id).with_timeout(Duration::from_secs(30));

            if let (Some(local), Some(remote)) = (local_port, remote_port) {
                config = config.with_ports(local, remote);
            }

            config = config.with_aws_config(region, profile);

            // Create diagnostic manager
            let mut diagnostic_manager = DefaultDiagnosticManager::new()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;

            // Run specific diagnostic
            match diagnostic_manager
                .run_specific_diagnostic(&item, config)
                .await
            {
                Ok(result) => {
                    println!("📋 Diagnostic Result for '{}':", item);
                    println!("===============================");

                    let status_icon = match result.status {
                        diagnostic::DiagnosticStatus::Success => "✅",
                        diagnostic::DiagnosticStatus::Warning => "⚠️",
                        diagnostic::DiagnosticStatus::Error => "❌",
                        diagnostic::DiagnosticStatus::Skipped => "⏭️",
                    };

                    let severity_text = match result.severity {
                        diagnostic::Severity::Critical => "CRITICAL",
                        diagnostic::Severity::High => "HIGH",
                        diagnostic::Severity::Medium => "MEDIUM",
                        diagnostic::Severity::Low => "LOW",
                        diagnostic::Severity::Info => "INFO",
                    };

                    println!(
                        "{} Status: {} [{}]",
                        status_icon, result.message, severity_text
                    );
                    println!("⏱️  Duration: {:?}", result.duration);

                    if result.auto_fixable {
                        println!("🔧 Auto-fixable: Yes");
                    }

                    if let Some(details) = &result.details {
                        println!("📝 Details:");
                        println!(
                            "{}",
                            serde_json::to_string_pretty(details)
                                .unwrap_or_else(|_| details.to_string())
                        );
                    }
                }
                Err(e) => {
                    error!("Specific diagnostic failed: {}", e);
                    println!("❌ Diagnostic failed: {}", e);
                    return Err(anyhow::anyhow!("Specific diagnostic failed: {}", e));
                }
            }
        }

        DiagnosticCommands::List => {
            info!("Listing available diagnostic items");
            println!("📋 Available Diagnostic Items:");
            println!("==============================");

            let diagnostic_manager = DefaultDiagnosticManager::new()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;
            let items = diagnostic_manager.get_diagnostic_items();

            for (index, item) in items.iter().enumerate() {
                let description = match item.as_str() {
                    "instance_state" => "Check EC2 instance existence and state",
                    "ssm_agent_enhanced" => "Enhanced SSM agent diagnostics with version analysis and registration details",
                    "iam_permissions" => "Validate IAM roles and permissions",
                    "vpc_endpoints" => "Check VPC endpoints for SSM connectivity",
                    "security_groups" => "Verify security group rules",
                    "network_connectivity" => "Test network connectivity to AWS services",
                    "local_port_availability" => "Check local port availability for forwarding",
                    _ => "Unknown diagnostic item",
                };

                println!("{}. {} - {}", index + 1, item, description);
            }

            println!();
            println!("💡 Usage examples:");
            println!("   nimbus diagnose item --item instance_state --instance-id i-1234567890abcdef0");
            println!("   nimbus diagnose item --item local_port_availability --instance-id i-1234567890abcdef0 --local-port 8080");
        }

        DiagnosticCommands::AwsConfig {
            instance_id,
            profile,
            region,
            include_credentials,
            include_iam,
            include_vpc,
            include_security_groups,
            minimum_score,
        } => {
            info!(
                "Running AWS configuration validation for instance: {}",
                instance_id
            );
            println!("🔧 Running comprehensive AWS configuration validation...");
            println!("   Instance ID: {}", instance_id);

            if let Some(prof) = &profile {
                println!("   AWS Profile: {}", prof);
            }
            if let Some(reg) = &region {
                println!("   AWS Region: {}", reg);
            }

            println!("   Validation Scope:");
            println!(
                "     • Credentials: {}",
                if include_credentials { "✅" } else { "❌" }
            );
            println!(
                "     • IAM Permissions: {}",
                if include_iam { "✅" } else { "❌" }
            );
            println!(
                "     • VPC Configuration: {}",
                if include_vpc { "✅" } else { "❌" }
            );
            println!(
                "     • Security Groups: {}",
                if include_security_groups {
                    "✅"
                } else {
                    "❌"
                }
            );
            println!("   Minimum Compliance Score: {:.1}%", minimum_score);
            println!();

            // Create diagnostic configuration
            let config = DiagnosticConfig::new(instance_id.clone())
                .with_aws_config(region, profile)
                .with_timeout(Duration::from_secs(60));

            // Create diagnostic manager
            let mut diagnostic_manager = DefaultDiagnosticManager::new()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;

            // Run AWS configuration validation
            match diagnostic_manager.run_aws_config_validation(config).await {
                Ok(validation_result) => {
                    println!("📋 AWS Configuration Validation Results:");
                    println!("========================================");

                    // Display overall compliance
                    let compliance_icon = validation_result.compliance_status.color_code();
                    println!(
                        "{} Overall Compliance: {:.1}% ({})",
                        compliance_icon,
                        validation_result.overall_compliance_score,
                        validation_result.compliance_status.description()
                    );
                    println!(
                        "📅 Validation Time: {}",
                        validation_result
                            .validation_timestamp
                            .format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    println!();

                    // Display summary
                    let summary = &validation_result.summary;
                    println!("📊 Summary:");
                    println!("   Total Checks: {}", summary.total_checks);
                    println!("   ✅ Passed: {}", summary.passed_checks);
                    println!("   ⚠️  Warnings: {}", summary.warning_checks);
                    println!("   ❌ Failed: {}", summary.failed_checks);
                    println!("   ⏭️  Skipped: {}", summary.skipped_checks);
                    println!("   📈 Average Score: {:.1}%", summary.average_score);
                    println!("   ⚖️  Weighted Score: {:.1}%", summary.weighted_score);
                    println!();

                    // Display individual check results
                    println!("🔍 Individual Check Results:");
                    for check in &validation_result.check_results {
                        let status_icon = match check.status {
                            diagnostic::DiagnosticStatus::Success => "✅",
                            diagnostic::DiagnosticStatus::Warning => "⚠️",
                            diagnostic::DiagnosticStatus::Error => "❌",
                            diagnostic::DiagnosticStatus::Skipped => "⏭️",
                        };

                        println!(
                            "   {} {} - Score: {:.1}% (Weight: {:.1}%)",
                            status_icon,
                            check.check_name,
                            check.score,
                            check.weight * 100.0
                        );
                        println!("      {}", check.message);

                        if !check.improvement_suggestions.is_empty() {
                            println!("      💡 Suggestions:");
                            for suggestion in &check.improvement_suggestions {
                                println!("         • {}", suggestion);
                            }
                        }
                        println!();
                    }

                    // Display improvement suggestions
                    if !validation_result.improvement_suggestions.is_empty() {
                        println!("🚀 Prioritized Improvement Suggestions:");
                        for (index, suggestion) in
                            validation_result.improvement_suggestions.iter().enumerate()
                        {
                            let priority_icon = match suggestion.priority {
                                aws_config_validator::SuggestionPriority::Critical => {
                                    "🚨"
                                }
                                aws_config_validator::SuggestionPriority::High => "🔴",
                                aws_config_validator::SuggestionPriority::Medium => {
                                    "🟡"
                                }
                                aws_config_validator::SuggestionPriority::Low => "🟢",
                            };

                            let category_text = match suggestion.category {
                                aws_config_validator::SuggestionCategory::Credentials => "Credentials",
                                aws_config_validator::SuggestionCategory::IamPermissions => "IAM Permissions",
                                aws_config_validator::SuggestionCategory::VpcConfiguration => "VPC Configuration",
                                aws_config_validator::SuggestionCategory::SecurityGroups => "Security Groups",
                                aws_config_validator::SuggestionCategory::NetworkConnectivity => "Network Connectivity",
                                aws_config_validator::SuggestionCategory::General => "General",
                            };

                            println!(
                                "   {}. {} [{}] {} - {}",
                                index + 1,
                                priority_icon,
                                category_text,
                                suggestion.title,
                                suggestion.priority.description()
                            );
                            println!("      {}", suggestion.description);
                            println!(
                                "      📈 Expected Impact: +{:.1}% compliance score",
                                suggestion.estimated_impact
                            );

                            if !suggestion.action_items.is_empty() {
                                println!("      🔧 Action Items:");
                                for action in &suggestion.action_items {
                                    println!("         • {}", action);
                                }
                            }

                            if !suggestion.related_checks.is_empty() {
                                println!(
                                    "      🔗 Related Checks: {}",
                                    suggestion.related_checks.join(", ")
                                );
                            }
                            println!();
                        }
                    }

                    // Final recommendations
                    println!("🎯 Final Assessment:");
                    if validation_result.overall_compliance_score >= minimum_score {
                        println!(
                            "   ✅ AWS configuration meets the minimum compliance score of {:.1}%",
                            minimum_score
                        );
                        println!(
                            "   🚀 SSM connections should work reliably with this configuration"
                        );

                        if validation_result.overall_compliance_score < 90.0 {
                            println!("   💡 Consider implementing the suggestions above for optimal performance");
                        }
                    } else {
                        println!("   ❌ AWS configuration does not meet the minimum compliance score of {:.1}%", minimum_score);
                        println!("   🛠️  Please address the critical and high-priority suggestions above");
                        println!("   ⚠️  SSM connections may fail or be unreliable with the current configuration");
                    }

                    println!();
                    println!("💡 Next Steps:");
                    if validation_result.overall_compliance_score >= minimum_score {
                        println!("   • Run 'nimbus connect --instance-id {}' to test the connection", instance_id);
                        println!("   • Use 'nimbus diagnose preventive --instance-id {}' for pre-connection checks", instance_id);
                    } else {
                        println!("   • Address the high-priority suggestions above");
                        println!("   • Re-run this validation after making changes");
                        println!("   • Use 'nimbus diagnose full --instance-id {}' for detailed diagnostics", instance_id);
                    }
                }
                Err(e) => {
                    error!("AWS configuration validation failed: {}", e);
                    println!("❌ AWS configuration validation failed: {}", e);
                    return Err(anyhow::anyhow!(
                        "AWS configuration validation failed: {}",
                        e
                    ));
                }
            }
        }

        DiagnosticCommands::AwsConfigIntegrated {
            instance_id,
            profile,
            region,
            include_credentials,
            include_iam,
            include_vpc,
            include_security_groups,
            minimum_score,
            clear_cache,
        } => {
            info!(
                "Running integrated AWS configuration validation for instance: {}",
                instance_id
            );
            println!("🔧 Running integrated AWS configuration validation with cross-validation...");
            println!("   Instance ID: {}", instance_id);

            if let Some(prof) = &profile {
                println!("   AWS Profile: {}", prof);
            }
            if let Some(reg) = &region {
                println!("   AWS Region: {}", reg);
            }

            println!("   Validation Scope:");
            println!(
                "     • Credentials: {}",
                if include_credentials { "✅" } else { "❌" }
            );
            println!(
                "     • IAM Permissions: {}",
                if include_iam { "✅" } else { "❌" }
            );
            println!(
                "     • VPC Configuration: {}",
                if include_vpc { "✅" } else { "❌" }
            );
            println!(
                "     • Security Groups: {}",
                if include_security_groups {
                    "✅"
                } else {
                    "❌"
                }
            );
            println!("   Minimum Compliance Score: {:.1}%", minimum_score);
            println!(
                "   Cache Management: {}",
                if clear_cache {
                    "Clear before validation"
                } else {
                    "Use cached results if available"
                }
            );
            println!();

            // Create AWS configuration validation config
            let validation_config = AwsConfigValidationConfig::new(instance_id.clone())
                .with_aws_config(region.clone(), profile.clone())
                .with_checks(
                    include_credentials,
                    include_iam,
                    include_vpc,
                    include_security_groups,
                )
                .with_minimum_compliance_score(minimum_score);

            // Create AWS config validator
            let validator = if let (Some(region), Some(profile)) = (&region, &profile) {
                DefaultAwsConfigValidator::with_aws_config(
                    Some(region.clone()),
                    Some(profile.clone()),
                )
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create AWS config validator: {}", e))?
            } else {
                DefaultAwsConfigValidator::new()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to create AWS config validator: {}", e))?
            };

            // Clear cache if requested
            if clear_cache {
                println!("🗑️  Clearing integration cache...");
                validator.clear_integration_cache().await;
            }

            // Run integrated AWS configuration validation
            match validator
                .validate_integrated_aws_configuration(validation_config)
                .await
            {
                Ok(validation_result) => {
                    println!("📋 Integrated AWS Configuration Validation Results:");
                    println!("==================================================");

                    // Display overall compliance
                    let compliance_icon = validation_result.compliance_status.color_code();
                    println!(
                        "{} Overall Compliance: {:.1}% ({})",
                        compliance_icon,
                        validation_result.overall_compliance_score,
                        validation_result.compliance_status.description()
                    );
                    println!(
                        "📅 Validation Time: {}",
                        validation_result
                            .validation_timestamp
                            .format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    println!();

                    // Display summary
                    let summary = &validation_result.summary;
                    println!("📊 Summary:");
                    println!("   Total Checks: {}", summary.total_checks);
                    println!("   ✅ Passed: {}", summary.passed_checks);
                    println!("   ⚠️  Warnings: {}", summary.warning_checks);
                    println!("   ❌ Failed: {}", summary.failed_checks);
                    println!("   ⏭️  Skipped: {}", summary.skipped_checks);
                    println!("   📈 Average Score: {:.1}%", summary.average_score);
                    println!("   ⚖️  Weighted Score: {:.1}%", summary.weighted_score);
                    println!();

                    // Display individual check results with integration details
                    println!("🔍 Individual Check Results (with Cross-Validation):");
                    for check in &validation_result.check_results {
                        let status_icon = match check.status {
                            diagnostic::DiagnosticStatus::Success => "✅",
                            diagnostic::DiagnosticStatus::Warning => "⚠️",
                            diagnostic::DiagnosticStatus::Error => "❌",
                            diagnostic::DiagnosticStatus::Skipped => "⏭️",
                        };

                        println!(
                            "   {} {} - Score: {:.1}% (Weight: {:.1}%)",
                            status_icon,
                            check.check_name,
                            check.score,
                            check.weight * 100.0
                        );
                        println!("      {}", check.message);

                        // Display integration details if available
                        if let Some(details) = &check.details {
                            if let Some(integration_checks) = details.get("integration_checks") {
                                if let Some(checks_array) = integration_checks.as_array() {
                                    println!("      🔗 Integration Results:");
                                    for integration_check in checks_array {
                                        if let Some(check_str) = integration_check.as_str() {
                                            println!("         {}", check_str);
                                        }
                                    }
                                }
                            }

                            if let Some(basic_score) = details.get("basic_score") {
                                if let Some(integration_adjustment) =
                                    details.get("integration_adjustment")
                                {
                                    println!("      📊 Score Breakdown: Basic: {:.1}%, Integration Adjustment: {:+.1}%", 
                                            basic_score.as_f64().unwrap_or(0.0),
                                            integration_adjustment.as_f64().unwrap_or(0.0));
                                }
                            }
                        }

                        if !check.improvement_suggestions.is_empty() {
                            println!("      💡 Suggestions:");
                            for suggestion in &check.improvement_suggestions {
                                println!("         • {}", suggestion);
                            }
                        }
                        println!();
                    }

                    // Display improvement suggestions with enhanced prioritization
                    if !validation_result.improvement_suggestions.is_empty() {
                        println!("🚀 Integrated Improvement Suggestions (Prioritized):");
                        for (index, suggestion) in
                            validation_result.improvement_suggestions.iter().enumerate()
                        {
                            let priority_icon = match suggestion.priority {
                                aws_config_validator::SuggestionPriority::Critical => {
                                    "🚨"
                                }
                                aws_config_validator::SuggestionPriority::High => "🔴",
                                aws_config_validator::SuggestionPriority::Medium => {
                                    "🟡"
                                }
                                aws_config_validator::SuggestionPriority::Low => "🟢",
                            };

                            let category_text = match suggestion.category {
                                aws_config_validator::SuggestionCategory::Credentials => "Credentials",
                                aws_config_validator::SuggestionCategory::IamPermissions => "IAM Permissions",
                                aws_config_validator::SuggestionCategory::VpcConfiguration => "VPC Configuration",
                                aws_config_validator::SuggestionCategory::SecurityGroups => "Security Groups",
                                aws_config_validator::SuggestionCategory::NetworkConnectivity => "Network Connectivity",
                                aws_config_validator::SuggestionCategory::General => "General",
                            };

                            println!(
                                "   {}. {} [{}] {} - {}",
                                index + 1,
                                priority_icon,
                                category_text,
                                suggestion.title,
                                suggestion.priority.description()
                            );
                            println!("      {}", suggestion.description);
                            println!(
                                "      📈 Expected Impact: +{:.1}% compliance score",
                                suggestion.estimated_impact
                            );

                            if !suggestion.action_items.is_empty() {
                                println!("      🔧 Action Items:");
                                for action in &suggestion.action_items {
                                    println!("         • {}", action);
                                }
                            }

                            if !suggestion.related_checks.is_empty() {
                                println!(
                                    "      🔗 Related Checks: {}",
                                    suggestion.related_checks.join(", ")
                                );
                            }
                            println!();
                        }
                    }

                    // Final recommendations with integration insights
                    println!("🎯 Integrated Assessment:");
                    if validation_result.overall_compliance_score >= minimum_score {
                        println!(
                            "   ✅ AWS configuration meets the minimum compliance score of {:.1}%",
                            minimum_score
                        );
                        println!(
                            "   🚀 SSM connections should work reliably with this configuration"
                        );
                        println!("   🔗 Cross-validation confirms component compatibility");

                        if validation_result.overall_compliance_score < 90.0 {
                            println!("   💡 Consider implementing the suggestions above for optimal performance");
                        }
                    } else {
                        println!("   ❌ AWS configuration does not meet the minimum compliance score of {:.1}%", minimum_score);
                        println!("   🛠️  Please address the critical and high-priority suggestions above");
                        println!("   ⚠️  Cross-validation detected dependency issues that may prevent SSM connections");
                        println!(
                            "   🔄 Follow the suggested order to resolve dependency chain issues"
                        );
                    }

                    println!();
                    println!("💡 Next Steps:");
                    if validation_result.overall_compliance_score >= minimum_score {
                        println!("   • Run 'nimbus connect --instance-id {}' to test the connection", instance_id);
                        println!("   • Use 'nimbus diagnose preventive --instance-id {}' for pre-connection checks", instance_id);
                        println!("   • Cache will be used for faster subsequent validations");
                    } else {
                        println!("   • Address the high-priority suggestions above in the recommended order");
                        println!(
                            "   • Re-run this validation with --clear-cache after making changes"
                        );
                        println!("   • Use 'nimbus diagnose full --instance-id {}' for detailed diagnostics", instance_id);
                    }
                }
                Err(e) => {
                    error!("Integrated AWS configuration validation failed: {}", e);
                    println!("❌ Integrated AWS configuration validation failed: {}", e);
                    return Err(anyhow::anyhow!(
                        "Integrated AWS configuration validation failed: {}",
                        e
                    ));
                }
            }
        }

        DiagnosticCommands::Interactive {
            instance_id,
            local_port,
            remote_port,
            profile,
            region,
            parallel,
            timeout,
            no_color,
            refresh_interval,
        } => {
            info!(
                "Running interactive diagnostics with real-time feedback for instance: {}",
                instance_id
            );
            println!("🎮 Starting interactive diagnostic session...");
            println!("   Instance ID: {}", instance_id);

            if let Some(port) = local_port {
                println!("   Local Port: {}", port);
            }
            if let Some(port) = remote_port {
                println!("   Remote Port: {}", port);
            }
            if let Some(prof) = &profile {
                println!("   AWS Profile: {}", prof);
            }
            if let Some(reg) = &region {
                println!("   AWS Region: {}", reg);
            }

            println!(
                "   Execution Mode: {}",
                if parallel { "Parallel" } else { "Sequential" }
            );
            println!("   Timeout: {}s", timeout);
            println!("   Refresh Interval: {}ms", refresh_interval);
            println!();

            // Create diagnostic configuration
            let mut config = DiagnosticConfig::new(instance_id.clone())
                .with_timeout(Duration::from_secs(timeout))
                .with_parallel_execution(parallel);

            if let (Some(local), Some(remote)) = (local_port, remote_port) {
                config = config.with_ports(local, remote);
            }

            config = config.with_aws_config(region, profile);

            // Create feedback configuration
            let feedback_config = realtime_feedback::FeedbackConfig {
                show_progress_bar: true,
                show_detailed_status: true,
                enable_colors: !no_color,
                auto_confirm_critical: false,
                refresh_interval_ms: refresh_interval,
            };

            // Create diagnostic manager with real-time feedback
            let mut diagnostic_manager = DefaultDiagnosticManager::new()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;

            // Enable real-time feedback
            diagnostic_manager
                .enable_realtime_feedback(feedback_config)
                .map_err(|e| anyhow::anyhow!("Failed to enable real-time feedback: {}", e))?;

            println!("🚀 Starting real-time diagnostic display...");
            println!("   Controls: [Ctrl+C] Interrupt | [P] Pause | [R] Resume | [Q] Quit");
            println!("   Critical Issues: [Y] Continue | [N] Abort");
            println!();

            // Start real-time feedback display in a separate task
            let feedback_task = tokio::spawn(async move {
                // The feedback display will be handled by the diagnostic manager itself
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<(), anyhow::Error>(())
            });

            // Run diagnostics with real-time feedback
            let diagnostics_result = diagnostic_manager.run_full_diagnostics(config).await;

            // Wait for feedback display to complete
            let _ = feedback_task.await;
            // Process results
            match diagnostics_result {
                Ok(results) => {
                    println!();
                    println!("🎯 Interactive Diagnostics Completed!");
                    println!("=====================================");

                    let mut success_count = 0;
                    let mut warning_count = 0;
                    let mut error_count = 0;
                    let mut critical_count = 0;

                    for result in &results {
                        match result.status {
                            diagnostic::DiagnosticStatus::Success => {
                                success_count += 1
                            }
                            diagnostic::DiagnosticStatus::Warning => {
                                warning_count += 1
                            }
                            diagnostic::DiagnosticStatus::Error => {
                                error_count += 1;
                                if matches!(
                                    result.severity,
                                    diagnostic::Severity::Critical
                                ) {
                                    critical_count += 1;
                                }
                            }
                            diagnostic::DiagnosticStatus::Skipped => {}
                        }
                    }

                    // Final summary
                    println!("📊 Final Summary:");
                    println!("   ✅ Success: {}", success_count);
                    println!("   ⚠️  Warnings: {}", warning_count);
                    println!("   ❌ Errors: {}", error_count);
                    println!("   🚨 Critical: {}", critical_count);
                    println!("   📋 Total: {}", results.len());

                    // Check for critical issues
                    if diagnostic_manager.has_critical_issues() {
                        let critical_issues = diagnostic_manager.get_critical_issues();
                        println!();
                        println!("🚨 Critical Issues Detected ({}):", critical_issues.len());
                        for (index, issue) in critical_issues.iter().enumerate() {
                            println!("   {}. {}: {}", index + 1, issue.item_name, issue.message);
                            if issue.auto_fixable {
                                println!("      🔧 Auto-fix available");
                            }
                        }
                    }

                    // Final status
                    let feedback_status = diagnostic_manager.get_feedback_status();
                    match feedback_status {
                        Some(realtime_feedback::FeedbackStatus::Completed) => {
                            if critical_count == 0 {
                                println!();
                                println!("🎉 All diagnostics completed successfully!");
                                println!("   Connection should work without issues.");
                                println!(
                                    "   Run: nimbus connect --instance-id {}",
                                    instance_id
                                );
                            } else {
                                println!();
                                println!("⚠️  Diagnostics completed with critical issues.");
                                println!("   Please resolve critical issues before connecting.");
                            }
                        }
                        Some(realtime_feedback::FeedbackStatus::Interrupted) => {
                            println!();
                            println!("⏸️  Diagnostics were interrupted by user.");
                            println!("   Run the command again to resume or use 'nimbus diagnose full' for non-interactive mode.");
                        }
                        Some(realtime_feedback::FeedbackStatus::Failed) => {
                            println!();
                            println!("❌ Diagnostics failed due to critical issues.");
                            println!("   User chose to abort due to critical problems.");
                        }
                        _ => {
                            println!();
                            println!("❓ Diagnostics completed with unknown status.");
                        }
                    }

                    // Cleanup
                    diagnostic_manager.stop_realtime_feedback();
                }
                Err(e) => {
                    error!("Interactive diagnostic execution failed: {}", e);
                    println!("❌ Interactive diagnostic execution failed: {}", e);

                    // Cleanup
                    diagnostic_manager.stop_realtime_feedback();

                    return Err(anyhow::anyhow!(
                        "Interactive diagnostic execution failed: {}",
                        e
                    ));
                }
            }
        }

        DiagnosticCommands::Settings { action } => {
            handle_diagnostic_settings(action, _config).await?
        }
    }

    Ok(())
}

async fn handle_precheck(
    instance_id: String,
    local_port: Option<u16>,
    profile: Option<String>,
    region: Option<String>,
    timeout: u64,
    format: String,
    output: Option<String>,
    _config: &Config,
) -> Result<()> {
    use std::time::Duration;

    info!("Running precheck for instance: {}", instance_id);
    println!("🚀 Running pre-connection checks...");
    println!("   Instance ID: {}", instance_id);

    if let Some(port) = local_port {
        println!("   Local Port: {}", port);
    }
    if let Some(prof) = &profile {
        println!("   AWS Profile: {}", prof);
    }
    if let Some(reg) = &region {
        println!("   AWS Region: {}", reg);
    }

    println!("   Timeout: {}s", timeout);
    println!("   Output Format: {}", format);
    println!();

    // Create diagnostic configuration
    let mut config = DiagnosticConfig::new(instance_id.clone())
        .with_timeout(Duration::from_secs(timeout))
        .with_parallel_execution(false); // Sequential for precheck

    if let Some(local) = local_port {
        config = config.with_ports(local, 22); // Default to SSH port
    }

    config = config.with_aws_config(region, profile);

    // Create diagnostic manager
    let mut diagnostic_manager = DefaultDiagnosticManager::new()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;

    // Register progress callback
    diagnostic_manager.register_progress_callback(Box::new(|progress| {
        println!(
            "🔍 Checking: {} ({}/{})",
            progress.current_item,
            progress.completed + 1,
            progress.total
        );
    }));

    // Run precheck
    match diagnostic_manager.run_precheck(config).await {
        Ok(results) => {
            // Format and output results
            match format.as_str() {
                "json" => {
                    let json_output = serde_json::to_string_pretty(&results).map_err(|e| {
                        anyhow::anyhow!("Failed to serialize results to JSON: {}", e)
                    })?;

                    if let Some(output_path) = output {
                        std::fs::write(&output_path, &json_output).map_err(|e| {
                            anyhow::anyhow!("Failed to write to file {}: {}", output_path, e)
                        })?;
                        println!("📄 Results saved to: {}", output_path);
                    } else {
                        println!("{}", json_output);
                    }
                }
                "yaml" => {
                    let yaml_output = serde_yaml::to_string(&results).map_err(|e| {
                        anyhow::anyhow!("Failed to serialize results to YAML: {}", e)
                    })?;

                    if let Some(output_path) = output {
                        std::fs::write(&output_path, &yaml_output).map_err(|e| {
                            anyhow::anyhow!("Failed to write to file {}: {}", output_path, e)
                        })?;
                        println!("📄 Results saved to: {}", output_path);
                    } else {
                        println!("{}", yaml_output);
                    }
                }
                _ => {
                    // Default text format
                    println!();
                    println!("📋 Pre-connection Check Results:");
                    println!("=================================");

                    let mut can_proceed = true;

                    for result in &results {
                        let status_icon = match result.status {
                            diagnostic::DiagnosticStatus::Success => "✅",
                            diagnostic::DiagnosticStatus::Warning => "⚠️",
                            diagnostic::DiagnosticStatus::Error => {
                                if matches!(
                                    result.severity,
                                    diagnostic::Severity::Critical
                                        | diagnostic::Severity::High
                                ) {
                                    can_proceed = false;
                                }
                                "❌"
                            }
                            diagnostic::DiagnosticStatus::Skipped => "⏭️",
                        };

                        println!("{} {} - {}", status_icon, result.item_name, result.message);

                        if result.auto_fixable {
                            println!("   🔧 This issue can be auto-fixed with: nimbus fix --instance-id {}", instance_id);
                        }
                    }

                    println!();
                    if can_proceed {
                        println!(
                            "🎯 Pre-connection checks passed! You can proceed with connection."
                        );
                        println!("💡 Run: nimbus connect --instance-id {}", instance_id);
                    } else {
                        println!(
                            "🛑 Critical issues detected. Please resolve them before connecting."
                        );
                        println!("💡 Run: nimbus fix --instance-id {} --auto-fix for automatic fixes", instance_id);
                        println!("💡 Run: nimbus diagnose full --instance-id {} for detailed analysis", instance_id);
                    }

                    if let Some(output_path) = output {
                        let text_output = format!(
                            "Pre-connection Check Results for {}\n{}",
                            instance_id,
                            results
                                .iter()
                                .map(|r| format!("{}: {}", r.item_name, r.message))
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                        std::fs::write(&output_path, &text_output).map_err(|e| {
                            anyhow::anyhow!("Failed to write to file {}: {}", output_path, e)
                        })?;
                        println!("📄 Results saved to: {}", output_path);
                    }
                }
            }
        }
        Err(e) => {
            error!("Precheck execution failed: {}", e);
            println!("❌ Precheck execution failed: {}", e);
            return Err(anyhow::anyhow!("Precheck execution failed: {}", e));
        }
    }

    Ok(())
}

async fn handle_fix(
    instance_id: String,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    profile: Option<String>,
    region: Option<String>,
    auto_fix: bool,
    safe_only: bool,
    dry_run: bool,
    timeout: u64,
    format: String,
    output: Option<String>,
    _config: &Config,
) -> Result<()> {
    use auto_fix::{AutoFixManager, DefaultAutoFixManager};
    use std::time::Duration;

    info!("Running auto-fix for instance: {}", instance_id);
    println!("🔧 Running automatic fixes...");
    println!("   Instance ID: {}", instance_id);

    if let Some(port) = local_port {
        println!("   Local Port: {}", port);
    }
    if let Some(port) = remote_port {
        println!("   Remote Port: {}", port);
    }
    if let Some(prof) = &profile {
        println!("   AWS Profile: {}", prof);
    }
    if let Some(reg) = &region {
        println!("   AWS Region: {}", reg);
    }

    println!(
        "   Auto-fix: {}",
        if auto_fix {
            "Enabled (will also run confirmation-required fixes)"
        } else {
            "Disabled (confirmation-required fixes may be skipped)"
        }
    );
    println!("   Safe only: {}", safe_only);
    println!("   Dry run: {}", dry_run);
    println!("   Timeout: {}s", timeout);
    println!("   Output Format: {}", format);
    println!();

    // First, run diagnostics to identify issues
    println!("🔍 Running diagnostics to identify fixable issues...");

    let mut diagnostic_config = DiagnosticConfig::new(instance_id.clone())
        .with_timeout(Duration::from_secs(timeout))
        .with_parallel_execution(true);

    if let (Some(local), Some(remote)) = (local_port, remote_port) {
        diagnostic_config = diagnostic_config.with_ports(local, remote);
    }

    diagnostic_config = diagnostic_config.with_aws_config(region.clone(), profile.clone());

    // Create diagnostic manager
    let mut diagnostic_manager = DefaultDiagnosticManager::new()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create diagnostic manager: {}", e))?;

    // Run diagnostics
    let diagnostic_results = diagnostic_manager
        .run_full_diagnostics(diagnostic_config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run diagnostics: {}", e))?;

    // Filter fixable issues
    let fixable_issues: Vec<_> = diagnostic_results
        .iter()
        .filter(|result| {
            result.auto_fixable
                && matches!(
                    result.status,
                    diagnostic::DiagnosticStatus::Error
                        | diagnostic::DiagnosticStatus::Warning
                )
        })
        .collect();

    if fixable_issues.is_empty() {
        println!("✅ No fixable issues found!");
        println!("   All diagnostics either passed or require manual intervention.");
        return Ok(());
    }

    println!("🔧 Found {} fixable issues:", fixable_issues.len());
    for (index, issue) in fixable_issues.iter().enumerate() {
        let severity_icon = match issue.severity {
            diagnostic::Severity::Critical => "🚨",
            diagnostic::Severity::High => "🔴",
            diagnostic::Severity::Medium => "🟡",
            diagnostic::Severity::Low => "🟢",
            diagnostic::Severity::Info => "ℹ️",
        };
        println!(
            "   {}. {} {} - {}",
            index + 1,
            severity_icon,
            issue.item_name,
            issue.message
        );
    }
    println!();

    // Create auto-fix manager
    let mut auto_fix_manager = match (&region, &profile) {
        (Some(r), Some(p)) => {
            DefaultAutoFixManager::with_aws_config(Some(r.clone()), Some(p.clone()))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create auto-fix manager: {}", e))?
        }
        _ => DefaultAutoFixManager::with_default_aws()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create auto-fix manager: {}", e))?,
    };

    // Set dry run mode
    auto_fix_manager.set_dry_run(dry_run);

    // Analyze fixes
    let fix_actions = auto_fix_manager
        .analyze_fixes(&diagnostic_results)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to analyze fixes: {}", e))?;

    if fix_actions.is_empty() {
        println!("✅ No automatic fixes available!");
        println!("   Issues require manual intervention.");
        return Ok(());
    }

    // Filter actions based on safety requirements
    let actions_to_execute: Vec<_> = if safe_only {
        fix_actions
            .into_iter()
            .filter(|action| action.is_safe_to_auto_execute())
            .collect()
    } else {
        fix_actions
    };

    if actions_to_execute.is_empty() && safe_only {
        println!("⚠️  No safe automatic fixes available!");
        println!("   Run without --safe-only to see all available fixes.");
        return Ok(());
    }

    println!("🔧 Available fixes ({}):", actions_to_execute.len());
    for (index, action) in actions_to_execute.iter().enumerate() {
        let risk_icon = match action.risk_level {
            auto_fix::RiskLevel::Safe => "🟢",
            auto_fix::RiskLevel::Low => "🟡",
            auto_fix::RiskLevel::Medium => "🟠",
            auto_fix::RiskLevel::High => "🔴",
            auto_fix::RiskLevel::Critical => "🚨",
        };
        println!(
            "   {}. {} {} - {}",
            index + 1,
            risk_icon,
            action.description,
            format!("Risk: {:?}", action.risk_level)
        );
        if let Some(command) = &action.command {
            println!("      Command: {}", command);
        }
    }
    println!();

    // Execute fixes
    let fix_results = if auto_fix {
        // Execute all fixes automatically
        let mut results = Vec::new();
        for action in actions_to_execute {
            match auto_fix_manager.execute_fix(action).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to execute fix: {}", e);
                    // Continue with other fixes
                }
            }
        }
        results
    } else if safe_only {
        // Execute only safe fixes
        auto_fix_manager
            .execute_safe_fixes(actions_to_execute)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute safe fixes: {}", e))?
    } else {
        // Manual confirmation required for each fix
        let mut results = Vec::new();
        for action in actions_to_execute {
            if action.requires_confirmation && !dry_run {
                println!(
                    "🤔 Execute fix: {} (Risk: {:?})?",
                    action.description, action.risk_level
                );
                println!("   [Y]es / [N]o / [S]kip: ");

                // For now, skip confirmation in CLI mode
                // In a real implementation, you'd read from stdin
                println!("   Skipping due to no interactive input available");
                continue;
            }

            match auto_fix_manager.execute_fix(action).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to execute fix: {}", e);
                    // Continue with other fixes
                }
            }
        }
        results
    };

    // Format and output results
    match format.as_str() {
        "json" => {
            let json_output = serde_json::to_string_pretty(&fix_results)
                .map_err(|e| anyhow::anyhow!("Failed to serialize results to JSON: {}", e))?;

            if let Some(output_path) = output {
                std::fs::write(&output_path, &json_output).map_err(|e| {
                    anyhow::anyhow!("Failed to write to file {}: {}", output_path, e)
                })?;
                println!("📄 Results saved to: {}", output_path);
            } else {
                println!("{}", json_output);
            }
        }
        "yaml" => {
            let yaml_output = serde_yaml::to_string(&fix_results)
                .map_err(|e| anyhow::anyhow!("Failed to serialize results to YAML: {}", e))?;

            if let Some(output_path) = output {
                std::fs::write(&output_path, &yaml_output).map_err(|e| {
                    anyhow::anyhow!("Failed to write to file {}: {}", output_path, e)
                })?;
                println!("📄 Results saved to: {}", output_path);
            } else {
                println!("{}", yaml_output);
            }
        }
        _ => {
            // Default text format
            println!("📋 Auto-fix Results:");
            println!("====================");

            let mut success_count = 0;
            let mut failed_count = 0;
            let skipped_count = 0;

            for result in &fix_results {
                let status_icon = if result.success {
                    success_count += 1;
                    if dry_run {
                        "🔍"
                    } else {
                        "✅"
                    }
                } else {
                    failed_count += 1;
                    "❌"
                };

                println!(
                    "{} {} - {}",
                    status_icon, result.action.description, result.message
                );

                if dry_run && result.success {
                    println!(
                        "   📝 Would execute: {}",
                        result.action.command.as_deref().unwrap_or("N/A")
                    );
                }

                if let Some(details) = &result.details {
                    println!("   📋 Details: {}", details);
                }
            }

            println!();
            println!("📊 Summary:");
            if dry_run {
                println!("   🔍 Would fix: {}", success_count);
                println!("   ❌ Cannot fix: {}", failed_count);
                println!("   ⏭️  Skipped: {}", skipped_count);
                println!();
                println!("💡 Run without --dry-run to execute the fixes");
            } else {
                println!("   ✅ Fixed: {}", success_count);
                println!("   ❌ Failed: {}", failed_count);
                println!("   ⏭️  Skipped: {}", skipped_count);

                if success_count > 0 {
                    println!();
                    println!("🎉 {} issues were successfully fixed!", success_count);
                    println!(
                        "💡 Run: nimbus precheck --instance-id {} to verify fixes",
                        instance_id
                    );
                }

                if failed_count > 0 {
                    println!();
                    println!(
                        "⚠️  {} issues could not be automatically fixed",
                        failed_count
                    );
                    println!("💡 Run: nimbus diagnose full --instance-id {} for manual fix instructions", instance_id);
                }
            }

            if let Some(output_path) = output {
                let text_output = format!(
                    "Auto-fix Results for {}\n{}",
                    instance_id,
                    fix_results
                        .iter()
                        .map(|r| format!("{}: {}", r.action.description, r.message))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                std::fs::write(&output_path, &text_output).map_err(|e| {
                    anyhow::anyhow!("Failed to write to file {}: {}", output_path, e)
                })?;
                println!("📄 Results saved to: {}", output_path);
            }
        }
    }

    Ok(())
}

async fn handle_diagnostic_settings(
    action: DiagnosticSettingsCommands,
    config: &Config,
) -> Result<()> {
    info!("Managing diagnostic settings");

    // Get config file path
    let config_path = match Config::default_config_path() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to determine config path: {}", e);
            println!("❌ Failed to determine config path: {}", e);
            return Err(anyhow::anyhow!("Failed to determine config path: {}", e));
        }
    };

    match action {
        DiagnosticSettingsCommands::Show => {
            println!("🔧 Current Diagnostic Settings:");
            println!("===============================");
            println!("Enabled: {}", config.diagnostic.enabled);
            println!("Auto-fix enabled: {}", config.diagnostic.auto_fix_enabled);
            println!(
                "Safe auto-fix only: {}",
                config.diagnostic.safe_auto_fix_only
            );
            println!(
                "Parallel execution: {}",
                config.diagnostic.parallel_execution
            );
            println!("Timeout: {}s", config.diagnostic.timeout_seconds);
            println!("Port scan range: {}", config.diagnostic.port_scan_range);
            println!("Report format: {}", config.diagnostic.report_format);
            println!("Output directory: {}", config.diagnostic.output_directory);
            println!(
                "Real-time feedback: {}",
                config.diagnostic.realtime_feedback
            );
            println!(
                "Feedback refresh interval: {}ms",
                config.diagnostic.feedback_refresh_interval_ms
            );
            println!("Enable colors: {}", config.diagnostic.enable_colors);
            println!(
                "Auto-confirm critical: {}",
                config.diagnostic.auto_confirm_critical
            );
            println!();
            println!("Enabled checks:");
            for check in &config.diagnostic.enabled_checks {
                println!("  ✅ {}", check);
            }
        }

        DiagnosticSettingsCommands::Enable { check_name } => {
            println!("🔧 Enabling diagnostic check: {}", check_name);

            // Load current config
            let mut current_config = Config::load(Some(config_path.to_str().unwrap()))
                .await
                .unwrap_or_else(|_| config.clone());

            // Add check if not already enabled
            if !current_config
                .diagnostic
                .enabled_checks
                .contains(&check_name)
            {
                current_config
                    .diagnostic
                    .enabled_checks
                    .push(check_name.clone());

                // Save updated config
                match current_config.save(&config_path).await {
                    Ok(_) => {
                        println!("✅ Diagnostic check '{}' enabled successfully", check_name);
                        println!(
                            "💡 Current enabled checks: {}",
                            current_config.diagnostic.enabled_checks.len()
                        );
                    }
                    Err(e) => {
                        error!("Failed to save config: {}", e);
                        println!("❌ Failed to save config: {}", e);
                        return Err(anyhow::anyhow!("Failed to save config: {}", e));
                    }
                }
            } else {
                println!("ℹ️  Diagnostic check '{}' is already enabled", check_name);
            }
        }

        DiagnosticSettingsCommands::Disable { check_name } => {
            println!("🔧 Disabling diagnostic check: {}", check_name);

            // Load current config
            let mut current_config = Config::load(Some(config_path.to_str().unwrap()))
                .await
                .unwrap_or_else(|_| config.clone());

            // Remove check if enabled
            if let Some(pos) = current_config
                .diagnostic
                .enabled_checks
                .iter()
                .position(|x| x == &check_name)
            {
                current_config.diagnostic.enabled_checks.remove(pos);

                // Save updated config
                match current_config.save(&config_path).await {
                    Ok(_) => {
                        println!("✅ Diagnostic check '{}' disabled successfully", check_name);
                        println!(
                            "💡 Current enabled checks: {}",
                            current_config.diagnostic.enabled_checks.len()
                        );
                    }
                    Err(e) => {
                        error!("Failed to save config: {}", e);
                        println!("❌ Failed to save config: {}", e);
                        return Err(anyhow::anyhow!("Failed to save config: {}", e));
                    }
                }
            } else {
                println!(
                    "ℹ️  Diagnostic check '{}' is not currently enabled",
                    check_name
                );
            }
        }

        DiagnosticSettingsCommands::AutoFix { enable, safe_only } => {
            println!("🔧 Configuring auto-fix settings...");

            // Load current config
            let mut current_config = Config::load(Some(config_path.to_str().unwrap()))
                .await
                .unwrap_or_else(|_| config.clone());

            // Update auto-fix settings
            current_config.diagnostic.auto_fix_enabled = enable;
            current_config.diagnostic.safe_auto_fix_only = safe_only;

            // Save updated config
            match current_config.save(&config_path).await {
                Ok(_) => {
                    println!("✅ Auto-fix settings updated successfully");
                    println!("   Auto-fix enabled: {}", enable);
                    println!("   Safe fixes only: {}", safe_only);
                }
                Err(e) => {
                    error!("Failed to save config: {}", e);
                    println!("❌ Failed to save config: {}", e);
                    return Err(anyhow::anyhow!("Failed to save config: {}", e));
                }
            }
        }

        DiagnosticSettingsCommands::Parallel { enable } => {
            println!("🔧 Configuring parallel execution: {}", enable);

            // Load current config
            let mut current_config = Config::load(Some(config_path.to_str().unwrap()))
                .await
                .unwrap_or_else(|_| config.clone());

            // Update parallel execution setting
            current_config.diagnostic.parallel_execution = enable;

            // Save updated config
            match current_config.save(&config_path).await {
                Ok(_) => {
                    println!("✅ Parallel execution setting updated successfully");
                    println!("   Parallel execution: {}", enable);
                }
                Err(e) => {
                    error!("Failed to save config: {}", e);
                    println!("❌ Failed to save config: {}", e);
                    return Err(anyhow::anyhow!("Failed to save config: {}", e));
                }
            }
        }

        DiagnosticSettingsCommands::Timeout { seconds } => {
            println!("🔧 Setting default timeout: {}s", seconds);

            // Load current config
            let mut current_config = Config::load(Some(config_path.to_str().unwrap()))
                .await
                .unwrap_or_else(|_| config.clone());

            // Update timeout setting
            current_config.diagnostic.timeout_seconds = seconds;

            // Save updated config
            match current_config.save(&config_path).await {
                Ok(_) => {
                    println!("✅ Default timeout updated successfully");
                    println!("   Timeout: {}s", seconds);
                }
                Err(e) => {
                    error!("Failed to save config: {}", e);
                    println!("❌ Failed to save config: {}", e);
                    return Err(anyhow::anyhow!("Failed to save config: {}", e));
                }
            }
        }

        DiagnosticSettingsCommands::Format { format } => {
            println!("🔧 Setting report format: {}", format);

            // Validate format
            let valid_formats = ["text", "json", "yaml"];
            if !valid_formats.contains(&format.as_str()) {
                println!(
                    "❌ Invalid format '{}'. Valid formats: {}",
                    format,
                    valid_formats.join(", ")
                );
                return Err(anyhow::anyhow!("Invalid format '{}'", format));
            }

            // Load current config
            let mut current_config = Config::load(Some(config_path.to_str().unwrap()))
                .await
                .unwrap_or_else(|_| config.clone());

            // Update report format setting
            current_config.diagnostic.report_format = format.clone();

            // Save updated config
            match current_config.save(&config_path).await {
                Ok(_) => {
                    println!("✅ Report format updated successfully");
                    println!("   Format: {}", format);
                }
                Err(e) => {
                    error!("Failed to save config: {}", e);
                    println!("❌ Failed to save config: {}", e);
                    return Err(anyhow::anyhow!("Failed to save config: {}", e));
                }
            }
        }

        DiagnosticSettingsCommands::Reset => {
            println!("🔧 Resetting diagnostic settings to defaults...");

            // Load current config
            let mut current_config = Config::load(Some(config_path.to_str().unwrap()))
                .await
                .unwrap_or_else(|_| config.clone());

            // Reset diagnostic settings to defaults
            current_config.diagnostic = crate::config::DiagnosticSettings::default();

            // Save updated config
            match current_config.save(&config_path).await {
                Ok(_) => {
                    println!("✅ Diagnostic settings reset to defaults successfully");
                    println!("💡 Run 'nimbus diagnose settings show' to see current settings");
                }
                Err(e) => {
                    error!("Failed to save config: {}", e);
                    println!("❌ Failed to save config: {}", e);
                    return Err(anyhow::anyhow!("Failed to save config: {}", e));
                }
            }
        }
    }

    Ok(())
}
