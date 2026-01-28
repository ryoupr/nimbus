#![allow(dead_code)]

use crate::session::Session;
use crate::error::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs}, Terminal,
};
use std::{io, time::Instant};
use tokio::sync::mpsc;
use tracing::info;

/// UI events
#[derive(Debug, Clone)]
pub enum UiEvent {
    Quit,
    Refresh,
    CreateSession,
    ShowMetrics,
    NextTab,
    PrevTab,
    ScrollUp,
    ScrollDown,
}

/// Progress information for operations
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub operation: String,
    pub progress: f64,
    pub message: String,
    pub started_at: Instant,
}

/// System resource metrics
#[derive(Debug, Clone)]
pub struct ResourceMetrics {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub active_sessions: u32,
    pub total_connections: u32,
    pub uptime_seconds: u64,
}

impl Default for ResourceMetrics {
    fn default() -> Self {
        Self {
            memory_usage_mb: 0.0,
            cpu_usage_percent: 0.0,
            active_sessions: 0,
            total_connections: 0,
            uptime_seconds: 0,
        }
    }
}

/// UI state
#[derive(Debug, Clone)]
pub struct UiState {
    pub sessions: Vec<Session>,
    pub current_tab: usize,
    pub progress: Option<ProgressInfo>,
    pub metrics: ResourceMetrics,
    pub last_update: Instant,
    pub scroll_offset: usize,
    pub warnings: Vec<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            current_tab: 0,
            progress: None,
            metrics: ResourceMetrics::default(),
            last_update: Instant::now(),
            scroll_offset: 0,
            warnings: Vec::new(),
        }
    }
}

/// Terminal UI manager
pub struct TerminalUi {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    state: UiState,
    event_sender: mpsc::UnboundedSender<UiEvent>,
    event_receiver: mpsc::UnboundedReceiver<UiEvent>,
    start_time: Instant,
}

