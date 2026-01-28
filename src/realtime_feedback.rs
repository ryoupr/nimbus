use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

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
    Paused,
    Interrupted,
    Completed,
    Failed,
}

/// Real-time feedback manager for diagnostic operations
pub struct RealtimeFeedbackManager {
    config: FeedbackConfig,
    status: Arc<Mutex<FeedbackStatus>>,
    current_progress: Arc<Mutex<Option<DiagnosticProgress>>>,
    completed_results: Arc<Mutex<Vec<DiagnosticResult>>>,
    critical_issues: Arc<Mutex<Vec<DiagnosticResult>>>,
    interrupt_sender: Option<mpsc::UnboundedSender<()>>,
    start_time: Instant,
}

impl RealtimeFeedbackManager {
    /// Create a new real-time feedback manager
    pub fn new(config: FeedbackConfig) -> Self {
        Self {
            config,
            status: Arc::new(Mutex::new(FeedbackStatus::Running)),
            current_progress: Arc::new(Mutex::new(None)),
            completed_results: Arc::new(Mutex::new(Vec::new())),
            critical_issues: Arc::new(Mutex::new(Vec::new())),
            interrupt_sender: None,
            start_time: Instant::now(),
        }
    }

    /// Start the real-time feedback display
    pub async fn start_feedback_display(&mut self) -> Result<(), anyhow::Error> {
        info!("Starting real-time diagnostic feedback display");

        // Enable raw mode for terminal input
        terminal::enable_raw_mode()?;

        // Create interrupt channel
        let (interrupt_tx, mut interrupt_rx) = mpsc::unbounded_channel();
        self.interrupt_sender = Some(interrupt_tx);

        // Clone shared state for the display task
        let status = Arc::clone(&self.status);
        let current_progress = Arc::clone(&self.current_progress);
        let completed_results = Arc::clone(&self.completed_results);
        let critical_issues = Arc::clone(&self.critical_issues);
        let config = self.config.clone();

        // Spawn display update task
        let display_task = tokio::spawn(async move {
            let mut stdout = io::stdout();
            let mut last_update = Instant::now();

            loop {
                // Check for interruption
                if let Ok(_) = interrupt_rx.try_recv() {
                    debug!("Received interrupt signal");
                    break;
                }

                // Update display at configured interval
                if last_update.elapsed().as_millis() >= config.refresh_interval_ms as u128 {
                    if let Err(e) = Self::update_display(
                        &mut stdout,
                        &status,
                        &current_progress,
                        &completed_results,
                        &critical_issues,
                        &config,
                    ).await {
                        error!("Failed to update display: {}", e);
                    }
                    last_update = Instant::now();
                }

                // Check for keyboard input
                if event::poll(Duration::from_millis(50))? {
                    if let Event::Key(key_event) = event::read()? {
                        if Self::handle_keyboard_input(key_event, &status, &critical_issues).await? {
                            break;
                        }
                    }
                }

                // Check if completed
                let current_status = {
                    let status_guard = status.lock().unwrap();
                    status_guard.clone()
                };

                if matches!(current_status, FeedbackStatus::Completed | FeedbackStatus::Failed) {
                    // Final display update
                    let _ = Self::update_display(
                        &mut stdout,
                        &status,
                        &current_progress,
                        &completed_results,
                        &critical_issues,
                        &config,
                    ).await;
                    break;
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            // Cleanup
            let _ = terminal::disable_raw_mode();
            let _ = execute!(stdout, ResetColor, cursor::Show);
            
            Ok::<(), anyhow::Error>(())
        });

        // Wait for display task to complete
        display_task.await??;

        info!("Real-time diagnostic feedback display completed");
        Ok(())
    }

    /// Update the display with current diagnostic information
    async fn update_display(
        stdout: &mut io::Stdout,
        status: &Arc<Mutex<FeedbackStatus>>,
        current_progress: &Arc<Mutex<Option<DiagnosticProgress>>>,
        completed_results: &Arc<Mutex<Vec<DiagnosticResult>>>,
        critical_issues: &Arc<Mutex<Vec<DiagnosticResult>>>,
        config: &FeedbackConfig,
    ) -> Result<(), anyhow::Error> {
        // Clear screen and move cursor to top
        execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        // Display header
        Self::display_header(stdout, config)?;

        // Display current progress
        if let Some(progress) = current_progress.lock().unwrap().as_ref() {
            Self::display_progress(stdout, progress, config)?;
        }

        // Display completed results
        let results = completed_results.lock().unwrap().clone();
        Self::display_results(stdout, &results, config)?;

        // Display critical issues if any
        let issues = critical_issues.lock().unwrap().clone();
        if !issues.is_empty() {
            Self::display_critical_issues(stdout, &issues, config)?;
        }

        // Display status and controls
        let current_status = status.lock().unwrap().clone();
        Self::display_status_and_controls(stdout, &current_status, config)?;

        stdout.flush()?;
        Ok(())
    }

    /// Display the header section
    fn display_header(stdout: &mut io::Stdout, config: &FeedbackConfig) -> Result<(), anyhow::Error> {
        if config.enable_colors {
            execute!(stdout, SetForegroundColor(Color::Cyan))?;
        }
        execute!(stdout, Print("╔══════════════════════════════════════════════════════════════════════════════╗\n"))?;
        execute!(stdout, Print("║                          SSM Connection Diagnostics                         ║\n"))?;
        execute!(stdout, Print("╚══════════════════════════════════════════════════════════════════════════════╝\n"))?;
        if config.enable_colors {
            execute!(stdout, ResetColor)?;
        }
        execute!(stdout, Print("\n"))?;
        Ok(())
    }

    /// Display current progress information
    fn display_progress(
        stdout: &mut io::Stdout,
        progress: &DiagnosticProgress,
        config: &FeedbackConfig,
    ) -> Result<(), anyhow::Error> {
        // Progress bar
        if config.show_progress_bar {
            let progress_percentage = progress.progress_percentage();
            let bar_width = 50;
            let filled_width = (progress_percentage / 100.0 * bar_width as f64) as usize;
            
            if config.enable_colors {
                execute!(stdout, SetForegroundColor(Color::Green))?;
            }
            execute!(stdout, Print("Progress: ["))?;
            
            for i in 0..bar_width {
                if i < filled_width {
                    execute!(stdout, Print("█"))?;
                } else {
                    execute!(stdout, Print("░"))?;
                }
            }
            
            execute!(stdout, Print(format!("] {:.1}%\n", progress_percentage)))?;
            if config.enable_colors {
                execute!(stdout, ResetColor)?;
            }
        }

        // Current item and timing information
        if config.show_detailed_status {
            execute!(stdout, Print(format!(
                "Current: {} ({}/{})\n",
                progress.current_item,
                progress.completed,
                progress.total
            )))?;

            if let Some(remaining) = progress.estimated_remaining {
                execute!(stdout, Print(format!(
                    "Estimated remaining: {:.1}s",
                    remaining.as_secs_f64()
                )))?;
            }
            execute!(stdout, Print("\n\n"))?;
        }

        Ok(())
    }

    /// Display completed diagnostic results
    fn display_results(
        stdout: &mut io::Stdout,
        results: &[DiagnosticResult],
        config: &FeedbackConfig,
    ) -> Result<(), anyhow::Error> {
        if results.is_empty() {
            return Ok(());
        }

        execute!(stdout, Print("Diagnostic Results:\n"))?;
        execute!(stdout, Print("─────────────────────────────────────────────────────────────────────────────\n"))?;

        for result in results {
            // Status icon and color
            let (icon, color) = match result.status {
                DiagnosticStatus::Success => ("✅", Color::Green),
                DiagnosticStatus::Warning => ("⚠️ ", Color::Yellow),
                DiagnosticStatus::Error => ("❌", Color::Red),
                DiagnosticStatus::Skipped => ("⏭️ ", Color::Blue),
            };

            if config.enable_colors {
                execute!(stdout, SetForegroundColor(color))?;
            }

            execute!(stdout, Print(format!(
                "{} {} ({:.2}s): {}\n",
                icon,
                result.item_name,
                result.duration.as_secs_f64(),
                result.message
            )))?;

            if config.enable_colors {
                execute!(stdout, ResetColor)?;
            }

            // Show severity for warnings and errors
            if matches!(result.status, DiagnosticStatus::Warning | DiagnosticStatus::Error) {
                let severity_color = match result.severity {
                    Severity::Critical => Color::Red,
                    Severity::High => Color::Magenta,
                    Severity::Medium => Color::Yellow,
                    Severity::Low => Color::Blue,
                    Severity::Info => Color::White,
                };

                if config.enable_colors {
                    execute!(stdout, SetForegroundColor(severity_color))?;
                }
                execute!(stdout, Print(format!("   Severity: {:?}", result.severity)))?;
                if result.auto_fixable {
                    execute!(stdout, Print(" (Auto-fixable)"))?;
                }
                execute!(stdout, Print("\n"))?;
                if config.enable_colors {
                    execute!(stdout, ResetColor)?;
                }
            }
        }

        execute!(stdout, Print("\n"))?;
        Ok(())
    }

    /// Display critical issues that require user attention
    fn display_critical_issues(
        stdout: &mut io::Stdout,
        issues: &[DiagnosticResult],
        config: &FeedbackConfig,
    ) -> Result<(), anyhow::Error> {
        if config.enable_colors {
            execute!(stdout, SetForegroundColor(Color::Red))?;
        }
        execute!(stdout, Print("⚠️  CRITICAL ISSUES DETECTED ⚠️\n"))?;
        execute!(stdout, Print("═════════════════════════════════════════════════════════════════════════════\n"))?;
        if config.enable_colors {
            execute!(stdout, ResetColor)?;
        }

        for (index, issue) in issues.iter().enumerate() {
            if config.enable_colors {
                execute!(stdout, SetForegroundColor(Color::Yellow))?;
            }
            execute!(stdout, Print(format!("{}. {}: {}\n", index + 1, issue.item_name, issue.message)))?;
            if config.enable_colors {
                execute!(stdout, ResetColor)?;
            }

            if issue.auto_fixable {
                if config.enable_colors {
                    execute!(stdout, SetForegroundColor(Color::Green))?;
                }
                execute!(stdout, Print("   → Auto-fix available\n"))?;
                if config.enable_colors {
                    execute!(stdout, ResetColor)?;
                }
            }
        }

        execute!(stdout, Print("\n"))?;
        Ok(())
    }

    /// Display current status and available controls
    fn display_status_and_controls(
        stdout: &mut io::Stdout,
        status: &FeedbackStatus,
        config: &FeedbackConfig,
    ) -> Result<(), anyhow::Error> {
        execute!(stdout, Print("─────────────────────────────────────────────────────────────────────────────\n"))?;

        // Status
        let (status_text, status_color) = match status {
            FeedbackStatus::Running => ("RUNNING", Color::Green),
            FeedbackStatus::Paused => ("PAUSED", Color::Yellow),
            FeedbackStatus::Interrupted => ("INTERRUPTED", Color::Red),
            FeedbackStatus::Completed => ("COMPLETED", Color::Blue),
            FeedbackStatus::Failed => ("FAILED", Color::Red),
        };

        execute!(stdout, Print("Status: "))?;
        if config.enable_colors {
            execute!(stdout, SetForegroundColor(status_color))?;
        }
        execute!(stdout, Print(status_text))?;
        if config.enable_colors {
            execute!(stdout, ResetColor)?;
        }
        execute!(stdout, Print("\n"))?;

        // Controls
        match status {
            FeedbackStatus::Running => {
                execute!(stdout, Print("Controls: [Ctrl+C] Interrupt | [P] Pause | [Q] Quit\n"))?;
            }
            FeedbackStatus::Paused => {
                execute!(stdout, Print("Controls: [R] Resume | [Ctrl+C] Interrupt | [Q] Quit\n"))?;
            }
            FeedbackStatus::Interrupted => {
                execute!(stdout, Print("Controls: [R] Resume | [Q] Quit\n"))?;
            }
            _ => {
                execute!(stdout, Print("Press any key to continue...\n"))?;
            }
        }

        Ok(())
    }

    /// Handle keyboard input for interactive control
    async fn handle_keyboard_input(
        key_event: KeyEvent,
        status: &Arc<Mutex<FeedbackStatus>>,
        critical_issues: &Arc<Mutex<Vec<DiagnosticResult>>>,
    ) -> Result<bool, anyhow::Error> {
        match key_event {
            // Ctrl+C - Interrupt
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                let mut status_guard = status.lock().unwrap();
                *status_guard = FeedbackStatus::Interrupted;
                info!("Diagnostic interrupted by user");
                return Ok(false); // Don't exit, allow resume
            }

            // P - Pause
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let mut status_guard = status.lock().unwrap();
                if *status_guard == FeedbackStatus::Running {
                    *status_guard = FeedbackStatus::Paused;
                    info!("Diagnostic paused by user");
                }
            }

            // R - Resume
            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let mut status_guard = status.lock().unwrap();
                if matches!(*status_guard, FeedbackStatus::Paused | FeedbackStatus::Interrupted) {
                    *status_guard = FeedbackStatus::Running;
                    info!("Diagnostic resumed by user");
                }
            }

            // Q - Quit
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                info!("Diagnostic quit by user");
                return Ok(true); // Exit
            }

