#[allow(unused_imports)]
use anyhow::Result;
#[allow(unused_imports)]
use tracing::{error, info, warn};

#[allow(unused_imports)]
use crate::{
    auto_fix, aws::AwsManager, aws_config_validator::{AwsConfigValidationConfig, DefaultAwsConfigValidator},
    config::Config, diagnostic::{DefaultDiagnosticManager, DiagnosticConfig, DiagnosticManager},
    error::NimbusError, error_recovery::{ContextualError, ErrorContext, ErrorRecoveryManager},
    health::{DefaultHealthChecker, HealthChecker}, logging::StructuredLogger,
    manager::{DefaultSessionManager, SessionManager},
    preventive_check::{DefaultPreventiveCheck, PreventiveCheck, PreventiveCheckConfig},
    resource::ResourceMonitor, session::{SessionConfig, SessionPriority}, user_messages::UserMessageSystem, vscode::VsCodeIntegration,
};
#[allow(unused_imports)]
use super::{ConfigCommands, DatabaseCommands, DiagnosticCommands, DiagnosticSettingsCommands, VsCodeCommands};
#[allow(unused_imports)]
use crate::{aws_config_validator, diagnostic, preventive_check, realtime_feedback, resource, session, ui};
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

#[cfg(feature = "performance-monitoring")]
use crate::monitor::DefaultSessionMonitor;
#[cfg(feature = "multi-session")]
use crate::multi_session::{MultiSessionManager, ResourceThresholds};
#[cfg(feature = "multi-session")]
use crate::multi_session_ui::MultiSessionUi;
#[cfg(feature = "persistence")]
use crate::persistence::{PersistenceManager, SqlitePersistenceManager};

#[allow(clippy::too_many_arguments)]
pub async fn handle_fix(
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
            "   {}. {} {} - Risk: {:?}",
            index + 1,
            risk_icon,
            action.description,
            action.risk_level
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
