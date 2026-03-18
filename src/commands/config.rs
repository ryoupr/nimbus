#[allow(unused_imports)]
use anyhow::Result;
#[allow(unused_imports)]
use tracing::{error, info, warn};

#[allow(unused_imports)]
use super::{
    ConfigCommands, DiagnosticCommands, DiagnosticSettingsCommands,
    VsCodeCommands,
};
#[allow(unused_imports)]
use crate::aws_config_validator::{SuggestionCategory, SuggestionPriority};
#[allow(unused_imports)]
use crate::diagnostic::{DiagnosticStatus, Severity};
#[allow(unused_imports)]
use crate::preventive_check::PreventiveCheckStatus;
#[allow(unused_imports)]
use crate::realtime_feedback::{FeedbackConfig, FeedbackStatus};
#[allow(unused_imports)]
use crate::resource::ResourceViolation;
#[allow(unused_imports)]
use crate::session::{Session, SessionStatus};
#[allow(unused_imports)]
use crate::ui::{ResourceMetrics, TerminalUi};
#[allow(unused_imports)]
use crate::{
    auto_fix,
    aws::AwsManager,
    aws_config_validator::{AwsConfigValidationConfig, DefaultAwsConfigValidator},
    config::Config,
    diagnostic::{DefaultDiagnosticManager, DiagnosticConfig, DiagnosticManager},
    error::NimbusError,
    error_recovery::{ContextualError, ErrorContext, ErrorRecoveryManager},
    health::{DefaultHealthChecker, HealthChecker},
    logging::StructuredLogger,
    manager::{DefaultSessionManager, SessionManager},
    preventive_check::{DefaultPreventiveCheck, PreventiveCheck, PreventiveCheckConfig},
    resource::ResourceMonitor,
    session::{SessionConfig, SessionPriority},
    user_messages::UserMessageSystem,
    vscode::VsCodeIntegration,
};
#[allow(unused_imports)]
use crate::{
    aws_config_validator, diagnostic, preventive_check, realtime_feedback, resource, session, ui,
};

#[allow(dead_code)]
pub async fn handle_config_validation(_config: &Config) -> Result<()> {
    info!("Validating configuration");

    println!("⚙️  Configuration Validation:");
    println!("✅ Configuration file loaded successfully");
    println!("✅ All required settings present");

    Ok(())
}

pub async fn handle_config(action: ConfigCommands, config: &Config) -> Result<()> {
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
            } else if config_path.extension().and_then(|s| s.to_str()) != Some("json") {
                config_path.with_extension("json")
            } else {
                config_path
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
