use crate::diagnostic::{DiagnosticProgress, DiagnosticResult, DiagnosticStatus, Severity};
use crate::user_messages::UserMessageSystem;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Gauge, LineGauge, List, ListItem, Paragraph, Row, Table,
    },
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};

/// Real-time diagnostic feedback system
pub struct DiagnosticFeedbackSystem {
    terminal: Option<Terminal<CrosstermBackend<io::Stdout>>>,
    state: Arc<Mutex<FeedbackState>>,
    event_sender: mpsc::UnboundedSender<FeedbackEvent>,
    event_receiver: mpsc::UnboundedReceiver<FeedbackEvent>,
    progress_receiver: watch::Receiver<Option<DiagnosticProgress>>,
    progress_sender: watch::Sender<Option<DiagnosticProgress>>,
    result_receiver: watch::Receiver<Vec<DiagnosticResult>>,
    result_sender: watch::Sender<Vec<DiagnosticResult>>,
    user_message_system: UserMessageSystem,
    start_time: Instant,
    is_running: bool,
}

/// Feedback system state
#[derive(Debug, Clone)]
pub struct FeedbackState {
    pub current_progress: Option<DiagnosticProgress>,
    pub diagnostic_results: Vec<DiagnosticResult>,
    pub current_item: String,
    pub completed_items: Vec<String>,
    pub failed_items: Vec<String>,
    pub warning_items: Vec<String>,
    pub is_paused: bool,
    pub show_details: bool,
    pub selected_item: usize,
    pub scroll_offset: usize,
    pub critical_issues: Vec<DiagnosticResult>,
    pub confirmation_pending: Option<ConfirmationRequest>,
    pub last_update: Instant,
}

/// Events for the feedback system
#[derive(Debug, Clone)]
pub enum FeedbackEvent {
    UpdateProgress(DiagnosticProgress),
    AddResult(DiagnosticResult),
    CriticalIssueDetected(DiagnosticResult),
    RequestConfirmation(ConfirmationRequest),
    UserConfirmation(bool),
    Pause,
    Resume,
    ToggleDetails,
    ScrollUp,
    ScrollDown,
    SelectNext,
    SelectPrev,
    Quit,
    Refresh,
}

/// Confirmation request for critical issues
#[derive(Debug, Clone)]
pub struct ConfirmationRequest {
    pub issue: DiagnosticResult,
    pub message: String,
    pub options: Vec<String>,
    pub default_option: usize,
    pub timestamp: Instant,
}

/// Display configuration for the feedback system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackDisplayConfig {
    pub show_progress_bar: bool,
    pub show_item_details: bool,
    pub color_coding_enabled: bool,
    pub auto_scroll: bool,
    pub refresh_interval_ms: u64,
    pub confirmation_timeout_seconds: u64,
}

impl Default for FeedbackDisplayConfig {
    fn default() -> Self {
        Self {
            show_progress_bar: true,
            show_item_details: true,
            color_coding_enabled: true,
            auto_scroll: true,
            refresh_interval_ms: 100,
            confirmation_timeout_seconds: 30,
        }
    }
}

impl Default for FeedbackState {
    fn default() -> Self {
        Self {
            current_progress: None,
            diagnostic_results: Vec::new(),
            current_item: String::new(),
            completed_items: Vec::new(),
            failed_items: Vec::new(),
            warning_items: Vec::new(),
            is_paused: false,
            show_details: false,
            selected_item: 0,
            scroll_offset: 0,
            critical_issues: Vec::new(),
            confirmation_pending: None,
            last_update: Instant::now(),
        }
    }
}

impl DiagnosticFeedbackSystem {
    /// Create a new diagnostic feedback system
    pub fn new() -> anyhow::Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let (progress_sender, progress_receiver) = watch::channel(None);
        let (result_sender, result_receiver) = watch::channel(Vec::new());

