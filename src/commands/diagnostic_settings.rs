#[allow(unused_imports)]
use anyhow::Result;
#[allow(unused_imports)]
use tracing::{error, info, warn};

#[allow(unused_imports)]
use super::{
    ConfigCommands, DatabaseCommands, DiagnosticCommands, DiagnosticSettingsCommands,
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

pub async fn handle_diagnostic_settings(
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
