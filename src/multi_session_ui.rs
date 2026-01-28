use crate::multi_session::{MultiSessionManager, MultiSessionState, ResourceWarningLevel};
use crate::session::{SessionPriority, SessionStatus};
use crate::manager::SessionManager;
use crate::monitor::SessionMonitor;
use crate::error::Result;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
    },
    Frame,
};
use std::time::SystemTime;

/// Multi-session UI state
#[derive(Debug, Clone)]
pub struct MultiSessionUiState {
    pub selected_tab: usize,
    pub selected_session: usize,
    pub show_warnings: bool,
    pub show_details: bool,
    pub auto_refresh: bool,
    pub refresh_interval_seconds: u64,
}

impl Default for MultiSessionUiState {
    fn default() -> Self {
        Self {
            selected_tab: 0,
            selected_session: 0,
            show_warnings: true,
            show_details: false,
            auto_refresh: true,
            refresh_interval_seconds: 5,
        }
    }
}

/// Multi-session UI renderer
pub struct MultiSessionUi<M: SessionManager, Mon: SessionMonitor> {
    manager: MultiSessionManager<M, Mon>,
    ui_state: MultiSessionUiState,
    last_update: Option<SystemTime>,
}

impl<M: SessionManager + Send + Sync, Mon: SessionMonitor + Send + Sync> MultiSessionUi<M, Mon> {
    pub fn new(manager: MultiSessionManager<M, Mon>) -> Self {
        Self {
            manager,
            ui_state: MultiSessionUiState::default(),
            last_update: None,
        }
    }
    
