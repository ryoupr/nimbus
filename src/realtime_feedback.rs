use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::warn;

use crate::diagnostic::{DiagnosticProgress, DiagnosticResult, DiagnosticStatus, Severity};

// CriticalIssue struct removed - using DiagnosticResult instead for type consistency

/// Configuration for real-time feedback display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackConfig {
    pub show_progress_bar: bool,
    pub show_detailed_status: bool,
    pub enable_colors: bool,
    pub auto_confirm_critical: bool,
    pub refresh_interval_ms: u64,
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            show_progress_bar: true,
            show_detailed_status: true,
            enable_colors: true,
            auto_confirm_critical: false,
            refresh_interval_ms: 100,
        }
    }
}

/// Status of the diagnostic feedback system
#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackStatus {
    Running,
}

/// Real-time feedback manager for diagnostic operations
pub struct RealtimeFeedbackManager {
    status: Arc<Mutex<FeedbackStatus>>,
    current_progress: Arc<Mutex<Option<DiagnosticProgress>>>,
    completed_results: Arc<Mutex<Vec<DiagnosticResult>>>,
    critical_issues: Arc<Mutex<Vec<DiagnosticResult>>>,
    interrupt_sender: Option<mpsc::UnboundedSender<()>>,
}

impl RealtimeFeedbackManager {
    /// Create a new real-time feedback manager
    pub fn new(_config: FeedbackConfig) -> Self {
        Self {
            status: Arc::new(Mutex::new(FeedbackStatus::Running)),
            current_progress: Arc::new(Mutex::new(None)),
            completed_results: Arc::new(Mutex::new(Vec::new())),
            critical_issues: Arc::new(Mutex::new(Vec::new())),
            interrupt_sender: None,
        }
    }
    /// Update progress information
    pub fn update_progress(&self, progress: DiagnosticProgress) {
        let mut progress_guard = self.current_progress.blocking_lock();
        *progress_guard = Some(progress);
    }

    /// Add a completed diagnostic result
    pub fn add_result(&self, result: DiagnosticResult) {
        // Check for critical issues
        if matches!(result.severity, Severity::Critical | Severity::High)
            && matches!(result.status, DiagnosticStatus::Error)
        {
            self.critical_issues.blocking_lock().push(result.clone());
            warn!(
                "Critical issue detected: {} - {}",
                result.item_name, result.message
            );
        }

        self.completed_results.blocking_lock().push(result);
    }
    /// Get the current feedback status
    pub fn get_status(&self) -> FeedbackStatus {
        self.status.blocking_lock().clone()
    }

    /// Check if there are unresolved critical issues
    pub fn has_critical_issues(&self) -> bool {
        !self.critical_issues.blocking_lock().is_empty()
    }

    /// Get critical issues
    pub fn get_critical_issues(&self) -> Vec<DiagnosticResult> {
        self.critical_issues.blocking_lock().clone()
    }

    /// Stop the feedback display
    pub fn stop(&self) {
        if let Some(sender) = &self.interrupt_sender {
            let _ = sender.send(());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_feedback_config_default() {
        let config = FeedbackConfig::default();
        assert!(config.show_progress_bar);
        assert!(config.show_detailed_status);
        assert!(config.enable_colors);
        assert!(!config.auto_confirm_critical);
        assert_eq!(config.refresh_interval_ms, 100);
    }

    #[test]
    fn test_realtime_feedback_manager_creation() {
        let config = FeedbackConfig::default();
        let manager = RealtimeFeedbackManager::new(config);

        assert_eq!(manager.get_status(), FeedbackStatus::Running);
        assert!(!manager.has_critical_issues());
    }

    #[test]
    fn test_critical_issue_detection() {
        let config = FeedbackConfig::default();
        let manager = RealtimeFeedbackManager::new(config);

        let critical_result = DiagnosticResult::error(
            "test_item".to_string(),
            "Critical error occurred".to_string(),
            std::time::Duration::from_millis(100),
            Severity::Critical,
        );

        manager.add_result(critical_result);
        assert!(manager.has_critical_issues());

        let issues = manager.get_critical_issues();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].item_name, "test_item");
        assert_eq!(issues[0].severity, Severity::Critical);
    }
}
