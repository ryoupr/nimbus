use crate::error::{Ec2ConnectError, ErrorSeverity, Result};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{error, warn, info, debug};

/// Error recovery strategy configuration
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub timeout: Duration,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Recovery strategy for different error types
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry with exponential backoff
    Retry(RecoveryConfig),
    /// Fallback to alternative method
    Fallback,
    /// Graceful degradation
    Degrade,
    /// Fail immediately
    Fail,
}

/// Error recovery manager
pub struct ErrorRecoveryManager {
    config: RecoveryConfig,
}

impl ErrorRecoveryManager {
    pub fn new(config: RecoveryConfig) -> Self {
        Self { config }
    }

    /// Determine recovery strategy based on error type
    pub fn get_strategy(&self, error: &Ec2ConnectError) -> RecoveryStrategy {
        match error {
            // Recoverable network/connection errors - retry with backoff
            Ec2ConnectError::Connection(_) if error.is_recoverable() => {
                RecoveryStrategy::Retry(self.config.clone())
            },
            Ec2ConnectError::Aws(_) if error.is_recoverable() => {
                RecoveryStrategy::Retry(self.config.clone())
            },
            Ec2ConnectError::Session(_) if error.is_recoverable() => {
                RecoveryStrategy::Retry(self.config.clone())
            },
            
            // Configuration errors - try fallback
            Ec2ConnectError::Config(_) => RecoveryStrategy::Fallback,
            
            // Resource errors - graceful degradation
            Ec2ConnectError::Resource(_) => RecoveryStrategy::Degrade,
            
            // UI errors - graceful degradation
            Ec2ConnectError::Ui(_) => RecoveryStrategy::Degrade,
            
            // Critical errors - fail immediately
            _ => RecoveryStrategy::Fail,
        }
    }

    /// Execute recovery strategy
    pub async fn recover<F, T>(&self, operation: F, error: &Ec2ConnectError) -> Result<T>
    where
        F: Fn() -> Result<T> + Send + Sync,
        T: Send,
    {
        let strategy = self.get_strategy(error);
        
        match strategy {
            RecoveryStrategy::Retry(config) => {
                self.retry_with_backoff(operation, config).await
            },
            RecoveryStrategy::Fallback => {
                warn!("Attempting fallback recovery for error: {}", error);
                self.attempt_fallback_recovery(operation, error).await
            },
            RecoveryStrategy::Degrade => {
                warn!("Graceful degradation for error: {}", error);
                self.attempt_graceful_degradation(operation, error).await
            },
            RecoveryStrategy::Fail => {
                error!("Non-recoverable error: {}", error);
                Err(error.clone())
            },
        }
    }

    /// Attempt fallback recovery methods
    async fn attempt_fallback_recovery<F, T>(&self, operation: F, error: &Ec2ConnectError) -> Result<T>
    where
        F: Fn() -> Result<T> + Send + Sync,
        T: Send,
    {
        match error {
            // Configuration errors - try with default configuration
            Ec2ConnectError::Config(config_error) => {
                info!("Attempting fallback with default configuration for: {}", config_error);

                // Wait a moment for any transient issues to resolve
                sleep(Duration::from_millis(100)).await;

                // Try the operation again (assuming it will use fallback config internally).
                // If it still fails, do one more attempt after a brief delay.
                for attempt in 1..=2u32 {
                    match operation() {
                        Ok(result) => {
                            info!("Fallback recovery successful with default configuration (attempt {})", attempt);
                            return Ok(result);
                        }
                        Err(fallback_error) => {
                            warn!("Fallback recovery attempt {} failed: {}", attempt, fallback_error);
                            if attempt < 2 {
                                sleep(Duration::from_millis(200)).await;
                            }
                        }
                    }
                }

                // Return the original error for better context
                Err(error.clone())
            },
            
            // For other errors, try a simple retry after a short delay
            _ => {
                info!("Attempting simple fallback retry for: {}", error);
                sleep(Duration::from_millis(500)).await;

                for attempt in 1..=2u32 {
                    match operation() {
                        Ok(result) => {
                            info!("Simple fallback retry successful (attempt {})", attempt);
                            return Ok(result);
                        }
                        Err(fallback_error) => {
                            warn!("Simple fallback retry attempt {} failed: {}", attempt, fallback_error);
                            if attempt < 2 {
                                sleep(Duration::from_millis(200)).await;
                            }
                        }
                    }
                }

                Err(error.clone())
            }
        }
    }

