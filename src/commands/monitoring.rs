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

#[cfg(feature = "performance-monitoring")]
use crate::monitor::DefaultSessionMonitor;
#[cfg(feature = "multi-session")]
use crate::multi_session::{MultiSessionManager, ResourceThresholds};
#[cfg(feature = "multi-session")]
use crate::multi_session_ui::MultiSessionUi;
#[cfg(feature = "persistence")]
use crate::persistence::{PersistenceManager, SqlitePersistenceManager};

pub async fn handle_metrics(_config: &Config) -> Result<()> {
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

pub async fn handle_resources(_config: &Config) -> Result<()> {
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

pub async fn handle_health(
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