        Ok(Self {
            terminal: None,
            state: Arc::new(Mutex::new(FeedbackState::default())),
            event_sender,
            event_receiver,
            progress_receiver,
            progress_sender,
            result_receiver,
            result_sender,
            user_message_system: UserMessageSystem::new(),
            start_time: Instant::now(),
            is_running: false,
        })
    }

    /// Initialize the terminal UI
    pub fn initialize_terminal(&mut self) -> anyhow::Result<()> {
        info!("Initializing diagnostic feedback terminal UI");

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        self.terminal = Some(terminal);
        Ok(())
    }

    /// Start the real-time feedback system
    pub async fn start(&mut self, config: FeedbackDisplayConfig) -> anyhow::Result<()> {
        info!("Starting diagnostic feedback system");

        if self.terminal.is_none() {
            self.initialize_terminal()?;
        }

        self.is_running = true;

        // Spawn input handler
        let event_sender = self.event_sender.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(event) = event::read() {
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                let _ = event_sender.send(FeedbackEvent::Quit);
                                break;
                            }
                            KeyCode::Char(' ') => {
                                let _ = event_sender.send(FeedbackEvent::Pause);
                            }
                            KeyCode::Char('r') | KeyCode::F(5) => {
                                let _ = event_sender.send(FeedbackEvent::Resume);
                            }
                            KeyCode::Char('d') => {
                                let _ = event_sender.send(FeedbackEvent::ToggleDetails);
                            }
                            KeyCode::Up => {
                                let _ = event_sender.send(FeedbackEvent::ScrollUp);
                            }
                            KeyCode::Down => {
                                let _ = event_sender.send(FeedbackEvent::ScrollDown);
                            }
                            KeyCode::Tab => {
                                let _ = event_sender.send(FeedbackEvent::SelectNext);
                            }
                            KeyCode::BackTab => {
                                let _ = event_sender.send(FeedbackEvent::SelectPrev);
                            }
                            KeyCode::Enter => {
                                let _ = event_sender.send(FeedbackEvent::UserConfirmation(true));
                            }
                            KeyCode::Char('n') => {
                                let _ = event_sender.send(FeedbackEvent::UserConfirmation(false));
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        });

        // Main feedback loop
        let mut last_draw = Instant::now();
        let refresh_interval = Duration::from_millis(config.refresh_interval_ms);

        while self.is_running {
            // Handle events
            while let Ok(event) = self.event_receiver.try_recv() {
                self.handle_event(event, &config).await?;
            }

            // Update display at regular intervals
            if last_draw.elapsed() >= refresh_interval {
                self.draw_feedback_ui(&config)?;
                last_draw = Instant::now();
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    /// Stop the feedback system
    pub fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping diagnostic feedback system");

        self.is_running = false;

        if let Some(terminal) = &mut self.terminal {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
        }

        self.terminal = None;
        Ok(())
    }

    /// Update progress information
    pub fn update_progress(&self, progress: DiagnosticProgress) -> anyhow::Result<()> {
        debug!(
            "Updating diagnostic progress: {}/{}",
            progress.completed, progress.total
        );

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.current_progress = Some(progress.clone());
            state.current_item = progress.current_item.clone();
            state.last_update = Instant::now();
        }

        // Send progress update
        let _ = self.progress_sender.send(Some(progress.clone()));
        let _ = self
            .event_sender
            .send(FeedbackEvent::UpdateProgress(progress));

        Ok(())
    }

    /// Add diagnostic result
    pub fn add_result(&self, result: DiagnosticResult) -> anyhow::Result<()> {
        debug!(
            "Adding diagnostic result: {} - {:?}",
            result.item_name, result.status
        );

        // Update state
        {
            let mut state = self.state.lock().unwrap();

            // Handle critical issues immediately to avoid race conditions with async event processing
            if result.severity == Severity::Critical {
                warn!("Critical issue detected: {}", result.message);
                state.critical_issues.push(result.clone());
                state.is_paused = true;
            }

            // Update item lists based on status
            match result.status {
                DiagnosticStatus::Success => {
                    state.completed_items.push(result.item_name.clone());
                }
                DiagnosticStatus::Error => {
                    state.failed_items.push(result.item_name.clone());
                }
                DiagnosticStatus::Warning => {
                    state.warning_items.push(result.item_name.clone());
                }
                DiagnosticStatus::Skipped => {
                    // Don't add to any specific list
                }
            }

            // Add to results
            state.diagnostic_results.push(result.clone());
            state.last_update = Instant::now();
        }

        // Request confirmation for critical issues
        if result.severity == Severity::Critical {
            self.request_confirmation(
                result.clone(),
                "重大な問題が検出されました。診断を続行しますか？".to_string(),
                vec!["続行".to_string(), "中止".to_string()],
            )?;
        }

        // Send result update
        let current_results = {
            let state = self.state.lock().unwrap();
            state.diagnostic_results.clone()
        };
        let _ = self.result_sender.send(current_results);
        let _ = self.event_sender.send(FeedbackEvent::AddResult(result));

        Ok(())
    }

    /// Request user confirmation for critical issues
    pub fn request_confirmation(
        &self,
        issue: DiagnosticResult,
        message: String,
        options: Vec<String>,
    ) -> anyhow::Result<()> {
        info!(
            "Requesting user confirmation for critical issue: {}",
            issue.item_name
        );

        let confirmation = ConfirmationRequest {
            issue,
            message,
            options,
            default_option: 0,
            timestamp: Instant::now(),
        };

        {
            let mut state = self.state.lock().unwrap();
            state.confirmation_pending = Some(confirmation.clone());
        }

        let _ = self
            .event_sender
            .send(FeedbackEvent::RequestConfirmation(confirmation));

        Ok(())
    }

    /// Pause diagnostic execution
    pub fn pause(&self) -> anyhow::Result<()> {
        info!("Pausing diagnostic execution");

        {
            let mut state = self.state.lock().unwrap();
            state.is_paused = true;
        }

        let _ = self.event_sender.send(FeedbackEvent::Pause);
        Ok(())
    }

    /// Resume diagnostic execution
    pub fn resume(&self) -> anyhow::Result<()> {
        info!("Resuming diagnostic execution");

        {
            let mut state = self.state.lock().unwrap();
            state.is_paused = false;
        }

        let _ = self.event_sender.send(FeedbackEvent::Resume);
        Ok(())
    }

    /// Get current state snapshot
    pub fn get_state_snapshot(&self) -> FeedbackState {
        let state = self.state.lock().unwrap();
        state.clone()
    }

    /// Handle feedback events
    async fn handle_event(
        &mut self,
        event: FeedbackEvent,
        config: &FeedbackDisplayConfig,
    ) -> anyhow::Result<()> {
        match event {
            FeedbackEvent::UpdateProgress(progress) => {
                // Progress already updated in update_progress method
            }
            FeedbackEvent::AddResult(result) => {
                // Result already added in add_result method
            }
            FeedbackEvent::CriticalIssueDetected(result) => {
                // Add to critical issues list
                let mut state = self.state.lock().unwrap();
                state.critical_issues.push(result.clone());

                // Auto-pause on critical issues
                state.is_paused = true;

                // Request confirmation
                drop(state);
                self.request_confirmation(
                    result,
                    "重大な問題が検出されました。診断を続行しますか？".to_string(),
                    vec!["続行".to_string(), "中止".to_string()],
                )?;
            }
            FeedbackEvent::RequestConfirmation(confirmation) => {
                // Confirmation already set in request_confirmation method
            }
            FeedbackEvent::UserConfirmation(confirmed) => {
                let mut state = self.state.lock().unwrap();
                if let Some(pending) = &state.confirmation_pending {
                    info!("User confirmation received: {}", confirmed);

                    if confirmed {
                        state.is_paused = false;
                    } else {
                        self.is_running = false;
                    }

                    state.confirmation_pending = None;
                }
            }
            FeedbackEvent::Pause => {
                let mut state = self.state.lock().unwrap();
                state.is_paused = !state.is_paused;
                info!("Diagnostic execution paused: {}", state.is_paused);
            }
            FeedbackEvent::Resume => {
                let mut state = self.state.lock().unwrap();
                state.is_paused = false;
                info!("Diagnostic execution resumed");
            }
            FeedbackEvent::ToggleDetails => {
                let mut state = self.state.lock().unwrap();
                state.show_details = !state.show_details;
                debug!("Details view toggled: {}", state.show_details);
            }
            FeedbackEvent::ScrollUp => {
                let mut state = self.state.lock().unwrap();
                if state.scroll_offset > 0 {
                    state.scroll_offset -= 1;
                }
            }
            FeedbackEvent::ScrollDown => {
                let mut state = self.state.lock().unwrap();
                state.scroll_offset += 1;
            }
            FeedbackEvent::SelectNext => {
                let mut state = self.state.lock().unwrap();
                if state.selected_item < state.diagnostic_results.len().saturating_sub(1) {
                    state.selected_item += 1;
                }
            }
            FeedbackEvent::SelectPrev => {
                let mut state = self.state.lock().unwrap();
                if state.selected_item > 0 {
                    state.selected_item -= 1;
                }
            }
            FeedbackEvent::Quit => {
                info!("Quit requested by user");
                self.is_running = false;
            }
            FeedbackEvent::Refresh => {
                // Force redraw
            }
        }

        Ok(())
    }

    /// Draw the feedback UI
    fn draw_feedback_ui(&mut self, config: &FeedbackDisplayConfig) -> anyhow::Result<()> {
        // Get state snapshot before borrowing terminal mutably
        let state = {
            let state_guard = self.state.lock().unwrap();
            state_guard.clone()
        };

        // Get start_time before borrowing terminal mutably
        let _start_time = self.start_time;

        if let Some(terminal) = &mut self.terminal {
            let state_clone = state.clone();
            let _config_clone = config.clone();
            terminal.draw(move |f| {
                // Create a simple layout without needing self
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // Header
                        Constraint::Length(3), // Progress
                        Constraint::Min(10),   // Main content
                        Constraint::Length(3), // Footer
                    ])
                    .split(f.size());

                // Render header
                let header_block = Block::default()
                    .title("SSM Connection Diagnostics")
                    .borders(Borders::ALL);
                f.render_widget(header_block, chunks[0]);

                // Render progress if available
                if let Some(progress) = &state_clone.current_progress {
                    let progress_text = format!(
                        "Progress: {}/{} - {}",
                        progress.completed, progress.total, progress.current_item
                    );
                    let progress_paragraph =
                        Paragraph::new(progress_text).block(Block::default().borders(Borders::ALL));
                    f.render_widget(progress_paragraph, chunks[1]);
                }

                // Render main content (results)
                let results_text = if state_clone.diagnostic_results.is_empty() {
                    "No results yet...".to_string()
                } else {
                    state_clone
                        .diagnostic_results
                        .iter()
                        .map(|r| format!("{}: {}", r.item_name, r.message))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let results_paragraph = Paragraph::new(results_text)
                    .block(Block::default().title("Results").borders(Borders::ALL))
                    .wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(results_paragraph, chunks[2]);

                // Render footer
                let footer_text = if state_clone.is_paused {
                    "Diagnostics paused... Press any key to continue"
                } else if !state_clone.failed_items.is_empty() {
                    "Some diagnostics failed - check results above"
                } else if !state_clone.completed_items.is_empty() {
                    "Diagnostics in progress..."
                } else {
                    "Starting diagnostics... Press Ctrl+C to interrupt"
                };
                let footer_paragraph =
                    Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
                f.render_widget(footer_paragraph, chunks[3]);
            })?;
        }

        Ok(())
    }

    /// Get progress receiver for external monitoring
    pub fn get_progress_receiver(&self) -> watch::Receiver<Option<DiagnosticProgress>> {
        self.progress_receiver.clone()
    }

    /// Get result receiver for external monitoring
    pub fn get_result_receiver(&self) -> watch::Receiver<Vec<DiagnosticResult>> {
        self.result_receiver.clone()
    }

    /// Check if the system is currently paused
    pub fn is_paused(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.is_paused
    }

    /// Check if the system is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

