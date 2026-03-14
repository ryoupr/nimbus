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

use crate::persistence::{PersistenceManager, SqlitePersistenceManager};

#[cfg(feature = "persistence")]
pub async fn handle_database(action: DatabaseCommands, _config: &Config) -> Result<()> {
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
