use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use std::env;
use anyhow::{Result, Context};
use tokio::fs;
use crate::session::ReconnectionPolicy;
use crate::vscode::VsCodeConfig;

/// Main configuration structure for EC2 Connect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// AWS configuration
    pub aws: AwsConfig,
    
    /// Session management configuration
    pub session: SessionConfig,
    
    /// Performance monitoring configuration
    pub performance: PerformanceConfig,
    
    /// Resource limits configuration
    pub resources: ResourceConfig,
    
    /// UI configuration
    pub ui: UiConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// VS Code integration configuration
    pub vscode: VsCodeConfig,
    
    /// Diagnostic configuration
    pub diagnostic: DiagnosticSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    /// Default AWS profile
    pub default_profile: Option<String>,
    
    /// Default AWS region
    pub default_region: String,
    
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    
    /// Request timeout in seconds
    pub request_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Maximum number of concurrent sessions per instance
    pub max_sessions_per_instance: u32,
    
    /// Session health check interval in seconds
    pub health_check_interval: u64,
    
    /// Session timeout prediction threshold in seconds
    pub timeout_prediction_threshold: u64,
    
    /// Inactive session detection timeout in seconds
    pub inactive_timeout: u64,
    
    /// Auto-reconnection configuration
    pub reconnection: ReconnectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectionConfig {
    /// Enable automatic reconnection
    pub enabled: bool,
    
    /// Maximum number of reconnection attempts
    pub max_attempts: u32,
    
    /// Base delay between reconnection attempts in milliseconds
    pub base_delay_ms: u64,
    
    /// Maximum delay between reconnection attempts in milliseconds
    pub max_delay_ms: u64,
    
    /// Enable aggressive reconnection mode
    pub aggressive_mode: bool,
    
    /// Number of aggressive reconnection attempts
    pub aggressive_attempts: u32,
    
    /// Interval between aggressive attempts in milliseconds
    pub aggressive_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable performance monitoring
    pub monitoring_enabled: bool,
    
    /// Performance metrics collection interval in seconds
    pub metrics_interval: u64,
    
    /// Connection latency threshold in milliseconds
    pub latency_threshold_ms: u64,
    
    /// Enable connection optimization
    pub optimization_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    
    /// Enable low power mode
    pub low_power_mode: bool,
    
    /// Resource monitoring interval in seconds
    pub monitoring_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Enable rich terminal UI
    pub rich_ui: bool,
    
    /// UI update interval in milliseconds
    pub update_interval_ms: u64,
    
    /// Show progress indicators
    pub show_progress: bool,
    
    /// Enable desktop notifications
    pub notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    
    /// Enable file logging
    pub file_logging: bool,
    
    /// Log file path
    pub log_file: Option<PathBuf>,
    
    /// Enable structured JSON logging
    pub json_format: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSettings {
    /// Enable diagnostic functionality
    pub enabled: bool,
    
    /// List of enabled diagnostic items
    pub enabled_checks: Vec<String>,
    
    /// Enable automatic fixes
    pub auto_fix_enabled: bool,
    
    /// Only run safe automatic fixes
    pub safe_auto_fix_only: bool,
    
    /// Enable parallel execution of diagnostics
    pub parallel_execution: bool,
    
    /// Default timeout for diagnostics in seconds
    pub timeout_seconds: u64,
    
    /// Port scan range for local port diagnostics
    pub port_scan_range: u16,
    
    /// Default report format (text, json, yaml)
    pub report_format: String,
    
    /// Default output directory for reports
    pub output_directory: String,
    
    /// Enable real-time feedback during diagnostics
    pub realtime_feedback: bool,
    
    /// Refresh interval for real-time feedback in milliseconds
    pub feedback_refresh_interval_ms: u64,
    
    /// Enable color coding in output
    pub enable_colors: bool,
    
    /// Auto-confirm critical issues during interactive diagnostics
    pub auto_confirm_critical: bool,
}

impl Default for DiagnosticSettings {
    fn default() -> Self {
        // Use standard data directory for diagnostic reports
        let reports_dir = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
            .map(|d| d.join("ec2-connect").join("reports"))
            .unwrap_or_else(|| PathBuf::from("reports"));
            
        Self {
            enabled: true,
            enabled_checks: vec![
                "instance_state".to_string(),
                "ssm_agent".to_string(),
                "iam_permissions".to_string(),
                "vpc_endpoints".to_string(),
                "security_groups".to_string(),
                "network_connectivity".to_string(),
                "local_port_availability".to_string(),
            ],
            auto_fix_enabled: false,
            safe_auto_fix_only: true,
            parallel_execution: true,
            timeout_seconds: 30,
            port_scan_range: 10,
            report_format: "text".to_string(),
            output_directory: reports_dir.to_string_lossy().to_string(),
            realtime_feedback: true,
            feedback_refresh_interval_ms: 100,
            enable_colors: true,
            auto_confirm_critical: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            aws: AwsConfig {
                default_profile: None,
                default_region: "us-east-1".to_string(),
                connection_timeout: 30,
                request_timeout: 60,
            },
            session: SessionConfig {
                max_sessions_per_instance: 3,
                health_check_interval: 5,
                timeout_prediction_threshold: 300, // 5 minutes
                inactive_timeout: 30,
                reconnection: ReconnectionConfig {
                    enabled: true,
                    max_attempts: 5,
                    base_delay_ms: 1000,
                    max_delay_ms: 16000,
                    aggressive_mode: false,
                    aggressive_attempts: 10,
                    aggressive_interval_ms: 500,
                },
            },
            performance: PerformanceConfig {
                monitoring_enabled: true,
                metrics_interval: 10,
                latency_threshold_ms: 200,
                optimization_enabled: true,
            },
            resources: ResourceConfig {
                max_memory_mb: 10,
                max_cpu_percent: 0.5,
                low_power_mode: true,
                monitoring_interval: 5,
            },
            ui: UiConfig {
                rich_ui: true,
                update_interval_ms: 1000,
                show_progress: true,
                notifications: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file_logging: true,
                log_file: None,
                json_format: false,
            },
            vscode: VsCodeConfig::default(),
            diagnostic: DiagnosticSettings::default(),
        }
    }
}

impl Config {
    /// Load configuration from file or create default
    pub async fn load(config_path: Option<&str>) -> Result<Self> {
        let config_file = match config_path {
            Some(path) => PathBuf::from(path),
            None => Self::default_config_path()?,
        };
        
        let mut config = if config_file.exists() {
            let content = fs::read_to_string(&config_file)
                .await
                .with_context(|| format!("Failed to read config file: {:?}", config_file))?;
            
            let config: Config = if config_file.extension().and_then(|s| s.to_str()) == Some("toml") {
                toml::from_str(&content)
                    .with_context(|| format!("Failed to parse TOML config: {:?}", config_file))?
            } else {
                serde_json::from_str(&content)
                    .with_context(|| format!("Failed to parse JSON config: {:?}", config_file))?
            };
            
            config
        } else {
            // Create default configuration
            let config = Self::default();
            config.save(&config_file).await?;
            config
        };
        
        // Apply environment variable overrides
        config.apply_env_overrides()?;
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Apply environment variable overrides to configuration
    pub fn apply_env_overrides(&mut self) -> Result<()> {
        // AWS configuration overrides
        if let Ok(profile) = env::var("EC2_CONNECT_AWS_PROFILE") {
            self.aws.default_profile = Some(profile);
        }
        
        if let Ok(region) = env::var("EC2_CONNECT_AWS_REGION") {
            self.aws.default_region = region;
        }
        
        if let Ok(timeout) = env::var("EC2_CONNECT_CONNECTION_TIMEOUT") {
            self.aws.connection_timeout = timeout.parse()
                .with_context(|| "Invalid EC2_CONNECT_CONNECTION_TIMEOUT value")?;
        }
        
        if let Ok(timeout) = env::var("EC2_CONNECT_REQUEST_TIMEOUT") {
            self.aws.request_timeout = timeout.parse()
                .with_context(|| "Invalid EC2_CONNECT_REQUEST_TIMEOUT value")?;
        }
        
        // Session configuration overrides
        if let Ok(max_sessions) = env::var("EC2_CONNECT_MAX_SESSIONS") {
            self.session.max_sessions_per_instance = max_sessions.parse()
                .with_context(|| "Invalid EC2_CONNECT_MAX_SESSIONS value")?;
        }
        
        if let Ok(health_interval) = env::var("EC2_CONNECT_HEALTH_CHECK_INTERVAL") {
            self.session.health_check_interval = health_interval.parse()
                .with_context(|| "Invalid EC2_CONNECT_HEALTH_CHECK_INTERVAL value")?;
        }
        
        if let Ok(inactive_timeout) = env::var("EC2_CONNECT_INACTIVE_TIMEOUT") {
            self.session.inactive_timeout = inactive_timeout.parse()
                .with_context(|| "Invalid EC2_CONNECT_INACTIVE_TIMEOUT value")?;
        }
        
        // Reconnection policy overrides
        if let Ok(enabled) = env::var("EC2_CONNECT_RECONNECTION_ENABLED") {
            self.session.reconnection.enabled = enabled.parse()
                .with_context(|| "Invalid EC2_CONNECT_RECONNECTION_ENABLED value (use true/false)")?;
        }
        
        if let Ok(max_attempts) = env::var("EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS") {
            self.session.reconnection.max_attempts = max_attempts.parse()
                .with_context(|| "Invalid EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS value")?;
        }
        
        if let Ok(base_delay) = env::var("EC2_CONNECT_RECONNECTION_BASE_DELAY_MS") {
            self.session.reconnection.base_delay_ms = base_delay.parse()
                .with_context(|| "Invalid EC2_CONNECT_RECONNECTION_BASE_DELAY_MS value")?;
        }
        
        if let Ok(max_delay) = env::var("EC2_CONNECT_RECONNECTION_MAX_DELAY_MS") {
            self.session.reconnection.max_delay_ms = max_delay.parse()
                .with_context(|| "Invalid EC2_CONNECT_RECONNECTION_MAX_DELAY_MS value")?;
        }
        
        if let Ok(aggressive) = env::var("EC2_CONNECT_AGGRESSIVE_RECONNECTION") {
            self.session.reconnection.aggressive_mode = aggressive.parse()
                .with_context(|| "Invalid EC2_CONNECT_AGGRESSIVE_RECONNECTION value (use true/false)")?;
        }
        
        if let Ok(aggressive_attempts) = env::var("EC2_CONNECT_AGGRESSIVE_ATTEMPTS") {
            self.session.reconnection.aggressive_attempts = aggressive_attempts.parse()
                .with_context(|| "Invalid EC2_CONNECT_AGGRESSIVE_ATTEMPTS value")?;
        }
        
        if let Ok(aggressive_interval) = env::var("EC2_CONNECT_AGGRESSIVE_INTERVAL_MS") {
            self.session.reconnection.aggressive_interval_ms = aggressive_interval.parse()
                .with_context(|| "Invalid EC2_CONNECT_AGGRESSIVE_INTERVAL_MS value")?;
        }
        
        // Performance configuration overrides
        if let Ok(monitoring) = env::var("EC2_CONNECT_PERFORMANCE_MONITORING") {
            self.performance.monitoring_enabled = monitoring.parse()
                .with_context(|| "Invalid EC2_CONNECT_PERFORMANCE_MONITORING value (use true/false)")?;
        }
        
        if let Ok(threshold) = env::var("EC2_CONNECT_LATENCY_THRESHOLD_MS") {
            self.performance.latency_threshold_ms = threshold.parse()
                .with_context(|| "Invalid EC2_CONNECT_LATENCY_THRESHOLD_MS value")?;
        }
        
        if let Ok(optimization) = env::var("EC2_CONNECT_OPTIMIZATION_ENABLED") {
            self.performance.optimization_enabled = optimization.parse()
                .with_context(|| "Invalid EC2_CONNECT_OPTIMIZATION_ENABLED value (use true/false)")?;
        }
        
        // Resource configuration overrides
        if let Ok(max_memory) = env::var("EC2_CONNECT_MAX_MEMORY_MB") {
            self.resources.max_memory_mb = max_memory.parse()
                .with_context(|| "Invalid EC2_CONNECT_MAX_MEMORY_MB value")?;
        }
        
        if let Ok(max_cpu) = env::var("EC2_CONNECT_MAX_CPU_PERCENT") {
            self.resources.max_cpu_percent = max_cpu.parse()
                .with_context(|| "Invalid EC2_CONNECT_MAX_CPU_PERCENT value")?;
        }
        
        if let Ok(low_power) = env::var("EC2_CONNECT_LOW_POWER_MODE") {
            self.resources.low_power_mode = low_power.parse()
                .with_context(|| "Invalid EC2_CONNECT_LOW_POWER_MODE value (use true/false)")?;
        }
        
        // UI configuration overrides
        if let Ok(rich_ui) = env::var("EC2_CONNECT_RICH_UI") {
            self.ui.rich_ui = rich_ui.parse()
                .with_context(|| "Invalid EC2_CONNECT_RICH_UI value (use true/false)")?;
        }
        
        if let Ok(update_interval) = env::var("EC2_CONNECT_UI_UPDATE_INTERVAL_MS") {
            self.ui.update_interval_ms = update_interval.parse()
                .with_context(|| "Invalid EC2_CONNECT_UI_UPDATE_INTERVAL_MS value")?;
        }
        
        if let Ok(notifications) = env::var("EC2_CONNECT_NOTIFICATIONS") {
            self.ui.notifications = notifications.parse()
                .with_context(|| "Invalid EC2_CONNECT_NOTIFICATIONS value (use true/false)")?;
        }
        
        // Logging configuration overrides
        if let Ok(level) = env::var("EC2_CONNECT_LOG_LEVEL") {
            let valid_levels = ["trace", "debug", "info", "warn", "error"];
            if !valid_levels.contains(&level.to_lowercase().as_str()) {
                anyhow::bail!("Invalid EC2_CONNECT_LOG_LEVEL value. Must be one of: {}", valid_levels.join(", "));
            }
            self.logging.level = level.to_lowercase();
        }
        
        if let Ok(file_logging) = env::var("EC2_CONNECT_FILE_LOGGING") {
            self.logging.file_logging = file_logging.parse()
                .with_context(|| "Invalid EC2_CONNECT_FILE_LOGGING value (use true/false)")?;
        }
        
        if let Ok(log_file) = env::var("EC2_CONNECT_LOG_FILE") {
            self.logging.log_file = Some(PathBuf::from(log_file));
        }
        
        if let Ok(json_format) = env::var("EC2_CONNECT_JSON_LOGGING") {
            self.logging.json_format = json_format.parse()
                .with_context(|| "Invalid EC2_CONNECT_JSON_LOGGING value (use true/false)")?;
        }
        
        // VS Code integration overrides
        if let Ok(vscode_path) = env::var("EC2_CONNECT_VSCODE_PATH") {
            self.vscode.vscode_path = Some(vscode_path);
        }
        
        if let Ok(ssh_config_path) = env::var("EC2_CONNECT_SSH_CONFIG_PATH") {
            self.vscode.ssh_config_path = Some(ssh_config_path);
        }
        
        if let Ok(auto_launch) = env::var("EC2_CONNECT_VSCODE_AUTO_LAUNCH") {
            self.vscode.auto_launch_enabled = auto_launch.parse()
                .with_context(|| "Invalid EC2_CONNECT_VSCODE_AUTO_LAUNCH value (use true/false)")?;
        }
        
        if let Ok(notifications) = env::var("EC2_CONNECT_VSCODE_NOTIFICATIONS") {
            self.vscode.notifications_enabled = notifications.parse()
                .with_context(|| "Invalid EC2_CONNECT_VSCODE_NOTIFICATIONS value (use true/false)")?;
        }
        
        if let Ok(delay) = env::var("EC2_CONNECT_VSCODE_LAUNCH_DELAY") {
            self.vscode.launch_delay_seconds = delay.parse()
                .with_context(|| "Invalid EC2_CONNECT_VSCODE_LAUNCH_DELAY value")?;
        }
        
        if let Ok(auto_update) = env::var("EC2_CONNECT_VSCODE_AUTO_UPDATE_SSH") {
            self.vscode.auto_update_ssh_config = auto_update.parse()
                .with_context(|| "Invalid EC2_CONNECT_VSCODE_AUTO_UPDATE_SSH value (use true/false)")?;
        }
        
        Ok(())
    }
    
    /// Save configuration to file
    pub async fn save(&self, config_path: &PathBuf) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        let content = if config_path.extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::to_string_pretty(self)?
        } else {
            serde_json::to_string_pretty(self)?
        };
        
        fs::write(config_path, content).await
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;
        
        Ok(())
    }
    
    /// Get default configuration file path
    pub fn default_config_path() -> Result<PathBuf> {
        // Use standard config directory as documented in README
        let config_dir = dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
            .context("Could not determine config directory")?;
        
        Ok(config_dir.join("ec2-connect").join("config.json"))
    }
    
    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate resource limits
        if self.resources.max_memory_mb == 0 {
            anyhow::bail!("max_memory_mb must be greater than 0");
        }
        
        if self.resources.max_memory_mb > 1024 {
            tracing::warn!("max_memory_mb is set to {}MB, which is higher than recommended (10MB)", self.resources.max_memory_mb);
        }
        
        if self.resources.max_cpu_percent <= 0.0 || self.resources.max_cpu_percent > 100.0 {
            anyhow::bail!("max_cpu_percent must be between 0.0 and 100.0");
        }
        
        if self.resources.max_cpu_percent > 5.0 {
            tracing::warn!("max_cpu_percent is set to {}%, which is higher than recommended (0.5%)", self.resources.max_cpu_percent);
        }
        
        // Validate session configuration
        if self.session.max_sessions_per_instance == 0 {
            anyhow::bail!("max_sessions_per_instance must be greater than 0");
        }
        
        if self.session.max_sessions_per_instance > 10 {
            tracing::warn!("max_sessions_per_instance is set to {}, which may cause resource issues", self.session.max_sessions_per_instance);
        }
        
        if self.session.health_check_interval == 0 {
            anyhow::bail!("health_check_interval must be greater than 0");
        }
        
        if self.session.health_check_interval > 60 {
            tracing::warn!("health_check_interval is set to {}s, which may delay disconnect detection", self.session.health_check_interval);
        }
        
        if self.session.inactive_timeout < 10 {
            tracing::warn!("inactive_timeout is set to {}s, which may cause premature session termination", self.session.inactive_timeout);
        }
        
        // Validate reconnection configuration
        if self.session.reconnection.enabled {
            if self.session.reconnection.max_attempts == 0 {
                anyhow::bail!("max_attempts must be greater than 0 when reconnection is enabled");
            }
            
            if self.session.reconnection.max_attempts > 20 {
                tracing::warn!("max_attempts is set to {}, which may cause excessive retry attempts", self.session.reconnection.max_attempts);
            }
            
            if self.session.reconnection.base_delay_ms == 0 {
                anyhow::bail!("base_delay_ms must be greater than 0");
            }
            
            if self.session.reconnection.base_delay_ms > 10000 {
                tracing::warn!("base_delay_ms is set to {}ms, which may cause slow reconnection", self.session.reconnection.base_delay_ms);
            }
            
            if self.session.reconnection.max_delay_ms < self.session.reconnection.base_delay_ms {
                anyhow::bail!("max_delay_ms must be greater than or equal to base_delay_ms");
            }
            
            if self.session.reconnection.aggressive_mode {
                if self.session.reconnection.aggressive_attempts == 0 {
                    anyhow::bail!("aggressive_attempts must be greater than 0 when aggressive_mode is enabled");
                }
                
                if self.session.reconnection.aggressive_interval_ms == 0 {
                    anyhow::bail!("aggressive_interval_ms must be greater than 0 when aggressive_mode is enabled");
                }
                
                if self.session.reconnection.aggressive_interval_ms < 100 {
                    tracing::warn!("aggressive_interval_ms is set to {}ms, which may cause excessive load", self.session.reconnection.aggressive_interval_ms);
                }
            }
        }
        
        // Validate performance configuration
        if self.performance.latency_threshold_ms == 0 {
            anyhow::bail!("latency_threshold_ms must be greater than 0");
        }
        
        if self.performance.metrics_interval == 0 {
            anyhow::bail!("metrics_interval must be greater than 0");
        }
        
        // Validate UI configuration
        if self.ui.update_interval_ms == 0 {
            anyhow::bail!("update_interval_ms must be greater than 0");
        }
        
        if self.ui.update_interval_ms < 100 {
            tracing::warn!("update_interval_ms is set to {}ms, which may cause high CPU usage", self.ui.update_interval_ms);
        }
        
        // Validate logging configuration
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            anyhow::bail!("Invalid log level '{}'. Must be one of: {}", self.logging.level, valid_levels.join(", "));
        }
        
        // Validate AWS configuration
        if self.aws.connection_timeout == 0 {
            anyhow::bail!("connection_timeout must be greater than 0");
        }
        
        if self.aws.request_timeout == 0 {
            anyhow::bail!("request_timeout must be greater than 0");
        }
        
        if self.aws.connection_timeout > self.aws.request_timeout {
            tracing::warn!("connection_timeout ({}s) is greater than request_timeout ({}s)", 
                         self.aws.connection_timeout, self.aws.request_timeout);
        }
        
        Ok(())
    }
    
    /// Get reconnection delay for attempt number
    pub fn get_reconnection_delay(&self, attempt: u32) -> Duration {
        if self.session.reconnection.aggressive_mode && attempt <= self.session.reconnection.aggressive_attempts {
            Duration::from_millis(self.session.reconnection.aggressive_interval_ms)
        } else {
            let delay_ms = std::cmp::min(
                self.session.reconnection.base_delay_ms * (2_u64.pow(attempt.saturating_sub(1))),
                self.session.reconnection.max_delay_ms,
            );
            Duration::from_millis(delay_ms)
        }
    }
    
    /// Print configuration summary for debugging
    pub fn print_summary(&self) {
        tracing::info!("Configuration Summary:");
        tracing::info!("  AWS Region: {}", self.aws.default_region);
        tracing::info!("  AWS Profile: {:?}", self.aws.default_profile);
        tracing::info!("  Max Sessions per Instance: {}", self.session.max_sessions_per_instance);
        tracing::info!("  Health Check Interval: {}s", self.session.health_check_interval);
        tracing::info!("  Reconnection Enabled: {}", self.session.reconnection.enabled);
        tracing::info!("  Max Memory: {}MB", self.resources.max_memory_mb);
        tracing::info!("  Max CPU: {}%", self.resources.max_cpu_percent);
        tracing::info!("  Log Level: {}", self.logging.level);
    }
    
    /// Get list of all supported environment variables
    pub fn get_env_variables_help() -> Vec<(&'static str, &'static str)> {
        vec![
            ("EC2_CONNECT_AWS_PROFILE", "AWS profile to use for connections"),
            ("EC2_CONNECT_AWS_REGION", "AWS region for EC2 instances"),
            ("EC2_CONNECT_CONNECTION_TIMEOUT", "Connection timeout in seconds"),
            ("EC2_CONNECT_REQUEST_TIMEOUT", "Request timeout in seconds"),
            ("EC2_CONNECT_MAX_SESSIONS", "Maximum sessions per instance"),
            ("EC2_CONNECT_HEALTH_CHECK_INTERVAL", "Health check interval in seconds"),
            ("EC2_CONNECT_INACTIVE_TIMEOUT", "Inactive session timeout in seconds"),
            ("EC2_CONNECT_RECONNECTION_ENABLED", "Enable automatic reconnection (true/false)"),
            ("EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS", "Maximum reconnection attempts"),
            ("EC2_CONNECT_RECONNECTION_BASE_DELAY_MS", "Base delay between reconnection attempts (ms)"),
            ("EC2_CONNECT_RECONNECTION_MAX_DELAY_MS", "Maximum delay between reconnection attempts (ms)"),
            ("EC2_CONNECT_AGGRESSIVE_RECONNECTION", "Enable aggressive reconnection mode (true/false)"),
            ("EC2_CONNECT_AGGRESSIVE_ATTEMPTS", "Number of aggressive reconnection attempts"),
            ("EC2_CONNECT_AGGRESSIVE_INTERVAL_MS", "Interval between aggressive attempts (ms)"),
            ("EC2_CONNECT_PERFORMANCE_MONITORING", "Enable performance monitoring (true/false)"),
            ("EC2_CONNECT_LATENCY_THRESHOLD_MS", "Latency threshold for optimization (ms)"),
            ("EC2_CONNECT_OPTIMIZATION_ENABLED", "Enable connection optimization (true/false)"),
            ("EC2_CONNECT_MAX_MEMORY_MB", "Maximum memory usage (MB)"),
            ("EC2_CONNECT_MAX_CPU_PERCENT", "Maximum CPU usage (%)"),
            ("EC2_CONNECT_LOW_POWER_MODE", "Enable low power mode (true/false)"),
            ("EC2_CONNECT_RICH_UI", "Enable rich terminal UI (true/false)"),
            ("EC2_CONNECT_UI_UPDATE_INTERVAL_MS", "UI update interval (ms)"),
            ("EC2_CONNECT_NOTIFICATIONS", "Enable desktop notifications (true/false)"),
            ("EC2_CONNECT_LOG_LEVEL", "Log level (trace/debug/info/warn/error)"),
            ("EC2_CONNECT_FILE_LOGGING", "Enable file logging (true/false)"),
            ("EC2_CONNECT_LOG_FILE", "Path to log file"),
            ("EC2_CONNECT_JSON_LOGGING", "Enable JSON log format (true/false)"),
            ("EC2_CONNECT_VSCODE_PATH", "Path to VS Code executable"),
            ("EC2_CONNECT_SSH_CONFIG_PATH", "Path to SSH config file"),
            ("EC2_CONNECT_VSCODE_AUTO_LAUNCH", "Enable VS Code auto launch (true/false)"),
            ("EC2_CONNECT_VSCODE_NOTIFICATIONS", "Enable VS Code integration notifications (true/false)"),
            ("EC2_CONNECT_VSCODE_LAUNCH_DELAY", "Delay before launching VS Code (seconds)"),
            ("EC2_CONNECT_VSCODE_AUTO_UPDATE_SSH", "Auto update SSH config (true/false)"),
        ]
    }
}

// Add dirs dependency to Cargo.toml for config directory detection

impl ReconnectionConfig {
    /// Convert to ReconnectionPolicy
    pub fn to_policy(&self) -> ReconnectionPolicy {
        ReconnectionPolicy {
            enabled: self.enabled,
            max_attempts: self.max_attempts,
            base_delay: Duration::from_millis(self.base_delay_ms),
            max_delay: Duration::from_millis(self.max_delay_ms),
            aggressive_mode: self.aggressive_mode,
            aggressive_attempts: self.aggressive_attempts,
            aggressive_interval: Duration::from_millis(self.aggressive_interval_ms),
        }
    }
    
    /// Create from ReconnectionPolicy
    pub fn from_policy(policy: &ReconnectionPolicy) -> Self {
        Self {
            enabled: policy.enabled,
            max_attempts: policy.max_attempts,
            base_delay_ms: policy.base_delay.as_millis() as u64,
            max_delay_ms: policy.max_delay.as_millis() as u64,
            aggressive_mode: policy.aggressive_mode,
            aggressive_attempts: policy.aggressive_attempts,
            aggressive_interval_ms: policy.aggressive_interval.as_millis() as u64,
        }
    }
}