impl Drop for DiagnosticFeedbackSystem {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{DiagnosticResult, DiagnosticStatus, Severity};
    use std::time::Duration;

    #[tokio::test]
    async fn test_feedback_system_creation() {
        let system = DiagnosticFeedbackSystem::new().expect("Failed to create feedback system");
        assert!(!system.is_running());
        assert!(!system.is_paused());
    }

    #[tokio::test]
    async fn test_progress_update() {
        let system = DiagnosticFeedbackSystem::new().expect("Failed to create feedback system");

        let progress =
            DiagnosticProgress::new("test_item".to_string(), 1, 5, Duration::from_secs(10));

        system
            .update_progress(progress.clone())
            .expect("Failed to update progress");

        let state = system.get_state_snapshot();
        assert!(state.current_progress.is_some());
        assert_eq!(state.current_item, "test_item");
    }

    #[tokio::test]
    async fn test_result_addition() {
        let system = DiagnosticFeedbackSystem::new().expect("Failed to create feedback system");

        let result = DiagnosticResult::success(
            "test_item".to_string(),
            "Test completed successfully".to_string(),
            Duration::from_millis(100),
        );

        system
            .add_result(result.clone())
            .expect("Failed to add result");

        let state = system.get_state_snapshot();
        assert_eq!(state.diagnostic_results.len(), 1);
        assert_eq!(state.completed_items.len(), 1);
        assert!(state.completed_items.contains(&"test_item".to_string()));
    }

