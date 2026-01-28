#![allow(dead_code)]

use crate::error::{NimbusError, ErrorSeverity};
use crate::error_recovery::ContextualError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use tracing;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Enable console logging
    pub console_enabled: bool,
    /// Enable file logging
    pub file_enabled: bool,
    /// Log file directory
    pub log_dir: PathBuf,
    /// Log file name prefix
    pub file_prefix: String,
    /// Log rotation (daily, hourly, never)
    pub rotation: String,
    /// Maximum log files to keep
    pub max_files: u32,
    /// Enable structured JSON logging
    pub json_format: bool,
    /// Enable performance tracing
    pub performance_tracing: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            console_enabled: true,
            file_enabled: true,
            log_dir: PathBuf::from("logs"),
            file_prefix: "nimbus".to_string(),
            rotation: "daily".to_string(),
            max_files: 7,
            json_format: false,
            performance_tracing: false,
        }
    }
}

/// Initialize logging system
pub fn init_logging(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.level))?;

    let mut layers = Vec::new();

    // Console layer
    if config.console_enabled {
        let console_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(if config.performance_tracing {
                FmtSpan::ENTER | FmtSpan::EXIT
            } else {
                FmtSpan::NONE
            })
            .with_filter(env_filter.clone());
        
        layers.push(console_layer.boxed());
    }

    // File layer
    if config.file_enabled {
        std::fs::create_dir_all(&config.log_dir)?;
        
        let file_appender = match config.rotation.as_str() {
            "daily" => rolling::daily(&config.log_dir, &config.file_prefix),
            "hourly" => rolling::hourly(&config.log_dir, &config.file_prefix),
            _ => rolling::never(&config.log_dir, format!("{}.log", config.file_prefix)),
        };
        
        let (non_blocking, _guard) = non_blocking(file_appender);
        
        let file_layer = if config.json_format {
            fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_span_events(if config.performance_tracing {
                    FmtSpan::ENTER | FmtSpan::EXIT
                } else {
                    FmtSpan::NONE
                })
                .with_filter(env_filter.clone())
                .boxed()
        } else {
            fmt::layer()
                .with_writer(non_blocking)
                .with_span_events(if config.performance_tracing {
                    FmtSpan::ENTER | FmtSpan::EXIT
                } else {
                    FmtSpan::NONE
                })
                .with_filter(env_filter.clone())
                .boxed()
        };
        
        layers.push(file_layer);
    }

    tracing_subscriber::registry()
        .with(layers)
        .init();

    Ok(())
}

/// Structured log entry for errors
#[derive(Debug, Serialize)]
pub struct ErrorLogEntry {
    pub timestamp: SystemTime,
    pub level: String,
    pub error_type: String,
    pub error_message: String,
    pub user_message: String,
    pub severity: String,
    pub component: String,
    pub operation: String,
    pub session_id: Option<String>,
    pub instance_id: Option<String>,
    pub recoverable: bool,
    pub context: HashMap<String, String>,
    pub stack_trace: Option<String>,
}

impl ErrorLogEntry {
    pub fn from_contextual_error(error: &ContextualError) -> Self {
        Self {
            timestamp: error.context.timestamp,
            level: match error.severity() {
                ErrorSeverity::Low => "WARN".to_string(),
                ErrorSeverity::Medium => "ERROR".to_string(),
                ErrorSeverity::High => "ERROR".to_string(),
                
            },
            error_type: format!("{:?}", std::mem::discriminant(&error.error)),
            error_message: error.error.to_string(),
            user_message: error.user_message(),
            severity: error.severity().as_str().to_string(),
            component: error.context.component.clone(),
            operation: error.context.operation.clone(),
            session_id: error.context.session_id.clone(),
            instance_id: error.context.instance_id.clone(),
            recoverable: error.error.is_recoverable(),
            context: error.context.additional_info.clone(),
            stack_trace: None, // Could be enhanced with backtrace
        }
    }

    pub fn from_error(error: &NimbusError, component: &str, operation: &str) -> Self {
        Self {
            timestamp: SystemTime::now(),
            level: match error.severity() {
                ErrorSeverity::Low => "WARN".to_string(),
                ErrorSeverity::Medium => "ERROR".to_string(),
                ErrorSeverity::High => "ERROR".to_string(),
                
            },
            error_type: format!("{:?}", std::mem::discriminant(error)),
            error_message: error.to_string(),
            user_message: error.user_message(),
            severity: error.severity().as_str().to_string(),
            component: component.to_string(),
            operation: operation.to_string(),
            session_id: None,
            instance_id: None,
            recoverable: error.is_recoverable(),
            context: HashMap::new(),
            stack_trace: None,
        }
    }
}

/// Logger utility for structured logging
pub struct StructuredLogger;

impl StructuredLogger {
    /// Log error with full context
    pub fn log_error(error: &ContextualError) {
        let entry = ErrorLogEntry::from_contextual_error(error);
        
        match error.severity() {
            ErrorSeverity::Low => {
                tracing::warn!(
                    error_type = %entry.error_type,
                    component = %entry.component,
                    operation = %entry.operation,
                    session_id = ?entry.session_id,
                    instance_id = ?entry.instance_id,
                    recoverable = %entry.recoverable,
                    "{}",
                    entry.error_message
                );
            },
            ErrorSeverity::Medium => {
                tracing::error!(
                    error_type = %entry.error_type,
                    component = %entry.component,
                    operation = %entry.operation,
                    session_id = ?entry.session_id,
                    instance_id = ?entry.instance_id,
                    recoverable = %entry.recoverable,
                    "{}",
                    entry.error_message
                );
            },
            ErrorSeverity::High => {
                tracing::error!(
                    error_type = %entry.error_type,
                    component = %entry.component,
                    operation = %entry.operation,
                    session_id = ?entry.session_id,
                    instance_id = ?entry.instance_id,
                    recoverable = %entry.recoverable,
                    severity = %entry.severity,
                    user_message = %entry.user_message,
                    "CRITICAL ERROR: {}",
                    entry.error_message
                );
            },
        }
    }

    /// Log session activity
    pub fn log_session_activity(
        session_id: &str,
        activity: &str,
        details: Option<&HashMap<String, String>>
    ) {
        tracing::info!(
            session_id = %session_id,
            activity = %activity,
            details = ?details,
            "Session activity: {}",
            activity
        );
    }

    /// Log system resource usage
    pub fn log_resource_usage(
        component: &str,
        memory_mb: f64,
        cpu_percent: f64,
        additional_metrics: Option<&HashMap<String, f64>>
    ) {
        tracing::debug!(
            component = %component,
            memory_mb = %memory_mb,
            cpu_percent = %cpu_percent,
            additional_metrics = ?additional_metrics,
            "Resource usage: Memory: {:.2}MB, CPU: {:.2}%",
            memory_mb,
            cpu_percent
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ConnectionError;
    use crate::error_recovery::ErrorContext;

    #[test]
    fn test_error_log_entry_creation() {
        let error = NimbusError::Connection(ConnectionError::PreventiveCheckFailed {
            reason: "test".to_string(),
            issues: vec!["issue1".to_string()],
        });
        let context = ErrorContext::new("connect", "session_manager")
            .with_session_id("test-session");
        let contextual_error = ContextualError::new(error, context);
        
        let log_entry = ErrorLogEntry::from_contextual_error(&contextual_error);
        
        assert_eq!(log_entry.component, "session_manager");
        assert_eq!(log_entry.operation, "connect");
        assert_eq!(log_entry.session_id, Some("test-session".to_string()));
        assert!(log_entry.recoverable);
    }
}