            // Y - Confirm critical issue continuation
            KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let issues = critical_issues.lock().unwrap();
                if !issues.is_empty() {
                    info!("User confirmed to continue despite critical issues");
                    // Clear critical issues to continue
                    drop(issues);
                    critical_issues.lock().unwrap().clear();
                }
            }

            // N - Abort due to critical issues
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let issues = critical_issues.lock().unwrap();
                if !issues.is_empty() {
                    info!("User chose to abort due to critical issues");
                    let mut status_guard = status.lock().unwrap();
                    *status_guard = FeedbackStatus::Failed;
                    return Ok(true); // Exit
                }
            }

            _ => {}
        }

        Ok(false)
    }

    /// Update progress information
    pub fn update_progress(&self, progress: DiagnosticProgress) {
        let mut progress_guard = self.current_progress.lock().unwrap();
        *progress_guard = Some(progress);
    }

    /// Add a completed diagnostic result
    pub fn add_result(&self, result: DiagnosticResult) {
        // Check for critical issues
        if matches!(result.severity, Severity::Critical | Severity::High) && 
           matches!(result.status, DiagnosticStatus::Error) {
            self.critical_issues.lock().unwrap().push(result.clone());
            warn!("Critical issue detected: {} - {}", result.item_name, result.message);
        }

        self.completed_results.lock().unwrap().push(result);
    }

    /// Set the feedback status
    pub fn set_status(&self, new_status: FeedbackStatus) {
        let mut status_guard = self.status.lock().unwrap();
        *status_guard = new_status;
    }

    /// Get the current feedback status
    pub fn get_status(&self) -> FeedbackStatus {
        self.status.lock().unwrap().clone()
    }

    /// Check if there are unresolved critical issues
    pub fn has_critical_issues(&self) -> bool {
        !self.critical_issues.lock().unwrap().is_empty()
    }

    /// Get critical issues
    pub fn get_critical_issues(&self) -> Vec<DiagnosticResult> {
        self.critical_issues.lock().unwrap().clone()
    }

    /// Stop the feedback display
    pub fn stop(&self) {
        if let Some(sender) = &self.interrupt_sender {
            let _ = sender.send(());
        }
    }
}