    #[tokio::test]
    async fn test_critical_issue_handling() {
        let system = DiagnosticFeedbackSystem::new().expect("Failed to create feedback system");

        let critical_result = DiagnosticResult::error(
            "critical_test".to_string(),
            "Critical error occurred".to_string(),
            Duration::from_millis(50),
            Severity::Critical,
        );

        system
            .add_result(critical_result.clone())
            .expect("Failed to add critical result");

        let state = system.get_state_snapshot();
        assert_eq!(state.critical_issues.len(), 1);
        assert_eq!(state.failed_items.len(), 1);
    }

    #[test]
    fn test_feedback_display_config() {
        let config = FeedbackDisplayConfig::default();

        assert!(config.show_progress_bar);
        assert!(config.show_item_details);
        assert!(config.color_coding_enabled);
        assert!(config.auto_scroll);
        assert_eq!(config.refresh_interval_ms, 100);
        assert_eq!(config.confirmation_timeout_seconds, 30);
    }

    #[test]
    fn test_confirmation_request() {
        let issue = DiagnosticResult::error(
            "test_issue".to_string(),
            "Test error".to_string(),
            Duration::from_millis(100),
            Severity::High,
        );

        let confirmation = ConfirmationRequest {
            issue: issue.clone(),
            message: "Continue with diagnostics?".to_string(),
            options: vec!["Yes".to_string(), "No".to_string()],
            default_option: 0,
            timestamp: Instant::now(),
        };

        assert_eq!(confirmation.issue.item_name, "test_issue");
        assert_eq!(confirmation.options.len(), 2);
        assert_eq!(confirmation.default_option, 0);
    }
}