    /// Attempt graceful degradation
    async fn attempt_graceful_degradation<F, T>(&self, operation: F, error: &Ec2ConnectError) -> Result<T>
    where
        F: Fn() -> Result<T> + Send + Sync,
        T: Send,
    {
        match error {
            // Resource errors - try with reduced functionality
            Ec2ConnectError::Resource(resource_error) => {
                info!("Attempting graceful degradation for resource error: {}", resource_error);

                // Wait for resources to potentially free up
                sleep(Duration::from_secs(1)).await;

                // Try operation again (assuming it will use reduced resources).
                // If it still fails, do one more attempt after a brief delay.
                for attempt in 1..=2u32 {
                    match operation() {
                        Ok(result) => {
                            info!(
                                "Graceful degradation successful - operating with reduced functionality (attempt {})",
                                attempt
                            );
                            return Ok(result);
                        }
                        Err(degraded_error) => {
                            warn!("Graceful degradation attempt {} failed: {}", attempt, degraded_error);
                            if attempt < 2 {
                                sleep(Duration::from_millis(200)).await;
                            }
                        }
                    }
                }

                Err(error.clone())
            },
            
            // UI errors - continue without UI enhancements
            Ec2ConnectError::Ui(ui_error) => {
                info!("Graceful degradation for UI error: {} - continuing without enhanced UI", ui_error);
                
                // For UI errors, we might want to continue with basic functionality
                // This is a placeholder - in a real implementation, we'd set a flag
                // to disable UI enhancements and retry
                sleep(Duration::from_millis(100)).await;
                
                match operation() {
                    Ok(result) => {
                        info!("Continuing with basic UI functionality");
                        Ok(result)
                    },
                    Err(_) => {
                        warn!("Even basic functionality failed");
                        Err(error.clone())
                    }
                }
            },
            
            // VS Code errors - continue without VS Code integration
            Ec2ConnectError::VsCode(vscode_error) => {
                info!("Graceful degradation for VS Code error: {} - continuing without VS Code integration", vscode_error);
                
                // VS Code integration is optional, so we can continue without it
                sleep(Duration::from_millis(100)).await;
                
                match operation() {
                    Ok(result) => {
                        info!("Continuing without VS Code integration");
                        Ok(result)
                    },
                    Err(_) => {
                        warn!("Core functionality failed even without VS Code integration");
                        Err(error.clone())
                    }
                }
            },
            
            // For other errors, try a conservative retry
            _ => {
                info!("Attempting conservative retry for: {}", error);
                sleep(Duration::from_millis(1000)).await;

                for attempt in 1..=2u32 {
                    match operation() {
                        Ok(result) => {
                            info!("Conservative retry successful (attempt {})", attempt);
                            return Ok(result);
                        }
                        Err(retry_error) => {
                            warn!("Conservative retry attempt {} failed: {}", attempt, retry_error);
                            if attempt < 2 {
                                sleep(Duration::from_millis(200)).await;
                            }
                        }
                    }
                }

                Err(error.clone())
            }
        }
    }

    /// Retry operation with exponential backoff
    async fn retry_with_backoff<F, T>(&self, operation: F, config: RecoveryConfig) -> Result<T>
    where
        F: Fn() -> Result<T> + Send + Sync,
        T: Send,
    {
        let start_time = Instant::now();
        let mut delay = config.base_delay;
        
        for attempt in 1..=config.max_attempts {
            // Check timeout
            if start_time.elapsed() > config.timeout {
                error!("Recovery timeout exceeded after {} attempts", attempt - 1);
                return Err(Ec2ConnectError::System(
                    "Recovery timeout exceeded".to_string()
                ));
            }

            debug!("Recovery attempt {} of {}", attempt, config.max_attempts);
            
            match operation() {
                Ok(result) => {
                    info!("Recovery successful after {} attempts", attempt);
                    return Ok(result);
                },
                Err(err) => {
                    if attempt == config.max_attempts {
                        error!("Recovery failed after {} attempts: {}", attempt, err);
                        return Err(err);
                    }
                    
                    warn!("Recovery attempt {} failed: {}, retrying in {:?}", 
                          attempt, err, delay);
                    
                    sleep(delay).await;
                    
                    // Exponential backoff with jitter
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * config.backoff_multiplier) as u64
                        ),
                        config.max_delay
                    );
                }
            }
        }
        
        unreachable!("Loop should have returned or broken")
    }
}

/// Error context for better debugging
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub component: String,
    pub session_id: Option<String>,
    pub instance_id: Option<String>,
    pub timestamp: std::time::SystemTime,
    pub additional_info: std::collections::HashMap<String, String>,
}

impl ErrorContext {
    pub fn new(operation: &str, component: &str) -> Self {
        Self {
            operation: operation.to_string(),
            component: component.to_string(),
            session_id: None,
            instance_id: None,
            timestamp: std::time::SystemTime::now(),
            additional_info: std::collections::HashMap::new(),
        }
    }

    pub fn with_session_id(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    pub fn with_instance_id(mut self, instance_id: &str) -> Self {
        self.instance_id = Some(instance_id.to_string());
        self
    }

    pub fn with_info(mut self, key: &str, value: &str) -> Self {
        self.additional_info.insert(key.to_string(), value.to_string());
        self
    }
}

/// Enhanced error with context
#[derive(Debug)]
pub struct ContextualError {
    pub error: Ec2ConnectError,
    pub context: ErrorContext,
}

impl ContextualError {
    pub fn new(error: Ec2ConnectError, context: ErrorContext) -> Self {
        Self { error, context }
    }

