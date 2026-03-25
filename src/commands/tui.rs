use anyhow::Result;
use tracing::{error, info};

use crate::config::Config;
use crate::{session, ui};

pub async fn handle_tui(_config: &Config) -> Result<()> {
    info!("Launching Terminal UI");

    println!("🖥️  Starting Terminal UI...");

    // Create Terminal UI
    let mut terminal_ui = ui::TerminalUi::new()?;

    // Initialize with some sample data for demonstration
    let sample_sessions = vec![
        session::Session {
            id: "session-001".to_string(),
            instance_id: "i-1234567890abcdef0".to_string(),
            local_port: 8080,
            remote_port: 80,
            remote_host: None,
            status: session::SessionStatus::Active,
            created_at: std::time::SystemTime::now() - std::time::Duration::from_secs(300),
            last_activity: std::time::SystemTime::now() - std::time::Duration::from_secs(30),
            process_id: Some(12345),
            connection_count: 5,
            data_transferred: 1024000,
            aws_profile: Some("default".to_string()),
            region: "us-east-1".to_string(),
            priority: session::SessionPriority::Normal,
            tags: std::collections::HashMap::new(),
        },
        session::Session {
            id: "session-002".to_string(),
            instance_id: "i-0987654321fedcba0".to_string(),
            local_port: 8081,
            remote_port: 443,
            remote_host: None,
            status: session::SessionStatus::Connecting,
            created_at: std::time::SystemTime::now() - std::time::Duration::from_secs(60),
            last_activity: std::time::SystemTime::now() - std::time::Duration::from_secs(10),
            process_id: Some(12346),
            connection_count: 0,
            data_transferred: 0,
            aws_profile: None,
            region: "us-west-2".to_string(),
            priority: session::SessionPriority::Normal,
            tags: std::collections::HashMap::new(),
        },
    ];

    // Update UI with sample data
    terminal_ui.update_sessions(sample_sessions);

    // Update metrics
    let sample_metrics = ui::ResourceMetrics {
        memory_usage_mb: 8.5,
        cpu_usage_percent: 0.3,
        uptime_seconds: 3600,
    };
    terminal_ui.update_metrics(sample_metrics);

    // Add some sample warnings
    terminal_ui
        .add_warning("Session session-002 is taking longer than expected to connect".to_string());
    terminal_ui.add_warning("Memory usage is approaching 85% of the 10MB limit".to_string());

    // Set initial progress for demonstration
    terminal_ui.set_progress(
        "Initializing".to_string(),
        0.8,
        "Loading session data...".to_string(),
    );

    // Clear progress after a moment
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    terminal_ui.clear_progress();

    // Run the Terminal UI
    match terminal_ui.run().await {
        Ok(_) => {
            println!("👋 Terminal UI closed");
        }
        Err(e) => {
            error!("Terminal UI error: {}", e);
            println!("❌ Terminal UI error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