/// Create a progress callback for the diagnostic manager
pub fn create_progress_callback(
    feedback_manager: Arc<RealtimeFeedbackManager>,
) -> Box<dyn Fn(DiagnosticProgress) + Send + Sync> {
    Box::new(move |progress| {
        feedback_manager.update_progress(progress);
    })
}

/// Create a result callback for adding diagnostic results
pub fn create_result_callback(
    feedback_manager: Arc<RealtimeFeedbackManager>,
) -> Box<dyn Fn(DiagnosticResult) + Send + Sync> {
    Box::new(move |result| {
        feedback_manager.add_result(result);
    })
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
            Duration::from_millis(100),
            Severity::Critical,
        );
        
        manager.add_result(critical_result);
        assert!(manager.has_critical_issues());
        
        let issues = manager.get_critical_issues();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].item_name, "test_item");
        assert_eq!(issues[0].severity, Severity::Critical);
    }

    #[test]
    fn test_status_management() {
        let config = FeedbackConfig::default();
        let manager = RealtimeFeedbackManager::new(config);
        
        assert_eq!(manager.get_status(), FeedbackStatus::Running);
        
        manager.set_status(FeedbackStatus::Paused);
        assert_eq!(manager.get_status(), FeedbackStatus::Paused);
        
        manager.set_status(FeedbackStatus::Completed);
        assert_eq!(manager.get_status(), FeedbackStatus::Completed);
    }
}