/// Render the main layout (standalone function to avoid borrowing issues)
fn render_main_layout(
    f: &mut Frame,
    state: &FeedbackState,
    config: &FeedbackDisplayConfig,
    start_time: Instant,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Length(3), // Progress bar
                Constraint::Min(10),   // Main content
                Constraint::Length(4), // Status footer
            ]
            .as_ref(),
        )
        .split(f.size());

    // Header
    render_header(f, chunks[0], state);

    // Progress bar
    if config.show_progress_bar {
        render_progress_bar(f, chunks[1], state, start_time);
    }

    // Main content
    render_main_content(f, chunks[2], state, config);

    // Status footer
    render_status_footer(f, chunks[3], state, start_time);

    // Confirmation dialog overlay
    if let Some(confirmation) = &state.confirmation_pending {
        render_confirmation_dialog(f, confirmation);
    }
}

/// Render header section
fn render_header(f: &mut Frame, area: Rect, state: &FeedbackState) {
    let title = if state.is_paused {
        "SSM接続診断 - 一時停止中"
    } else {
        "SSM接続診断 - 実行中"
    };

    let header = Paragraph::new(title)
        .block(
            Block::default()
                .title("診断システム")
                .borders(Borders::ALL)
                .border_style(if state.is_paused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                }),
        )
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

/// Render progress bar
fn render_progress_bar(f: &mut Frame, area: Rect, state: &FeedbackState, start_time: Instant) {
    if let Some(progress) = &state.current_progress {
        let progress_ratio = progress.progress_percentage() / 100.0;
        let elapsed = start_time.elapsed();

        let label = format!(
            "{} ({}/{}) - 経過時間: {:?}{}",
            progress.current_item,
            progress.completed,
            progress.total,
            elapsed,
            if let Some(remaining) = progress.estimated_remaining {
                format!(" - 残り時間: {:?}", remaining)
            } else {
                String::new()
            }
        );

        let gauge = Gauge::default()
            .block(Block::default().title("進捗").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Blue))
            .percent((progress_ratio * 100.0) as u16)
            .label(label);

        f.render_widget(gauge, area);
    } else {
        let gauge = Gauge::default()
            .block(Block::default().title("進捗").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Gray))
            .percent(0)
            .label("診断開始待ち...");

        f.render_widget(gauge, area);
    }
}

/// Render main content area
fn render_main_content(
    f: &mut Frame,
    area: Rect,
    state: &FeedbackState,
    config: &FeedbackDisplayConfig,
) {
    if config.show_item_details && state.show_details {
        render_detailed_results(f, area, state);
    } else {
        render_summary_results(f, area, state);
    }
}