impl TerminalUi {
    pub fn new() -> Result<Self> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Ok(Self {
            terminal,
            state: UiState::default(),
            event_sender,
            event_receiver,
            start_time: Instant::now(),
        })
    }
    
    /// Update session data
    pub fn update_sessions(&mut self, sessions: Vec<Session>) {
        self.state.sessions = sessions;
        self.state.last_update = Instant::now();
    }
    
    /// Update resource metrics
    pub fn update_metrics(&mut self, metrics: ResourceMetrics) {
        self.state.metrics = metrics;
        self.state.metrics.uptime_seconds = self.start_time.elapsed().as_secs();
    }
    
    /// Set progress information
    pub fn set_progress(&mut self, operation: String, progress: f64, message: String) {
        self.state.progress = Some(ProgressInfo {
            operation,
            progress,
            message,
            started_at: Instant::now(),
        });
    }
    
    /// Clear progress information
    pub fn clear_progress(&mut self) {
        self.state.progress = None;
    }
    
    /// Add warning message
    pub fn add_warning(&mut self, warning: String) {
        self.state.warnings.push(warning);
        // Keep only last 10 warnings
        if self.state.warnings.len() > 10 {
            self.state.warnings.remove(0);
        }
    }
    
    /// Run the terminal UI
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting Terminal UI");
        
        // Spawn input handler
        let sender = self.event_sender.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(event) = event::read() {
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    let _ = sender.send(UiEvent::Quit);
                                    break;
                                },
                                KeyCode::Char('r') | KeyCode::F(5) => {
                                    let _ = sender.send(UiEvent::Refresh);
                                },
                                KeyCode::Char('c') => {
                                    let _ = sender.send(UiEvent::CreateSession);
                                },
                                KeyCode::Char('m') => {
                                    let _ = sender.send(UiEvent::ShowMetrics);
                                },
                                KeyCode::Tab => {
                                    let _ = sender.send(UiEvent::NextTab);
                                },
                                KeyCode::BackTab => {
                                    let _ = sender.send(UiEvent::PrevTab);
                                },
                                KeyCode::Up => {
                                    let _ = sender.send(UiEvent::ScrollUp);
                                },
                                KeyCode::Down => {
                                    let _ = sender.send(UiEvent::ScrollDown);
                                },
                                _ => {}
                            }
                        },
                        _ => {}
                    }
                }
            }
        });
        
        // Main UI loop with 1-second update interval for real-time display
        let mut last_draw = Instant::now();
        loop {
            // Draw UI at least once per second for real-time updates
            if last_draw.elapsed().as_millis() >= 1000 {
                self.draw()?;
                last_draw = Instant::now();
            }
            
            if let Ok(event) = self.event_receiver.try_recv() {
                match event {
                    UiEvent::Quit => break,
                    UiEvent::Refresh => {
                        // Force immediate redraw
                        self.draw()?;
                        #[allow(unused_assignments)]
                        { last_draw = Instant::now(); }
                    },
                    UiEvent::CreateSession => {
                        // TODO: Show create session dialog
                        self.add_warning("Create session dialog not implemented yet".to_string());
                    },
                    UiEvent::ShowMetrics => {
                        self.state.current_tab = 1;
                    },
                    UiEvent::NextTab => {
                        self.state.current_tab = (self.state.current_tab + 1) % 3;
                    },
                    UiEvent::PrevTab => {
                        self.state.current_tab = if self.state.current_tab == 0 { 2 } else { self.state.current_tab - 1 };
                    },
                    UiEvent::ScrollUp => {
                        if self.state.scroll_offset > 0 {
                            self.state.scroll_offset -= 1;
                        }
                    },
                    UiEvent::ScrollDown => {
                        self.state.scroll_offset += 1;
                    },
                }
                // Redraw immediately after handling events
                self.draw()?;
                last_draw = Instant::now();
            }
            
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        
        Ok(())
    }
    
    /// Draw the UI
    fn draw(&mut self) -> Result<()> {
        let current_tab = self.state.current_tab;
        let progress = self.state.progress.clone();
        let active_sessions = self.state.sessions.len();
        let memory_usage = self.state.metrics.memory_usage_mb;
        let cpu_usage = self.state.metrics.cpu_usage_percent;
        
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // Header
                    Constraint::Min(0),    // Main content
                    Constraint::Length(4), // Footer with status
                ].as_ref())
                .split(f.size());
            
            // Header with tabs
            let tabs = Tabs::new(vec!["Sessions", "Metrics", "Logs"])
                .block(Block::default()
                    .title("Nimbus Manager v3.0")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .select(current_tab);
            
            f.render_widget(tabs, chunks[0]);
            
            // Main content based on current tab
            match current_tab {
                0 => {
                    // Sessions tab content
                    let block = Block::default()
                        .title("Sessions")
                        .borders(Borders::ALL);
                    f.render_widget(block, chunks[1]);
                },
                1 => {
                    // Metrics tab content
                    let block = Block::default()
                        .title("Metrics")
                        .borders(Borders::ALL);
                    f.render_widget(block, chunks[1]);
                },
                2 => {
                    // Logs tab content
                    let block = Block::default()
                        .title("Logs")
                        .borders(Borders::ALL);
                    f.render_widget(block, chunks[1]);
                },
                _ => {
                    let block = Block::default()
                        .title("Sessions")
                        .borders(Borders::ALL);
                    f.render_widget(block, chunks[1]);
                }
            }
            
            // Footer with status
            let status_text = format!(
                "Status: {} sessions active | Memory: {:.1}MB | CPU: {:.2}% | Press 'q' to quit, 'r' to refresh",
                active_sessions,
                memory_usage,
                cpu_usage
            );
            
            let status = Paragraph::new(status_text)
                .block(Block::default()
                    .title("System Status")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)));
            
            f.render_widget(status, chunks[2]);
            
            // Progress overlay if active
            if progress.is_some() {
                // Progress overlay rendering will be added here
            }
        })?;
        
        Ok(())
    }
}

impl Drop for TerminalUi {
    fn drop(&mut self) {
        // Restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}