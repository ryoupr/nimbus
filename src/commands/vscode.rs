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

#[cfg(feature = "performance-monitoring")]
use crate::monitor::DefaultSessionMonitor;
#[cfg(feature = "multi-session")]
use crate::multi_session::{MultiSessionManager, ResourceThresholds};
#[cfg(feature = "multi-session")]
use crate::multi_session_ui::MultiSessionUi;
#[cfg(feature = "persistence")]
use crate::persistence::{PersistenceManager, SqlitePersistenceManager};

pub async fn handle_vscode(action: VsCodeCommands, config: &Config) -> Result<()> {
    use crate::session;
    use crate::vscode::VsCodeIntegration;
    use tokio::fs;
    match action {
        VsCodeCommands::Status => {
            info!("Checking VS Code integration status");
            println!("🔧 VS Code Integration Status:");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => match integration.check_integration_status().await {
                    Ok(status) => {
                        println!(
                            "  📊 Overall Status: {}",
                            if status.is_fully_available() {
                                "✅ Ready"
                            } else {
                                "⚠️  Partial"
                            }
                        );
                        println!();

                        println!("  🔍 Component Status:");
                        println!(
                            "    VS Code: {}",
                            if status.vscode_available {
                                "✅ Available"
                            } else {
                                "❌ Not Found"
                            }
                        );
                        if let Some(path) = &status.vscode_path {
                            println!("      Path: {:?}", path);
                        }

                        println!(
                            "    SSH Config: {}",
                            if status.ssh_config_writable {
                                "✅ Writable"
                            } else {
                                "❌ Not Writable"
                            }
                        );
                        println!("      Path: {:?}", status.ssh_config_path);

                        println!(
                            "    Auto Launch: {}",
                            if status.auto_launch_enabled {
                                "✅ Enabled"
                            } else {
                                "⚪ Disabled"
                            }
                        );

                        println!(
                            "    Notifications: {}",
                            if status.notifications_enabled {
                                "✅ Enabled"
                            } else {
                                "⚪ Disabled"
                            }
                        );

                        println!();

                        let features = status.available_features();
                        if !features.is_empty() {
                            println!("  ✅ Available Features:");
                            for feature in features {
                                println!("    • {}", feature);
                            }
                            println!();
                        }

                        let missing = status.missing_requirements();
                        if !missing.is_empty() {
                            println!("  ❌ Missing Requirements:");
                            for requirement in missing {
                                println!("    • {}", requirement);
                            }
                            println!();

                            println!("  💡 Recommendations:");
                            if !status.vscode_available {
                                println!(
                                    "    • Install VS Code from https://code.visualstudio.com/"
                                );
                                println!("    • Or set NIMBUS_VSCODE_PATH environment variable");
                            }
                            if !status.ssh_config_writable {
                                println!("    • Check permissions on ~/.ssh/config file");
                                println!("    • Create ~/.ssh directory if it doesn't exist");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to check integration status: {}", e);
                        println!("  ❌ Status check failed: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }

        VsCodeCommands::Test { session_id } => {
            info!("Testing VS Code integration");
            println!("🧪 Testing VS Code Integration:");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => {
                    // Check status first
                    match integration.check_integration_status().await {
                        Ok(status) => {
                            if !status.is_fully_available() {
                                println!("  ⚠️  Integration not fully available. Run 'vscode status' for details.");
                                return Ok(());
                            }

                            // Create or use existing session for testing
                            let test_session = match session_id {
                                Some(id) => {
                                    println!("  🔍 Using existing session: {}", id);
                                    // In a real implementation, you would load the session from the session manager
                                    // For now, create a mock session
                                    session::Session {
                                        id: id.clone(),
                                        instance_id: "i-test123456789abcdef".to_string(),
                                        local_port: 8080,
                                        remote_port: 22,
                                        remote_host: None,
                                        status: session::SessionStatus::Active,
                                        created_at: std::time::SystemTime::now(),
                                        last_activity: std::time::SystemTime::now(),
                                        process_id: Some(12345),
                                        connection_count: 1,
                                        data_transferred: 0,
                                        aws_profile: None,
                                        region: "us-east-1".to_string(),
                                        priority: session::SessionPriority::Normal,
                                        tags: std::collections::HashMap::new(),
                                    }
                                }
                                None => {
                                    println!("  🆕 Creating test session...");
                                    session::Session {
                                        id: "test-session-vscode".to_string(),
                                        instance_id: "i-test123456789abcdef".to_string(),
                                        local_port: 8080,
                                        remote_port: 22,
                                        remote_host: None,
                                        status: session::SessionStatus::Active,
                                        created_at: std::time::SystemTime::now(),
                                        last_activity: std::time::SystemTime::now(),
                                        process_id: Some(12345),
                                        connection_count: 1,
                                        data_transferred: 0,
                                        aws_profile: None,
                                        region: "us-east-1".to_string(),
                                        priority: session::SessionPriority::Normal,
                                        tags: std::collections::HashMap::new(),
                                    }
                                }
                            };

                            println!("  📋 Test Session Details:");
                            println!("    Session ID: {}", test_session.id);
                            println!("    Instance ID: {}", test_session.instance_id);
                            println!("    Local Port: {}", test_session.local_port);
                            println!("    Remote Port: {}", test_session.remote_port);
                            println!();

                            // Perform integration test
                            match integration.integrate_session(&test_session).await {
                                Ok(result) => {
                                    println!("  📊 Integration Test Results:");
                                    println!(
                                        "    Overall Success: {}",
                                        if result.success { "✅ Yes" } else { "❌ No" }
                                    );
                                    println!(
                                        "    SSH Config Updated: {}",
                                        if result.ssh_config_updated {
                                            "✅ Yes"
                                        } else {
                                            "❌ No"
                                        }
                                    );
                                    println!(
                                        "    VS Code Launched: {}",
                                        if result.vscode_launched {
                                            "✅ Yes"
                                        } else {
                                            "❌ No"
                                        }
                                    );
                                    println!(
                                        "    Notification Sent: {}",
                                        if result.notification_sent {
                                            "✅ Yes"
                                        } else {
                                            "❌ No"
                                        }
                                    );

                                    if let Some(connection_info) = &result.connection_info {
                                        println!();
                                        println!("  🔗 Connection Information:");
                                        println!("    SSH Host: {}", connection_info.ssh_host);
                                        println!(
                                            "    Connection URL: {}",
                                            connection_info.connection_url
                                        );
                                    }

                                    if let Some(error) = &result.error_message {
                                        println!();
                                        println!("  ❌ Error Details: {}", error);
                                    }

                                    if result.success {
                                        println!();
                                        println!("  ✅ Integration test completed successfully!");
                                        println!("  💡 You can now connect to the instance using:");
                                        if let Some(connection_info) = &result.connection_info {
                                            println!("     ssh {}", connection_info.ssh_host);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Integration test failed: {}", e);
                                    println!("  ❌ Integration test failed: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to check integration status: {}", e);
                            println!("  ❌ Status check failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }

        VsCodeCommands::Setup => {
            info!("Setting up VS Code integration");
            println!("⚙️  VS Code Integration Setup:");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => match integration.check_integration_status().await {
                    Ok(status) => {
                        println!("  🔍 Current Status:");
                        println!(
                            "    VS Code: {}",
                            if status.vscode_available {
                                "✅ Found"
                            } else {
                                "❌ Not Found"
                            }
                        );
                        println!(
                            "    SSH Config: {}",
                            if status.ssh_config_writable {
                                "✅ Writable"
                            } else {
                                "❌ Not Writable"
                            }
                        );
                        println!();

                        if status.is_fully_available() {
                            println!(
                                "  ✅ VS Code integration is already set up and ready to use!"
                            );
                            println!();
                            println!("  📋 Configuration:");
                            if let Some(path) = &status.vscode_path {
                                println!("    VS Code Path: {:?}", path);
                            }
                            println!("    SSH Config: {:?}", status.ssh_config_path);
                            println!("    Auto Launch: {}", status.auto_launch_enabled);
                            println!("    Notifications: {}", status.notifications_enabled);
                        } else {
                            println!("  ⚠️  Setup incomplete. Please address the following:");
                            println!();

                            let missing = status.missing_requirements();
                            for (i, requirement) in missing.iter().enumerate() {
                                println!("  {}. {}", i + 1, requirement);
                            }

                            println!();
                            println!("  💡 Setup Instructions:");

                            if !status.vscode_available {
                                println!("    📥 Install VS Code:");
                                println!("      • Download from: https://code.visualstudio.com/");
                                println!("      • Or use package manager:");
                                println!("        - macOS: brew install --cask visual-studio-code");
                                println!("        - Ubuntu: snap install code --classic");
                                println!(
                                    "        - Windows: winget install Microsoft.VisualStudioCode"
                                );
                                println!();
                                println!("    🔧 Alternative: Set custom path");
                                println!("      export NIMBUS_VSCODE_PATH=/path/to/code");
                                println!();
                            }

                            if !status.ssh_config_writable {
                                println!("    📁 Fix SSH config permissions:");
                                println!("      mkdir -p ~/.ssh");
                                println!("      chmod 700 ~/.ssh");
                                println!("      touch ~/.ssh/config");
                                println!("      chmod 600 ~/.ssh/config");
                                println!();
                            }

                            println!("  🔄 After completing setup, run:");
                            println!("    nimbus vscode status");
                        }
                    }
                    Err(e) => {
                        error!("Failed to check integration status: {}", e);
                        println!("  ❌ Status check failed: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }

        VsCodeCommands::Cleanup { session_id } => {
            info!("Cleaning up VS Code integration");

            match VsCodeIntegration::new(config.vscode.clone()) {
                Ok(integration) => {
                    match session_id {
                        Some(id) => {
                            println!("🧹 Cleaning up SSH config for session: {}", id);

                            match integration.cleanup_ssh_config(&id).await {
                                Ok(_) => {
                                    println!("  ✅ SSH config cleaned up successfully");
                                }
                                Err(e) => {
                                    error!("Failed to clean up SSH config: {}", e);
                                    println!("  ❌ Cleanup failed: {}", e);
                                }
                            }
                        }
                        None => {
                            println!("🧹 Cleaning up all Nimbus entries from SSH config...");

                            // Read SSH config and remove all Nimbus entries
                            match integration.check_integration_status().await {
                                Ok(status) => {
                                    let ssh_config_path = &status.ssh_config_path;
                                    if ssh_config_path.exists() {
                                        match fs::read_to_string(ssh_config_path).await {
                                            Ok(content) => {
                                                let lines: Vec<&str> = content.lines().collect();
                                                let mut result_lines = Vec::new();
                                                let mut skip_section = false;

                                                for line in lines {
                                                    let trimmed = line.trim();

                                                    // Skip Nimbus sections
                                                    if trimmed.starts_with("# Nimbus") {
                                                        skip_section = true;
                                                        continue;
                                                    }

                                                    // End skip when we hit a new section or empty line
                                                    if skip_section {
                                                        if (trimmed.starts_with("Host ")
                                                            && !trimmed.contains("ec2-"))
                                                            || (trimmed.is_empty()
                                                                && result_lines.last().is_some_and(
                                                                    |l: &String| {
                                                                        l.trim().is_empty()
                                                                    },
                                                                ))
                                                        {
                                                            skip_section = false;
                                                        }

                                                        if skip_section {
                                                            continue;
                                                        }
                                                    }

                                                    result_lines.push(line.to_string());
                                                }

                                                let cleaned_content = result_lines.join("\n");

                                                match fs::write(ssh_config_path, cleaned_content)
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        println!("  ✅ All Nimbus entries removed from SSH config");
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to write cleaned SSH config: {}", e);
                                                        println!("  ❌ Failed to write cleaned SSH config: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to read SSH config: {}", e);
                                                println!("  ❌ Failed to read SSH config: {}", e);
                                            }
                                        }
                                    } else {
                                        println!("  ℹ️  SSH config file does not exist, nothing to clean");
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to get integration status: {}", e);
                                    println!("  ❌ Failed to get integration status: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to initialize VS Code integration: {}", e);
                    println!("  ❌ Integration initialization failed: {}", e);
                }
            }
        }
    }

    Ok(())
}