/// Render detailed results view
fn render_detailed_results(f: &mut Frame, area: Rect, state: &FeedbackState) {
    let items: Vec<ListItem> = state
        .diagnostic_results
        .iter()
        .skip(state.scroll_offset)
        .enumerate()
        .map(|(i, result)| {
            let (icon, style) = match result.status {
                DiagnosticStatus::Success => ("✅", Style::default().fg(Color::Green)),
                DiagnosticStatus::Warning => ("⚠️", Style::default().fg(Color::Yellow)),
                DiagnosticStatus::Error => ("❌", Style::default().fg(Color::Red)),
                DiagnosticStatus::Skipped => ("⏭️", Style::default().fg(Color::Gray)),
            };

            let severity_text = match result.severity {
                Severity::Critical => " [重大]",
                Severity::High => " [高]",
                Severity::Medium => " [中]",
                Severity::Low => " [低]",
                Severity::Info => "",
            };

            let text = format!(
                "{} {}{} - {} ({}ms)",
                icon,
                result.item_name,
                severity_text,
                result.message,
                result.duration.as_millis()
            );

            let mut item_style = style;
            if i + state.scroll_offset == state.selected_item {
                item_style = item_style.add_modifier(Modifier::REVERSED);
            }

            ListItem::new(text).style(item_style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title("診断結果詳細").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
}

/// Render summary results view
fn render_summary_results(f: &mut Frame, area: Rect, state: &FeedbackState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ]
            .as_ref(),
        )
        .split(area);

    // Success count
    let success_count = state.completed_items.len();
    let success_gauge = Gauge::default()
        .block(
            Block::default()
                .title("成功")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .percent(100)
        .label(format!("{} 項目", success_count));
    f.render_widget(success_gauge, chunks[0]);

    // Warning count
    let warning_count = state.warning_items.len();
    let warning_gauge = Gauge::default()
        .block(
            Block::default()
                .title("警告")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .gauge_style(Style::default().fg(Color::Yellow))
        .percent(if warning_count > 0 { 100 } else { 0 })
        .label(format!("{} 項目", warning_count));
    f.render_widget(warning_gauge, chunks[1]);

    // Error count
    let error_count = state.failed_items.len();
    let error_gauge = Gauge::default()
        .block(
            Block::default()
                .title("エラー")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .gauge_style(Style::default().fg(Color::Red))
        .percent(if error_count > 0 { 100 } else { 0 })
        .label(format!("{} 項目", error_count));
    f.render_widget(error_gauge, chunks[2]);

    // Critical issues count
    let critical_count = state.critical_issues.len();
    let critical_gauge = Gauge::default()
        .block(
            Block::default()
                .title("重大問題")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .gauge_style(Style::default().fg(Color::Magenta))
        .percent(if critical_count > 0 { 100 } else { 0 })
        .label(format!("{} 項目", critical_count));
    f.render_widget(critical_gauge, chunks[3]);
}

/// Render status footer
fn render_status_footer(f: &mut Frame, area: Rect, state: &FeedbackState, start_time: Instant) {
    let status_text = if state.is_paused {
        "一時停止中 - Spaceで再開, qで終了, dで詳細表示切替"
    } else {
        "実行中 - Spaceで一時停止, qで終了, dで詳細表示切替, ↑↓でスクロール"
    };

    let elapsed = start_time.elapsed();
    let footer_text = format!(
        "{} | 経過時間: {:?} | 最終更新: {:?}前",
        status_text,
        elapsed,
        state.last_update.elapsed()
    );

    let footer = Paragraph::new(footer_text)
        .block(
            Block::default()
                .title("操作ガイド")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left);

    f.render_widget(footer, area);
}

/// Render confirmation dialog
fn render_confirmation_dialog(f: &mut Frame, confirmation: &ConfirmationRequest) {
    let area = f.size();
    let popup_area = Rect {
        x: area.width / 4,
        y: area.height / 4,
        width: area.width / 2,
        height: area.height / 2,
    };

    // Clear background
    f.render_widget(Clear, popup_area);

    // Dialog content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Title
                Constraint::Min(3),    // Message
                Constraint::Length(3), // Options
            ]
            .as_ref(),
        )
        .split(popup_area);

    // Title
    let title = Paragraph::new("重大問題の確認")
        .block(
            Block::default()
                .title("確認が必要")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Message
    let message_text = format!(
        "項目: {}\n問題: {}\n\n{}",
        confirmation.issue.item_name, confirmation.issue.message, confirmation.message
    );
    let message = Paragraph::new(message_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(message, chunks[1]);

    // Options
    let options_text = format!(
        "選択肢: {} | Enterで続行, nで中止",
        confirmation.options.join(" / ")
    );
    let options = Paragraph::new(options_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);
    f.render_widget(options, chunks[2]);

    // Render the dialog with border
    let dialog_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
    f.render_widget(dialog_block, popup_area);
}
