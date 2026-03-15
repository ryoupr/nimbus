use clap::Subcommand;

mod config;
mod connect;
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

pub use config::*;
pub use connect::*;
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

#[derive(Subcommand)]
pub enum ConfigCommands {
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
pub enum VsCodeCommands {
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
pub enum DiagnosticCommands {
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
pub enum DiagnosticSettingsCommands {
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
pub enum DatabaseCommands {
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
pub enum Commands {
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
