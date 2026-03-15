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

/// Generic recovery wrapper that eliminates duplicated error-handling logic.
///
/// Executes `operation`, and on failure:
/// 1. Converts the error to `NimbusError` and logs it
/// 2. If recoverable, runs the recovery manager then retries `operation` once
/// 3. Displays user-friendly error messages on final failure
async fn with_recovery<F, Fut>(
    operation: F,
    context: ErrorContext,
    recovery_manager: &ErrorRecoveryManager,
    message_system: &UserMessageSystem,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    match operation().await {
        Ok(()) => Ok(()),
        Err(e) => {
            let ec2_error = match e.downcast::<NimbusError>() {
                Ok(ec2_err) => ec2_err,
                Err(other_err) => NimbusError::System(other_err.to_string()),
            };

            let contextual_error = ContextualError::new(ec2_error, context);
            StructuredLogger::log_error(&contextual_error);

            if contextual_error.error.is_recoverable() {
                warn!(
                    "{} failed, attempting recovery: {}",
                    contextual_error.context.operation, contextual_error.error
                );

                let recovery_operation =
                    || -> crate::error::Result<()> { Err(contextual_error.error.clone()) };

                match recovery_manager
                    .recover(recovery_operation, &contextual_error.error)
                    .await
                {
                    Ok(_) => match operation().await {
                        Ok(()) => {
                            info!("Operation recovered successfully after retry");
                            Ok(())
                        }
                        Err(retry_error) => {
                            let retry_ec2_error = match retry_error.downcast::<NimbusError>() {
                                Ok(ec2_err) => ec2_err,
                                Err(other_err) => NimbusError::System(other_err.to_string()),
                            };
                            let user_message = message_system.get_error_message(&retry_ec2_error);
                            eprintln!("{}", user_message.format_for_display());
                            Err(retry_ec2_error.into())
                        }
                    },
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

#[allow(clippy::too_many_arguments)]
pub async fn handle_connect_with_recovery(
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

    let config = config.clone();
    with_recovery(
        || {
            let (instance_id, remote_host, profile, region, priority, config) = (
                instance_id.clone(),
                remote_host.clone(),
                profile.clone(),
                region.clone(),
                priority.clone(),
                config.clone(),
            );
            async move {
                handle_connect(
                    instance_id,
                    local_port,
                    remote_port,
                    remote_host,
                    profile,
                    region,
                    priority,
                    precheck,
                    &config,
                )
                .await
            }
        },
        context,
        recovery_manager,
        message_system,
    )
    .await
}

pub async fn handle_list_with_recovery(
    config: &Config,
    recovery_manager: &ErrorRecoveryManager,
    message_system: &UserMessageSystem,
) -> Result<()> {
    let context = ErrorContext::new("list_sessions", "aws_manager");
    let config = config.clone();
    with_recovery(
        || {
            let config = config.clone();
            async move { handle_list(&config).await }
        },
        context,
        recovery_manager,
        message_system,
    )
    .await
}

pub async fn handle_terminate_with_recovery(
    session_id: String,
    config: &Config,
    recovery_manager: &ErrorRecoveryManager,
    message_system: &UserMessageSystem,
) -> Result<()> {
    let context =
        ErrorContext::new("terminate_session", "aws_manager").with_session_id(&session_id);
    let config = config.clone();
    with_recovery(
        || {
            let (session_id, config) = (session_id.clone(), config.clone());
            async move { handle_terminate(session_id, &config).await }
        },
        context,
        recovery_manager,
        message_system,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_connect(
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
                                reason: "Critical issues detected during preventive checks"
                                    .to_string(),
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
            let ec2_error = NimbusError::Session(crate::error::SessionError::CreationFailed {
                reason: e.to_string(),
            });

            return Err(ec2_error.into());
        }
    }

    Ok(())
}

pub async fn handle_list(config: &Config) -> Result<()> {
    info!("Listing active sessions");

    println!("📋 Active Sessions:");

    // Create AWS manager to list sessions
    let aws_manager = AwsManager::new(
        Some(config.aws.default_region.clone()),
        config.aws.default_profile.clone(),
    )
    .await?;

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

pub async fn handle_terminate(session_id: String, config: &Config) -> Result<()> {
    info!("Terminating session: {}", session_id);

    println!("🛑 Terminating session: {}", session_id);

    // Create AWS manager to terminate session
    let aws_manager = AwsManager::new(
        Some(config.aws.default_region.clone()),
        config.aws.default_profile.clone(),
    )
    .await?;

    match aws_manager.terminate_ssm_session(&session_id).await {
        Ok(_) => {
            println!("✅ Session terminated successfully");
        }
        Err(e) => {
            error!("Failed to terminate session: {}", e);
            println!("❌ Failed to terminate session: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

pub async fn handle_status(session_id: Option<String>, config: &Config) -> Result<()> {
    let aws_manager = AwsManager::new(
        Some(config.aws.default_region.clone()),
        config.aws.default_profile.clone(),
    )
    .await?;

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
