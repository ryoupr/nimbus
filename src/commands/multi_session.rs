#[allow(unused_imports)]
use anyhow::Result;
#[allow(unused_imports)]
use tracing::{error, info, warn};

#[allow(unused_imports)]
use super::{ConfigCommands, DiagnosticCommands, DiagnosticSettingsCommands, VsCodeCommands};
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
#[cfg(feature = "multi-session")]
pub async fn handle_multi_session(_config: &Config) -> Result<()> {
    info!("Launching Multi-Session Management UI");

    println!("🖥️  Starting Multi-Session Management UI...");

    // Create session manager and monitor
    let session_manager = DefaultSessionManager::new(10).await.map_err(|e| {
        NimbusError::Session(crate::error::SessionError::CreationFailed {
            reason: format!("Failed to create session manager: {}", e),
        })
    })?;

    let session_monitor = DefaultSessionMonitor::new();

    // Create resource thresholds
    let thresholds = ResourceThresholds {
        memory_warning_mb: 8.0,
        memory_critical_mb: 10.0,
        cpu_warning_percent: 0.3,
        cpu_critical_percent: 0.5,
        max_sessions_per_instance: 3,
        max_total_sessions: 10,
    };

    // Create multi-session manager
    let multi_manager =
        MultiSessionManager::new(session_manager, session_monitor, Some(thresholds));

    // Create and run multi-session UI
    let mut multi_ui = MultiSessionUi::new(multi_manager);

    println!("🎯 Multi-Session Management UI is ready!");
    println!("📋 Use tabs to navigate: 1=Sessions, 2=Resources, 3=Warnings, 4=Details");
    println!("🔄 Press 'R' to refresh, 'Q' to quit");

    // Initialize terminal
    use crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main UI loop
    let mut should_quit = false;

    while !should_quit {
        // Render UI
        if let Err(e) = terminal.draw(|f| {
            if let Err(render_error) =
                tokio::runtime::Handle::current().block_on(multi_ui.render(f))
            {
                error!("UI render error: {}", render_error);
            }
        }) {
            error!("Terminal draw error: {}", e);
            break;
        }

        // Handle events
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        should_quit = true;
                    }
                    KeyCode::Char(c) => {
                        multi_ui.handle_input(c);
                    }
                    KeyCode::Up => {
                        multi_ui.handle_input('k');
                    }
                    KeyCode::Down => {
                        multi_ui.handle_input('j');
                    }
                    KeyCode::Esc => {
                        should_quit = true;
                    }
                    _ => {}
                }
            }
        }

        // Small delay to prevent excessive CPU usage
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("👋 Multi-Session Management UI closed");

    Ok(())
}
