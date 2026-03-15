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

pub async fn handle_diagnose(action: DiagnosticCommands, _config: &Config) -> Result<()> {
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
                                    diagnostic::Severity::Critical | diagnostic::Severity::High
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
                        println!(
                            "💡 Run: nimbus diagnose full --instance-id {} for detailed analysis",
                            results
                                .first()
                                .map(|r| r.item_name.as_str())
                                .unwrap_or("INSTANCE_ID")
                        );
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
                        println!(
                            "   Run 'nimbus diagnose full --instance-id {}' for detailed analysis.",
                            instance_id
                        );
                    } else {
                        match result.overall_status {
                            preventive_check::PreventiveCheckStatus::Ready => {
                                println!("🚀 All checks passed! You can proceed with connection.");
                                println!("   Run: nimbus connect --instance-id {}", instance_id);
                            }
                            preventive_check::PreventiveCheckStatus::Warning => {
                                println!("⚠️  Connection can proceed but with warnings.");
                                println!(
                                    "   Consider addressing warnings for optimal performance."
                                );
                                println!("   Run: nimbus connect --instance-id {}", instance_id);
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
            println!(
                "   nimbus diagnose item --item instance_state --instance-id i-1234567890abcdef0"
            );
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
                                aws_config_validator::SuggestionPriority::Critical => "🚨",
                                aws_config_validator::SuggestionPriority::High => "🔴",
                                aws_config_validator::SuggestionPriority::Medium => "🟡",
                                aws_config_validator::SuggestionPriority::Low => "🟢",
                            };

                            let category_text = match suggestion.category {
                                aws_config_validator::SuggestionCategory::Credentials => {
                                    "Credentials"
                                }
                                aws_config_validator::SuggestionCategory::IamPermissions => {
                                    "IAM Permissions"
                                }
                                aws_config_validator::SuggestionCategory::VpcConfiguration => {
                                    "VPC Configuration"
                                }
                                aws_config_validator::SuggestionCategory::SecurityGroups => {
                                    "Security Groups"
                                }
                                aws_config_validator::SuggestionCategory::NetworkConnectivity => {
                                    "Network Connectivity"
                                }
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
                        println!(
                            "   • Run 'nimbus connect --instance-id {}' to test the connection",
                            instance_id
                        );
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
                                aws_config_validator::SuggestionPriority::Critical => "🚨",
                                aws_config_validator::SuggestionPriority::High => "🔴",
                                aws_config_validator::SuggestionPriority::Medium => "🟡",
                                aws_config_validator::SuggestionPriority::Low => "🟢",
                            };

                            let category_text = match suggestion.category {
                                aws_config_validator::SuggestionCategory::Credentials => {
                                    "Credentials"
                                }
                                aws_config_validator::SuggestionCategory::IamPermissions => {
                                    "IAM Permissions"
                                }
                                aws_config_validator::SuggestionCategory::VpcConfiguration => {
                                    "VPC Configuration"
                                }
                                aws_config_validator::SuggestionCategory::SecurityGroups => {
                                    "Security Groups"
                                }
                                aws_config_validator::SuggestionCategory::NetworkConnectivity => {
                                    "Network Connectivity"
                                }
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
                        println!(
                            "   • Run 'nimbus connect --instance-id {}' to test the connection",
                            instance_id
                        );
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
                            diagnostic::DiagnosticStatus::Success => success_count += 1,
                            diagnostic::DiagnosticStatus::Warning => warning_count += 1,
                            diagnostic::DiagnosticStatus::Error => {
                                error_count += 1;
                                if matches!(result.severity, diagnostic::Severity::Critical) {
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
                    let _feedback_status = diagnostic_manager.get_feedback_status();

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
            super::handle_diagnostic_settings(action, _config).await?
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_precheck(
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
                                    diagnostic::Severity::Critical | diagnostic::Severity::High
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
                        println!(
                            "💡 Run: nimbus fix --instance-id {} --auto-fix for automatic fixes",
                            instance_id
                        );
                        println!(
                            "💡 Run: nimbus diagnose full --instance-id {} for detailed analysis",
                            instance_id
                        );
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