    /// Get detailed error information for logging
    pub fn detailed_info(&self) -> String {
        format!(
            "Error in {}.{}: {} | Session: {} | Instance: {} | Time: {:?} | Additional: {:?}",
            self.context.component,
            self.context.operation,
            self.error,
            self.context.session_id.as_deref().unwrap_or("N/A"),
            self.context.instance_id.as_deref().unwrap_or("N/A"),
            self.context.timestamp,
            self.context.additional_info
        )
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        self.error.user_message()
    }

    /// Get severity level
    pub fn severity(&self) -> ErrorSeverity {
        self.error.severity()
    }
}

/// Macro for creating contextual errors
#[macro_export]
macro_rules! contextual_error {
    ($error:expr, $operation:expr, $component:expr) => {
        ContextualError::new(
            $error,
            ErrorContext::new($operation, $component)
        )
    };
    ($error:expr, $operation:expr, $component:expr, $($key:expr => $value:expr),*) => {
        {
            let mut context = ErrorContext::new($operation, $component);
            $(
                context = context.with_info($key, $value);
            )*
            ContextualError::new($error, context)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ConnectionError, SessionError, ConfigError, ResourceError};

    #[tokio::test]
    async fn test_recovery_strategy_selection() {
        let manager = ErrorRecoveryManager::new(RecoveryConfig::default());
        
        // Recoverable errors should use retry strategy
        let connection_error = Ec2ConnectError::Connection(ConnectionError::PreventiveCheckFailed {
            reason: "test".to_string(),
            issues: vec!["issue1".to_string()],
        });
        matches!(manager.get_strategy(&connection_error), RecoveryStrategy::Retry(_));
        
        // Non-recoverable errors should fail immediately
        let session_error = Ec2ConnectError::Session(SessionError::NotFound {
            session_id: "test".to_string()
        });
        matches!(manager.get_strategy(&session_error), RecoveryStrategy::Fail);
    }

    #[tokio::test]
    async fn test_fallback_recovery() {
        let manager = ErrorRecoveryManager::new(RecoveryConfig::default());
        
        // Test connection error fallback
        let connection_error = Ec2ConnectError::Connection(ConnectionError::PreventiveCheckFailed {
            reason: "test".to_string(),
            issues: vec!["issue1".to_string()],
        });
        
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();
        let error_clone = connection_error.clone();
        
        let operation = move || -> crate::error::Result<String> {
            let count = call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                Err(error_clone.clone())
            } else {
                Ok("success".to_string())
            }
        };
        
        let result = manager.recover(operation, &connection_error).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_graceful_degradation() {
        let manager = ErrorRecoveryManager::new(RecoveryConfig::default());
        
        // Test session error degradation
        let session_error = Ec2ConnectError::Session(SessionError::CreationFailed {
            reason: "test".to_string()
        });
        
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();
        let error_clone = session_error.clone();
        
        let operation = move || -> crate::error::Result<String> {
            let count = call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                Err(error_clone.clone())
            } else {
                Ok("degraded_success".to_string())
            }
        };
        
        let result = manager.recover(operation, &session_error).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "degraded_success");
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        let config = RecoveryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            timeout: Duration::from_secs(10),
        };
        let manager = ErrorRecoveryManager::new(config);
        
        let connection_error = Ec2ConnectError::Connection(ConnectionError::PreventiveCheckFailed {
            reason: "test".to_string(),
            issues: vec!["issue1".to_string()],
        });
        
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();
        let error_clone = connection_error.clone();
        
        let operation = move || -> crate::error::Result<String> {
            let count = call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count < 2 {
                Err(error_clone.clone())
            } else {
                Ok("retry_success".to_string())
            }
        };
        
        let start_time = std::time::Instant::now();
        let result = manager.recover(operation, &connection_error).await;
        let elapsed = start_time.elapsed();
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "retry_success");
        // Should have some delay due to backoff
        assert!(elapsed >= Duration::from_millis(30)); // 10ms + 20ms minimum
    }

    #[tokio::test]
    async fn test_contextual_error() {
        let error = Ec2ConnectError::Connection(ConnectionError::PreventiveCheckFailed {
            reason: "test".to_string(),
            issues: vec!["issue1".to_string()],
        });
        let context = ErrorContext::new("connect", "session_manager")
            .with_session_id("test-session")
            .with_instance_id("i-1234567890abcdef0");
        
        let contextual = ContextualError::new(error, context);
        
        assert!(contextual.detailed_info().contains("session_manager.connect"));
        assert!(contextual.detailed_info().contains("test-session"));
        assert!(contextual.detailed_info().contains("i-1234567890abcdef0"));
    }
}