    /// Render the multi-session UI
    pub async fn render(&mut self, f: &mut Frame<'_>) -> Result<()> {
        let size = f.size();
        
        // Update state if needed
        if self.should_refresh().await {
            self.manager.update_state().await?;
            self.last_update = Some(SystemTime::now());
        }
        
        let state = self.manager.get_state().await;
        
        // Main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Footer
            ])
            .split(size);
        
        // Render header
        self.render_header(f, chunks[0], &state);
        
        // Render content based on selected tab
        match self.ui_state.selected_tab {
            0 => self.render_sessions_overview(f, chunks[1], &state).await?,
            1 => self.render_resource_monitoring(f, chunks[1], &state).await?,
            2 => self.render_warnings_alerts(f, chunks[1], &state).await?,
            3 => self.render_session_details(f, chunks[1], &state).await?,
            _ => self.render_sessions_overview(f, chunks[1], &state).await?,
        }
        
        // Render footer
        self.render_footer(f, chunks[2]);
        
        Ok(())
    }
    
    /// Check if UI should refresh
    async fn should_refresh(&self) -> bool {
        if !self.ui_state.auto_refresh {
            return false;
        }
        
        match self.last_update {
            Some(last) => {
                last.elapsed()
                    .map(|d| d.as_secs() >= self.ui_state.refresh_interval_seconds)
                    .unwrap_or(true)
            }
            None => true,
        }
    }
    
    /// Render header with tabs
    fn render_header(&self, f: &mut Frame<'_>, area: Rect, _state: &MultiSessionState) {
        let tab_titles = vec!["Sessions", "Resources", "Warnings", "Details"];
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("EC2 Connect - Multi-Session Manager"))
            .select(self.ui_state.selected_tab)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        
        f.render_widget(tabs, area);
    }
    
    /// Render sessions overview
    async fn render_sessions_overview(&self, f: &mut Frame<'_>, area: Rect, state: &MultiSessionState) -> Result<()> {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);
        
        // Sessions list
        self.render_sessions_list(f, chunks[0]).await?;
        
        // Statistics panel
        self.render_statistics_panel(f, chunks[1], state);
        
        Ok(())
    }
    
    /// Render sessions list
    async fn render_sessions_list(&self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let sessions = self.manager.get_sessions_by_priority().await?;
        
        let header = Row::new(vec![
            Cell::from("ID"),
            Cell::from("Instance"),
            Cell::from("Port"),
            Cell::from("Status"),
            Cell::from("Priority"),
            Cell::from("Idle"),
        ])
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        
        let rows: Vec<Row> = sessions
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let style = if i == self.ui_state.selected_session {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                
                let status_color = match session.status {
                    SessionStatus::Active => Color::Green,
                    SessionStatus::Connecting => Color::Yellow,
                    SessionStatus::Reconnecting => Color::Magenta,
                    SessionStatus::Inactive => Color::Gray,
                    SessionStatus::Terminated => Color::Red,
                };
                
                let priority_color = match session.priority {
                    SessionPriority::Critical => Color::Red,
                    SessionPriority::High => Color::Magenta,
                    SessionPriority::Normal => Color::White,
                    SessionPriority::Low => Color::Gray,
                };
                
                Row::new(vec![
                    Cell::from(&session.id[..8]),
                    Cell::from(session.instance_id.as_str()),
                    Cell::from(format!("{}:{}", session.local_port, session.remote_port)),
                    Cell::from(session.status.to_string()).style(Style::default().fg(status_color)),
                    Cell::from(session.priority.to_string()).style(Style::default().fg(priority_color)),
                    Cell::from(format!("{}s", session.idle_seconds())),
                ])
                .style(style)
            })
            .collect();
        
        let widths = [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(6),
        ];
        
        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("Active Sessions"));
        
        f.render_widget(table, area);
        
        Ok(())
    }
    
    /// Render statistics panel
    fn render_statistics_panel(&self, f: &mut Frame<'_>, area: Rect, state: &MultiSessionState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),  // Summary
                Constraint::Length(6),  // Priority breakdown
                Constraint::Min(0),     // Instance breakdown
            ])
            .split(area);
        
        // Summary statistics
        let summary_text = vec![
            Line::from(vec![
                Span::styled("Total Sessions: ", Style::default().fg(Color::White)),
                Span::styled(state.total_sessions.to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Active Sessions: ", Style::default().fg(Color::White)),
                Span::styled(state.active_sessions.to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Memory Usage: ", Style::default().fg(Color::White)),
                Span::styled(format!("{:.1} MB", state.resource_usage.memory_mb), 
                           if state.resource_usage.memory_mb > 8.0 { 
                               Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                           } else { 
                               Style::default().fg(Color::Green) 
                           }),
            ]),
            Line::from(vec![
                Span::styled("CPU Usage: ", Style::default().fg(Color::White)),
                Span::styled(format!("{:.1}%", state.resource_usage.cpu_percent), 
                           if state.resource_usage.cpu_percent > 0.3 { 
                               Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                           } else { 
                               Style::default().fg(Color::Green) 
                           }),
            ]),
            Line::from(vec![
                Span::styled("Warnings: ", Style::default().fg(Color::White)),
                Span::styled(state.resource_warnings.len().to_string(), 
                           if state.resource_warnings.is_empty() { 
                               Style::default().fg(Color::Green) 
                           } else { 
                               Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                           }),
            ]),
        ];
        
        let summary = Paragraph::new(summary_text)
            .block(Block::default().borders(Borders::ALL).title("Summary"))
            .wrap(Wrap { trim: true });
        
        f.render_widget(summary, chunks[0]);
        
        // Priority breakdown
        let priority_items: Vec<ListItem> = [
            SessionPriority::Critical,
            SessionPriority::High,
            SessionPriority::Normal,
            SessionPriority::Low,
        ]
        .iter()
        .map(|priority| {
            let count = state.sessions_by_priority.get(priority).unwrap_or(&0);
            let color = match priority {
                SessionPriority::Critical => Color::Red,
                SessionPriority::High => Color::Magenta,
                SessionPriority::Normal => Color::White,
                SessionPriority::Low => Color::Gray,
            };
            
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:?}: ", priority), Style::default().fg(color)),
                Span::styled(count.to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]))
        })
        .collect();
        
        let priority_list = List::new(priority_items)
            .block(Block::default().borders(Borders::ALL).title("By Priority"));
        
        f.render_widget(priority_list, chunks[1]);
        
        // Instance breakdown
        let instance_items: Vec<ListItem> = state
            .sessions_by_instance
            .iter()
            .map(|(instance, count)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{}: ", instance), Style::default().fg(Color::Cyan)),
                    Span::styled(count.to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]))
            })
            .collect();
        
        let instance_list = List::new(instance_items)
            .block(Block::default().borders(Borders::ALL).title("By Instance"));
        
        f.render_widget(instance_list, chunks[2]);
    }
    
    /// Render resource monitoring
    async fn render_resource_monitoring(&self, f: &mut Frame<'_>, area: Rect, state: &MultiSessionState) -> Result<()> {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Memory gauge
                Constraint::Length(3), // CPU gauge
                Constraint::Min(0),    // Resource history/details
            ])
            .split(area);
        
        // Memory usage gauge
        let memory_ratio = (state.resource_usage.memory_mb / 10.0).min(1.0);
        let memory_color = if memory_ratio > 0.8 {
            Color::Red
        } else if memory_ratio > 0.6 {
            Color::Yellow
        } else {
            Color::Green
        };
        
        let memory_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
            .gauge_style(Style::default().fg(memory_color))
            .ratio(memory_ratio)
            .label(format!("{:.1} MB / 10.0 MB", state.resource_usage.memory_mb));
        
        f.render_widget(memory_gauge, chunks[0]);
        
        // CPU usage gauge
        let cpu_ratio = (state.resource_usage.cpu_percent / 0.5).min(1.0);
        let cpu_color = if cpu_ratio > 0.8 {
            Color::Red
        } else if cpu_ratio > 0.6 {
            Color::Yellow
        } else {
            Color::Green
        };
        
        let cpu_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
            .gauge_style(Style::default().fg(cpu_color))
            .ratio(cpu_ratio)
            .label(format!("{:.1}% / 0.5%", state.resource_usage.cpu_percent));
        
        f.render_widget(cpu_gauge, chunks[1]);
        
        // Resource optimization suggestions
        let optimization_text = self.get_optimization_suggestions(state);
        let optimization = Paragraph::new(optimization_text)
            .block(Block::default().borders(Borders::ALL).title("Optimization Suggestions"))
            .wrap(Wrap { trim: true });
        
        f.render_widget(optimization, chunks[2]);
        
        Ok(())
    }
    
    /// Get optimization suggestions based on current state
    fn get_optimization_suggestions(&self, state: &MultiSessionState) -> Text<'_> {
        let mut suggestions = Vec::new();
        
        if state.resource_usage.memory_mb > 8.0 {
            suggestions.push(Line::from(vec![
                Span::styled("⚠ ", Style::default().fg(Color::Red)),
                Span::styled("High memory usage detected. Consider terminating idle sessions.", Style::default().fg(Color::White)),
            ]));
        }
        
        if state.resource_usage.cpu_percent > 0.3 {
            suggestions.push(Line::from(vec![
                Span::styled("⚠ ", Style::default().fg(Color::Yellow)),
                Span::styled("High CPU usage. Reduce monitoring frequency for idle sessions.", Style::default().fg(Color::White)),
            ]));
        }
        
        let total_low_priority = state.sessions_by_priority.get(&SessionPriority::Low).unwrap_or(&0);
        if *total_low_priority > 2 {
            suggestions.push(Line::from(vec![
                Span::styled("💡 ", Style::default().fg(Color::Blue)),
                Span::styled("Multiple low-priority sessions detected. Consider consolidation.", Style::default().fg(Color::White)),
            ]));
        }
        
        if suggestions.is_empty() {
            suggestions.push(Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled("Resource usage is optimal.", Style::default().fg(Color::Green)),
            ]));
        }
        
        Text::from(suggestions)
    }
    
    /// Render warnings and alerts
    async fn render_warnings_alerts(&self, f: &mut Frame<'_>, area: Rect, _state: &MultiSessionState) -> Result<()> {
        let warnings = self.manager.get_warning_history().await;
        
        let warning_items: Vec<ListItem> = warnings
            .iter()
            .rev() // Show most recent first
            .take(20) // Limit to last 20 warnings
            .map(|warning| {
                let level_color = match warning.level {
                    ResourceWarningLevel::Critical => Color::Red,
                    ResourceWarningLevel::Warning => Color::Yellow,
                    ResourceWarningLevel::Normal => Color::Green,
                    ResourceWarningLevel::Emergency => Color::Magenta,
                };
                
                let level_symbol = match warning.level {
                    ResourceWarningLevel::Critical => "🔴",
                    ResourceWarningLevel::Warning => "🟡",
                    ResourceWarningLevel::Normal => "🟢",
                    ResourceWarningLevel::Emergency => "🚨",
                };
                
                let timestamp = warning.timestamp
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| {
                        let secs = d.as_secs();
                        format!("{:02}:{:02}:{:02}", 
                               (secs / 3600) % 24, 
                               (secs / 60) % 60, 
                               secs % 60)
                    })
                    .unwrap_or_else(|_| "??:??:??".to_string());
                
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(level_symbol, Style::default().fg(level_color)),
                        Span::styled(format!(" [{}] ", timestamp), Style::default().fg(Color::Gray)),
                        Span::styled(&warning.message, Style::default().fg(Color::White)),
                    ]),
                    Line::from(vec![
                        Span::styled(format!("    Resource: {} | Current: {:.1} | Threshold: {:.1}", 
                                           warning.resource_type, warning.current_value, warning.threshold), 
                                   Style::default().fg(Color::Gray)),
                    ]),
                ])
            })
            .collect();
        
        let warnings_list = List::new(warning_items)
            .block(Block::default().borders(Borders::ALL).title(format!("Warnings & Alerts ({})", warnings.len())));
        
        f.render_widget(warnings_list, area);
        
        Ok(())
    }
    
    /// Render session details
    async fn render_session_details(&self, f: &mut Frame<'_>, area: Rect, _state: &MultiSessionState) -> Result<()> {
        let sessions = self.manager.get_sessions_by_priority().await?;
        
        if let Some(session) = sessions.get(self.ui_state.selected_session) {
            let details_text = vec![
                Line::from(vec![
                    Span::styled("Session ID: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(&session.id, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Instance ID: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(&session.instance_id, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Local Port: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(session.local_port.to_string(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Remote Port: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(session.remote_port.to_string(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(session.status.to_string(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Priority: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(session.priority.to_string(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Age: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}s", session.age_seconds()), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Idle Time: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}s", session.idle_seconds()), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Connections: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(session.connection_count.to_string(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Data Transferred: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} bytes", session.data_transferred), Style::default().fg(Color::White)),
                ]),
            ];
            
            let details = Paragraph::new(details_text)
                .block(Block::default().borders(Borders::ALL).title("Session Details"))
                .wrap(Wrap { trim: true });
            
            f.render_widget(details, area);
        } else {
            let no_session = Paragraph::new("No session selected")
                .block(Block::default().borders(Borders::ALL).title("Session Details"))
                .alignment(Alignment::Center);
            
            f.render_widget(no_session, area);
        }
        
        Ok(())
    }
    
    /// Render footer with controls
    fn render_footer(&self, f: &mut Frame<'_>, area: Rect) {
        let controls = vec![
            Line::from(vec![
                Span::styled("Tab: ", Style::default().fg(Color::Yellow)),
                Span::styled("1-4", Style::default().fg(Color::White)),
                Span::styled(" | Select: ", Style::default().fg(Color::Yellow)),
                Span::styled("↑↓", Style::default().fg(Color::White)),
                Span::styled(" | Refresh: ", Style::default().fg(Color::Yellow)),
                Span::styled("R", Style::default().fg(Color::White)),
                Span::styled(" | Quit: ", Style::default().fg(Color::Yellow)),
                Span::styled("Q", Style::default().fg(Color::White)),
            ]),
        ];
        
        let footer = Paragraph::new(controls)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);
        
        f.render_widget(footer, area);
    }
    
    /// Handle input events
    pub fn handle_input(&mut self, key: char) {
        match key {
            '1' => self.ui_state.selected_tab = 0,
            '2' => self.ui_state.selected_tab = 1,
            '3' => self.ui_state.selected_tab = 2,
            '4' => self.ui_state.selected_tab = 3,
            'j' | 'J' => {
                if self.ui_state.selected_session > 0 {
                    self.ui_state.selected_session -= 1;
                }
            }
            'k' | 'K' => {
                self.ui_state.selected_session += 1;
            }
            'r' | 'R' => {
                self.last_update = None; // Force refresh
            }
            't' | 'T' => {
                self.ui_state.auto_refresh = !self.ui_state.auto_refresh;
            }
            'w' | 'W' => {
                self.ui_state.show_warnings = !self.ui_state.show_warnings;
            }
            'd' | 'D' => {
                self.ui_state.show_details = !self.ui_state.show_details;
            }
            _ => {}
        }
    }
    
    /// Get UI state
    pub fn get_ui_state(&self) -> &MultiSessionUiState {
        &self.ui_state
    }
    
    /// Set UI state
    pub fn set_ui_state(&mut self, state: MultiSessionUiState) {
        self.ui_state = state;
    }
}