use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn, error, debug};
use async_trait::async_trait;

// Import diagnostic modules at the top level
use crate::instance_diagnostics::{InstanceDiagnostics, DefaultInstanceDiagnostics};
use crate::port_diagnostics::{PortDiagnostics, DefaultPortDiagnostics};
use crate::ssm_agent_diagnostics::{SsmAgentDiagnostics, DefaultSsmAgentDiagnostics};
use crate::iam_diagnostics::{IamDiagnostics, DefaultIamDiagnostics};
use crate::network_diagnostics::{NetworkDiagnostics, DefaultNetworkDiagnostics};
use crate::aws_config_validator::{AwsConfigValidator, DefaultAwsConfigValidator, AwsConfigValidationConfig, AwsConfigValidationResult};
use crate::realtime_feedback::{RealtimeFeedbackManager, FeedbackConfig, FeedbackStatus, create_progress_callback};
use crate::diagnostic_feedback::{DiagnosticFeedbackSystem, FeedbackDisplayConfig};

/// Diagnostic configuration for SSM connection diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticConfig {
    pub instance_id: String,
    pub local_port: Option<u16>,
    pub remote_port: Option<u16>,
    pub region: Option<String>,
    pub profile: Option<String>,
    pub parallel_execution: bool,
    pub timeout: Duration,
}

impl DiagnosticConfig {
    pub fn new(instance_id: String) -> Self {
        Self {
            instance_id,
            local_port: None,
            remote_port: None,
            region: None,
            profile: None,
            parallel_execution: true,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_ports(mut self, local_port: u16, remote_port: u16) -> Self {
        self.local_port = Some(local_port);
        self.remote_port = Some(remote_port);
        self
    }

    pub fn with_aws_config(mut self, region: Option<String>, profile: Option<String>) -> Self {
        self.region = region;
        self.profile = profile;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_parallel_execution(mut self, parallel: bool) -> Self {
        self.parallel_execution = parallel;
        self
    }
}

/// Status of a diagnostic check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiagnosticStatus {
    Success,
    Warning,
    Error,
    Skipped,
}

/// Severity level of diagnostic issues
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// Result of a single diagnostic check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub item_name: String,
    pub status: DiagnosticStatus,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub duration: Duration,
    pub severity: Severity,
    pub auto_fixable: bool,
}

impl DiagnosticResult {
    pub fn success(item_name: String, message: String, duration: Duration) -> Self {
        Self {
            item_name,
            status: DiagnosticStatus::Success,
            message,
            details: None,
            duration,
            severity: Severity::Info,
            auto_fixable: false,
        }
    }

    pub fn warning(item_name: String, message: String, duration: Duration, severity: Severity) -> Self {
        Self {
            item_name,
            status: DiagnosticStatus::Warning,
            message,
            details: None,
            duration,
            severity,
            auto_fixable: false,
        }
    }

    pub fn error(item_name: String, message: String, duration: Duration, severity: Severity) -> Self {
        Self {
            item_name,
            status: DiagnosticStatus::Error,
            message,
            details: None,
            duration,
            severity,
            auto_fixable: false,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_auto_fixable(mut self, auto_fixable: bool) -> Self {
        self.auto_fixable = auto_fixable;
        self
    }
}

/// Progress information for diagnostic execution
#[derive(Debug, Clone)]
pub struct DiagnosticProgress {
    pub current_item: String,
    pub completed: usize,
    pub total: usize,
    pub elapsed: Duration,
    pub estimated_remaining: Option<Duration>,
}

impl DiagnosticProgress {
    pub fn new(current_item: String, completed: usize, total: usize, elapsed: Duration) -> Self {
        let estimated_remaining = if completed > 0 {
            let avg_time_per_item = elapsed.as_secs_f64() / completed as f64;
            let remaining_items = total.saturating_sub(completed);
            Some(Duration::from_secs_f64(avg_time_per_item * remaining_items as f64))
        } else {
            None
        };

        Self {
            current_item,
            completed,
            total,
            elapsed,
            estimated_remaining,
        }
    }

    pub fn progress_percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.completed as f64 / self.total as f64) * 100.0
        }
    }
}

/// Trait for diagnostic manager implementations
#[async_trait]
pub trait DiagnosticManager {
    /// Run comprehensive diagnostics for SSM connection
    async fn run_full_diagnostics(&mut self, config: DiagnosticConfig) -> Result<Vec<DiagnosticResult>, Box<dyn std::error::Error>>;
    
    /// Run pre-connection checks
    async fn run_precheck(&mut self, config: DiagnosticConfig) -> Result<Vec<DiagnosticResult>, Box<dyn std::error::Error>>;
    
    /// Run a specific diagnostic item
    async fn run_specific_diagnostic(&mut self, item: &str, config: DiagnosticConfig) -> Result<DiagnosticResult, Box<dyn std::error::Error>>;
    
    /// Run comprehensive AWS configuration validation
    async fn run_aws_config_validation(&mut self, config: DiagnosticConfig) -> Result<AwsConfigValidationResult, Box<dyn std::error::Error>>;
    
    /// Register a progress callback function
    fn register_progress_callback(&mut self, callback: Box<dyn Fn(DiagnosticProgress) + Send + Sync>);
}

/// Default implementation of the diagnostic manager
pub struct DefaultDiagnosticManager {
    progress_callback: Option<std::sync::Arc<std::sync::Mutex<Box<dyn Fn(DiagnosticProgress) + Send + Sync>>>>,
    diagnostic_items: Vec<String>,
    feedback_system: Option<DiagnosticFeedbackSystem>,
    realtime_feedback: Option<std::sync::Arc<RealtimeFeedbackManager>>,
}

impl Clone for DefaultDiagnosticManager {
    fn clone(&self) -> Self {
        Self {
            progress_callback: self.progress_callback.clone(),
            diagnostic_items: self.diagnostic_items.clone(),
            feedback_system: None, // Don't clone feedback system
            realtime_feedback: self.realtime_feedback.clone(),
        }
    }
}

impl DefaultDiagnosticManager {
    pub async fn new() -> anyhow::Result<Self> {
        let diagnostic_items = vec![
            "instance_state".to_string(),
            "ssm_agent_enhanced".to_string(),
            "iam_permissions".to_string(),
            "vpc_endpoints".to_string(),
            "security_groups".to_string(),
            "network_connectivity".to_string(),
            "local_port_availability".to_string(),
        ];

        Ok(Self {
            progress_callback: None,
            diagnostic_items,
            feedback_system: None,
            realtime_feedback: None,
        })
    }

    /// Get the list of available diagnostic items
    pub fn get_diagnostic_items(&self) -> &[String] {
        &self.diagnostic_items
    }

    /// Report progress to registered callback
    fn report_progress(&self, progress: DiagnosticProgress) {
        if let Some(callback) = &self.progress_callback {
            if let Ok(callback_guard) = callback.lock() {
                callback_guard(progress.clone());
            }
        }
        
        // Also report to feedback system if available
        if let Some(feedback_system) = &self.feedback_system {
            let _ = feedback_system.update_progress(progress.clone());
        }

        // Also report to realtime feedback if available
        if let Some(realtime_feedback) = &self.realtime_feedback {
            realtime_feedback.update_progress(progress);
        }
    }

    /// Report diagnostic result to feedback systems
    fn report_result_to_realtime(&self, result: DiagnosticResult) {
        // Report to realtime feedback if available
        if let Some(realtime_feedback) = &self.realtime_feedback {
            realtime_feedback.add_result(result);
        }
    }

    /// Enable real-time feedback display
    pub fn enable_realtime_feedback(&mut self, config: FeedbackConfig) -> anyhow::Result<()> {
        info!("Enabling real-time diagnostic feedback display");
        let feedback_manager = std::sync::Arc::new(RealtimeFeedbackManager::new(config));
        
        // Register callbacks
        let progress_callback = create_progress_callback(feedback_manager.clone());
        
        self.progress_callback = Some(std::sync::Arc::new(std::sync::Mutex::new(progress_callback)));
        self.realtime_feedback = Some(feedback_manager);
        
        info!("Real-time feedback system enabled");
        Ok(())
    }

    /// Start real-time feedback display (should be called in a separate task)
    pub async fn start_realtime_feedback_display(&mut self) -> Result<(), anyhow::Error> {
        if let Some(feedback_manager) = &mut self.realtime_feedback {
            // Clone the Arc to get a mutable reference
            let manager = feedback_manager.clone();
            let mut manager_mut = std::sync::Arc::try_unwrap(manager)
                .map_err(|_| anyhow::anyhow!("Cannot get exclusive access to feedback manager"))?;
            manager_mut.start_feedback_display().await?;
        }
        Ok(())
    }

    /// Check if there are critical issues that require user attention
    pub fn has_critical_issues(&self) -> bool {
        if let Some(feedback_manager) = &self.realtime_feedback {
            feedback_manager.has_critical_issues()
        } else {
            false
        }
    }

    /// Get current feedback status
    pub fn get_feedback_status(&self) -> Option<FeedbackStatus> {
        self.realtime_feedback.as_ref().map(|fm| fm.get_status())
    }

    /// Get critical issues
    pub fn get_critical_issues(&self) -> Vec<DiagnosticResult> {
        if let Some(feedback_manager) = &self.realtime_feedback {
            feedback_manager.get_critical_issues()
        } else {
            Vec::new()
        }
    }

    /// Stop real-time feedback display
    pub fn stop_realtime_feedback(&self) {
        if let Some(feedback_manager) = &self.realtime_feedback {
            feedback_manager.stop();
        }
    }

    /// Enable real-time feedback system
    pub fn enable_feedback_system(&mut self) -> anyhow::Result<()> {
        info!("Enabling real-time diagnostic feedback system");
        let mut feedback_system = DiagnosticFeedbackSystem::new()?;
        feedback_system.initialize_terminal()?;
        self.feedback_system = Some(feedback_system);
        Ok(())
    }

    /// Disable real-time feedback system
    pub fn disable_feedback_system(&mut self) -> anyhow::Result<()> {
        info!("Disabling real-time diagnostic feedback system");
        if let Some(mut feedback_system) = self.feedback_system.take() {
            feedback_system.stop()?;
        }
        Ok(())
    }

    /// Start feedback system with configuration
    pub async fn start_feedback_system(&mut self, config: FeedbackDisplayConfig) -> anyhow::Result<()> {
        if let Some(feedback_system) = &mut self.feedback_system {
            info!("Starting real-time diagnostic feedback system");
            
            // Start the feedback system in a separate task
            let mut feedback_clone = DiagnosticFeedbackSystem::new()?;
            feedback_clone.initialize_terminal()?;
            
            tokio::spawn(async move {
                if let Err(e) = feedback_clone.start(config).await {
                    error!("Feedback system error: {}", e);
                }
            });
        }
        Ok(())
    }

    /// Check if feedback system is enabled
    pub fn is_feedback_enabled(&self) -> bool {
        self.feedback_system.is_some()
    }

    /// Get feedback system reference
    pub fn get_feedback_system(&self) -> Option<&DiagnosticFeedbackSystem> {
        self.feedback_system.as_ref()
    }

    /// Pause diagnostic execution (if feedback system is enabled)
    pub fn pause_diagnostics(&self) -> anyhow::Result<()> {
        if let Some(feedback_system) = &self.feedback_system {
            feedback_system.pause()?;
        }
        Ok(())
    }

    /// Resume diagnostic execution (if feedback system is enabled)
    pub fn resume_diagnostics(&self) -> anyhow::Result<()> {
        if let Some(feedback_system) = &self.feedback_system {
            feedback_system.resume()?;
        }
        Ok(())
    }

    /// Add diagnostic result to feedback system
    fn report_result(&self, result: &DiagnosticResult) {
        if let Some(feedback_system) = &self.feedback_system {
            let _ = feedback_system.add_result(result.clone());
            
            // Handle critical issues
            if result.severity == Severity::Critical {
                let _ = feedback_system.request_confirmation(
                    result.clone(),
                    "重大な問題が検出されました。診断を続行しますか？".to_string(),
                    vec!["続行".to_string(), "中止".to_string()],
                );
            }
        }
        
        // Also report to realtime feedback
        self.report_result_to_realtime(result.clone());
    }

    /// Execute a single diagnostic item with timing
    async fn execute_diagnostic_item(&self, item: &str, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        debug!("Starting diagnostic item: {}", item);

        let result = match item {
            "instance_state" => self.check_instance_state(config).await,
            "ssm_agent_enhanced" => self.check_ssm_agent(config).await,
            "iam_permissions" => self.check_iam_permissions(config).await,
            "vpc_endpoints" => self.check_vpc_endpoints(config).await,
            "security_groups" => self.check_security_groups(config).await,
            "network_connectivity" => self.check_network_connectivity(config).await,
            "local_port_availability" => self.check_local_port_availability(config).await,
            _ => {
                warn!("Unknown diagnostic item: {}", item);
                DiagnosticResult::error(
                    item.to_string(),
                    format!("Unknown diagnostic item: {}", item),
                    start_time.elapsed(),
                    Severity::Low,
                )
            }
        };

        debug!("Completed diagnostic item: {} in {:?}", item, result.duration);
        
        // Report result to feedback system
        self.report_result(&result);
        
        result
    }

    /// Check EC2 instance state
    async fn check_instance_state(&self, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        
        info!("Checking instance state for: {}", config.instance_id);
        
        match DefaultInstanceDiagnostics::with_default_aws().await {
            Ok(instance_diagnostics) => {
                match instance_diagnostics.check_instance_state(&config.instance_id).await {
                    Ok(result) => result,
                    Err(e) => {
                        error!("Instance state check failed: {}", e);
                        DiagnosticResult::error(
                            "instance_state".to_string(),
                            format!("Instance state check failed: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        )
                    }
                }
            }
            Err(e) => {
                error!("Failed to create instance diagnostics: {}", e);
                DiagnosticResult::error(
                    "instance_state".to_string(),
                    format!("Failed to create instance diagnostics: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                )
            }
        }
    }

    /// Check SSM agent status with enhanced diagnostics
    async fn check_ssm_agent(&self, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        
        info!("Running enhanced SSM agent diagnostics for: {}", config.instance_id);
        
        match DefaultSsmAgentDiagnostics::with_default_aws().await {
            Ok(ssm_diagnostics) => {
                // Run comprehensive SSM agent diagnostics including enhanced features for Task 25.2
                match ssm_diagnostics.run_ssm_agent_diagnostics(&config.instance_id).await {
                    Ok(results) => {
                        // Combine all results into a single comprehensive result
                        let mut success_count = 0;
                        let mut warning_count = 0;
                        let mut error_count = 0;
                        let mut all_details = serde_json::Map::new();
                        let mut all_messages = Vec::new();
                        let mut highest_severity = Severity::Info;
                        
                        for result in &results {
                            match result.status {
                                DiagnosticStatus::Success => success_count += 1,
                                DiagnosticStatus::Warning => warning_count += 1,
                                DiagnosticStatus::Error => error_count += 1,
                                DiagnosticStatus::Skipped => {},
                            }
                            
                            // Track highest severity
                            if result.severity > highest_severity {
                                highest_severity = result.severity.clone();
                            }
                            
                            // Collect messages
                            all_messages.push(format!("{}: {}", result.item_name, result.message));
                            
                            // Collect details
                            if let Some(details) = &result.details {
                                all_details.insert(result.item_name.clone(), details.clone());
                            }
                        }
                        
                        let total_checks = results.len();
                        let summary_message = format!(
                            "Enhanced SSM Agent diagnostics completed: {} checks ({} success, {} warnings, {} errors)",
                            total_checks, success_count, warning_count, error_count
                        );
                        
                        // Determine overall status based on results
                        let overall_status = if error_count > 0 {
                            DiagnosticStatus::Error
                        } else if warning_count > 0 {
                            DiagnosticStatus::Warning
                        } else {
                            DiagnosticStatus::Success
                        };
                        
                        let mut result = DiagnosticResult {
                            item_name: "ssm_agent_enhanced".to_string(),
                            status: overall_status,
                            message: summary_message,
                            details: Some(serde_json::Value::Object(all_details)),
                            duration: start_time.elapsed(),
                            severity: highest_severity,
                            auto_fixable: error_count > 0 || warning_count > 0,
                        };
                        
                        // Add detailed messages as additional context
                        if let Some(details) = result.details.as_mut() {
                            if let Some(details_obj) = details.as_object_mut() {
                                details_obj.insert(
                                    "detailed_messages".to_string(),
                                    serde_json::Value::Array(
                                        all_messages.into_iter()
                                            .map(serde_json::Value::String)
                                            .collect()
                                    )
                                );
                            }
                        }
                        
                        result
                    }
                    Err(e) => {
                        error!("Enhanced SSM agent diagnostics failed: {}", e);
                        DiagnosticResult::error(
                            "ssm_agent_enhanced".to_string(),
                            format!("Enhanced SSM agent diagnostics failed: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        )
                    }
                }
            }
            Err(e) => {
                error!("Failed to create SSM agent diagnostics: {}", e);
                DiagnosticResult::error(
                    "ssm_agent_enhanced".to_string(),
                    format!("Failed to create SSM agent diagnostics: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                )
            }
        }
    }

    /// Check IAM permissions
    async fn check_iam_permissions(&self, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        
        info!("Checking IAM permissions for: {}", config.instance_id);
        
        match DefaultIamDiagnostics::with_default_aws().await {
            Ok(iam_diagnostics) => {
                // Run comprehensive IAM diagnostics including enhanced Task 25.3 features
                match iam_diagnostics.diagnose_iam_configuration(&config.instance_id).await {
                    Ok(results) => {
                        // Combine all results into a single comprehensive result
                        let mut success_count = 0;
                        let mut warning_count = 0;
                        let mut error_count = 0;
                        let mut all_details = serde_json::Map::new();
                        let mut all_messages = Vec::new();
                        let mut highest_severity = Severity::Info;
                        
                        for result in &results {
                            match result.status {
                                DiagnosticStatus::Success => success_count += 1,
                                DiagnosticStatus::Warning => warning_count += 1,
                                DiagnosticStatus::Error => error_count += 1,
                                DiagnosticStatus::Skipped => {},
                            }
                            
                            // Track highest severity
                            if result.severity > highest_severity {
                                highest_severity = result.severity.clone();
                            }
                            
                            // Collect messages
                            all_messages.push(format!("{}: {}", result.item_name, result.message));
                            
                            // Collect details
                            if let Some(details) = &result.details {
                                all_details.insert(result.item_name.clone(), details.clone());
                            }
                        }
                        
                        let total_checks = results.len();
                        let summary_message = format!(
                            "Comprehensive IAM diagnostics completed: {} checks ({} success, {} warnings, {} errors)",
                            total_checks, success_count, warning_count, error_count
                        );
                        
                        // Determine overall status based on results
                        let overall_status = if error_count > 0 {
                            DiagnosticStatus::Error
                        } else if warning_count > 0 {
                            DiagnosticStatus::Warning
                        } else {
                            DiagnosticStatus::Success
                        };
                        
                        let mut result = DiagnosticResult {
                            item_name: "iam_permissions".to_string(),
                            status: overall_status,
                            message: summary_message,
                            details: Some(serde_json::Value::Object(all_details)),
                            duration: start_time.elapsed(),
                            severity: highest_severity,
                            auto_fixable: error_count > 0 || warning_count > 0,
                        };
                        
                        // Add detailed messages as additional context
                        if let Some(details) = result.details.as_mut() {
                            if let Some(details_obj) = details.as_object_mut() {
                                details_obj.insert(
                                    "detailed_messages".to_string(),
                                    serde_json::Value::Array(
                                        all_messages.into_iter()
                                            .map(serde_json::Value::String)
                                            .collect()
                                    )
                                );
                            }
                        }
                        
                        result
                    }
                    Err(e) => {
                        error!("Comprehensive IAM diagnostics failed: {}", e);
                        DiagnosticResult::error(
                            "iam_permissions".to_string(),
                            format!("Comprehensive IAM diagnostics failed: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        )
                    }
                }
            }
            Err(e) => {
                error!("Failed to create IAM diagnostics: {}", e);
                DiagnosticResult::error(
                    "iam_permissions".to_string(),
                    format!("Failed to create IAM diagnostics: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                )
            }
        }
    }

    /// Check VPC endpoints
    async fn check_vpc_endpoints(&self, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        
        info!("Checking VPC endpoints for: {}", config.instance_id);
        
        match DefaultNetworkDiagnostics::with_default_aws().await {
            Ok(network_diagnostics) => {
                // Run comprehensive VPC endpoint analysis including enhanced Task 25.3 features
                match network_diagnostics.detailed_vpc_endpoint_analysis(&config.instance_id).await {
                    Ok(results) => {
                        // Combine all results into a single comprehensive result
                        let mut success_count = 0;
                        let mut warning_count = 0;
                        let mut error_count = 0;
                        let mut all_details = serde_json::Map::new();
                        let mut all_messages = Vec::new();
                        let mut highest_severity = Severity::Info;
                        
                        for result in &results {
                            match result.status {
                                DiagnosticStatus::Success => success_count += 1,
                                DiagnosticStatus::Warning => warning_count += 1,
                                DiagnosticStatus::Error => error_count += 1,
                                DiagnosticStatus::Skipped => {},
                            }
                            
                            // Track highest severity
                            if result.severity > highest_severity {
                                highest_severity = result.severity.clone();
                            }
                            
                            // Collect messages
                            all_messages.push(format!("{}: {}", result.item_name, result.message));
                            
                            // Collect details
                            if let Some(details) = &result.details {
                                all_details.insert(result.item_name.clone(), details.clone());
                            }
                        }
                        
                        let total_checks = results.len();
                        let summary_message = if total_checks > 0 {
                            format!(
                                "Detailed VPC endpoint analysis completed: {} endpoints ({} success, {} warnings, {} errors)",
                                total_checks, success_count, warning_count, error_count
                            )
                        } else {
                            "No VPC endpoints found - SSM will use internet gateway if available".to_string()
                        };
                        
                        // Determine overall status based on results
                        let overall_status = if error_count > 0 {
                            DiagnosticStatus::Error
                        } else if warning_count > 0 {
                            DiagnosticStatus::Warning
                        } else if total_checks > 0 {
                            DiagnosticStatus::Success
                        } else {
                            DiagnosticStatus::Warning // No endpoints found
                        };
                        
                        let severity = if total_checks == 0 {
                            Severity::Medium // No VPC endpoints is a medium concern
                        } else {
                            highest_severity
                        };
                        
                        let mut result = DiagnosticResult {
                            item_name: "vpc_endpoints".to_string(),
                            status: overall_status,
                            message: summary_message,
                            details: Some(serde_json::Value::Object(all_details)),
                            duration: start_time.elapsed(),
                            severity,
                            auto_fixable: error_count > 0 || warning_count > 0,
                        };
                        
                        // Add detailed messages as additional context
                        if let Some(details) = result.details.as_mut() {
                            if let Some(details_obj) = details.as_object_mut() {
                                details_obj.insert(
                                    "detailed_messages".to_string(),
                                    serde_json::Value::Array(
                                        all_messages.into_iter()
                                            .map(serde_json::Value::String)
                                            .collect()
                                    )
                                );
                            }
                        }
                        
                        result
                    }
                    Err(e) => {
                        error!("Detailed VPC endpoint analysis failed: {}", e);
                        DiagnosticResult::error(
                            "vpc_endpoints".to_string(),
                            format!("Detailed VPC endpoint analysis failed: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        )
                    }
                }
            }
            Err(e) => {
                error!("Failed to create network diagnostics: {}", e);
                DiagnosticResult::error(
                    "vpc_endpoints".to_string(),
                    format!("Failed to create network diagnostics: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                )
            }
        }
    }

    /// Check security groups
    async fn check_security_groups(&self, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        
        info!("Checking security groups for: {}", config.instance_id);
        
        match DefaultNetworkDiagnostics::with_default_aws().await {
            Ok(network_diagnostics) => {
                // Run comprehensive security group analysis including enhanced Task 25.3 features
                match network_diagnostics.detailed_security_group_analysis(&config.instance_id).await {
                    Ok(results) => {
                        // Combine all results into a single comprehensive result
                        let mut success_count = 0;
                        let mut warning_count = 0;
                        let mut error_count = 0;
                        let mut all_details = serde_json::Map::new();
                        let mut all_messages = Vec::new();
                        let mut highest_severity = Severity::Info;
                        
                        for result in &results {
                            match result.status {
                                DiagnosticStatus::Success => success_count += 1,
                                DiagnosticStatus::Warning => warning_count += 1,
                                DiagnosticStatus::Error => error_count += 1,
                                DiagnosticStatus::Skipped => {},
                            }
                            
                            // Track highest severity
                            if result.severity > highest_severity {
                                highest_severity = result.severity.clone();
                            }
                            
                            // Collect messages
                            all_messages.push(format!("{}: {}", result.item_name, result.message));
                            
                            // Collect details
                            if let Some(details) = &result.details {
                                all_details.insert(result.item_name.clone(), details.clone());
                            }
                        }
                        
                        let total_checks = results.len();
                        let summary_message = format!(
                            "Detailed security group analysis completed: {} groups ({} success, {} warnings, {} errors)",
                            total_checks, success_count, warning_count, error_count
                        );
                        
                        // Determine overall status based on results
                        let overall_status = if error_count > 0 {
                            DiagnosticStatus::Error
                        } else if warning_count > 0 {
                            DiagnosticStatus::Warning
                        } else {
                            DiagnosticStatus::Success
                        };
                        
                        let mut result = DiagnosticResult {
                            item_name: "security_groups".to_string(),
                            status: overall_status,
                            message: summary_message,
                            details: Some(serde_json::Value::Object(all_details)),
                            duration: start_time.elapsed(),
                            severity: highest_severity,
                            auto_fixable: error_count > 0 || warning_count > 0,
                        };
                        
                        // Add detailed messages as additional context
                        if let Some(details) = result.details.as_mut() {
                            if let Some(details_obj) = details.as_object_mut() {
                                details_obj.insert(
                                    "detailed_messages".to_string(),
                                    serde_json::Value::Array(
                                        all_messages.into_iter()
                                            .map(serde_json::Value::String)
                                            .collect()
                                    )
                                );
                            }
                        }
                        
                        result
                    }
                    Err(e) => {
                        error!("Detailed security group analysis failed: {}", e);
                        DiagnosticResult::error(
                            "security_groups".to_string(),
                            format!("Detailed security group analysis failed: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        )
                    }
                }
            }
            Err(e) => {
                error!("Failed to create network diagnostics: {}", e);
                DiagnosticResult::error(
                    "security_groups".to_string(),
                    format!("Failed to create network diagnostics: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                )
            }
        }
    }

    /// Check network connectivity
    async fn check_network_connectivity(&self, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        
        info!("Checking network connectivity for: {}", config.instance_id);
        
        match DefaultNetworkDiagnostics::with_default_aws().await {
            Ok(network_diagnostics) => {
                match network_diagnostics.test_network_connectivity(&config.instance_id).await {
                    Ok(result) => result,
                    Err(e) => {
                        error!("Network connectivity check failed: {}", e);
                        DiagnosticResult::error(
                            "network_connectivity".to_string(),
                            format!("Network connectivity check failed: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        )
                    }
                }
            }
            Err(e) => {
                error!("Failed to create network diagnostics: {}", e);
                DiagnosticResult::error(
                    "network_connectivity".to_string(),
                    format!("Failed to create network diagnostics: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                )
            }
        }
    }

    /// Check local port availability
    async fn check_local_port_availability(&self, config: &DiagnosticConfig) -> DiagnosticResult {
        let start_time = Instant::now();
        
        if let Some(port) = config.local_port {
            info!("Checking local port availability: {}", port);
            
            let port_diagnostics = DefaultPortDiagnostics::new();
            port_diagnostics.diagnose_port(port).await
        } else {
            info!("Skipping local port check - no port specified");
            DiagnosticResult::success(
                "local_port_availability".to_string(),
                "No local port specified - skipping port availability check".to_string(),
                start_time.elapsed(),
            )
        }
    }
}

#[async_trait]
impl DiagnosticManager for DefaultDiagnosticManager {
    async fn run_full_diagnostics(&mut self, config: DiagnosticConfig) -> Result<Vec<DiagnosticResult>, Box<dyn std::error::Error>> {
        info!("Starting full diagnostics for instance: {}", config.instance_id);
        let start_time = Instant::now();
        
        let total_items = self.diagnostic_items.len();
        let mut results = Vec::with_capacity(total_items);
        
        if config.parallel_execution {
            // Run diagnostics in parallel
            info!("Running diagnostics in parallel mode");
            
            let mut tasks = Vec::new();
            for (index, item) in self.diagnostic_items.iter().enumerate() {
                let item_clone = item.clone();
                let config_clone = config.clone();
                
                // Create a future for each diagnostic item
                let task = async move {
                    let start_time = Instant::now();
                    debug!("Starting diagnostic item: {}", item_clone);

                    let result = match item_clone.as_str() {
                        "instance_state" => {
                            info!("Checking instance state for: {}", config_clone.instance_id);
                            
                            match DefaultInstanceDiagnostics::with_default_aws().await {
                                Ok(temp_instance_diagnostics) => {
                                    match temp_instance_diagnostics.check_instance_state(&config_clone.instance_id).await {
                                        Ok(result) => result,
                                        Err(e) => {
                                            error!("Instance state check failed: {}", e);
                                            DiagnosticResult::error(
                                                "instance_state".to_string(),
                                                format!("Instance state check failed: {}", e),
                                                start_time.elapsed(),
                                                Severity::High,
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to create instance diagnostics: {}", e);
                                    DiagnosticResult::error(
                                        "instance_state".to_string(),
                                        format!("Failed to create instance diagnostics: {}", e),
                                        start_time.elapsed(),
                                        Severity::High,
                                    )
                                }
                            }
                        },
                        "ssm_agent_enhanced" => {
                            info!("Running enhanced SSM agent diagnostics for: {}", config_clone.instance_id);
                            
                            match DefaultSsmAgentDiagnostics::with_default_aws().await {
                                Ok(temp_ssm_diagnostics) => {
                                    match temp_ssm_diagnostics.run_ssm_agent_diagnostics(&config_clone.instance_id).await {
                                        Ok(results) => {
                                            // Combine all results into a single comprehensive result
                                            let mut success_count = 0;
                                            let mut warning_count = 0;
                                            let mut error_count = 0;
                                            let mut all_details = serde_json::Map::new();
                                            let mut all_messages = Vec::new();
                                            let mut highest_severity = Severity::Info;
                                            
                                            for result in &results {
                                                match result.status {
                                                    DiagnosticStatus::Success => success_count += 1,
                                                    DiagnosticStatus::Warning => warning_count += 1,
                                                    DiagnosticStatus::Error => error_count += 1,
                                                    DiagnosticStatus::Skipped => {},
                                                }
                                                
                                                // Track highest severity
                                                if result.severity > highest_severity {
                                                    highest_severity = result.severity.clone();
                                                }
                                                
                                                // Collect messages
                                                all_messages.push(format!("{}: {}", result.item_name, result.message));
                                                
                                                // Collect details
                                                if let Some(details) = &result.details {
                                                    all_details.insert(result.item_name.clone(), details.clone());
                                                }
                                            }
                                            
                                            let total_checks = results.len();
                                            let summary_message = format!(
                                                "Enhanced SSM Agent diagnostics completed: {} checks ({} success, {} warnings, {} errors)",
                                                total_checks, success_count, warning_count, error_count
                                            );
                                            
                                            // Determine overall status based on results
                                            let overall_status = if error_count > 0 {
                                                DiagnosticStatus::Error
                                            } else if warning_count > 0 {
                                                DiagnosticStatus::Warning
                                            } else {
                                                DiagnosticStatus::Success
                                            };
                                            
                                            let mut result = DiagnosticResult {
                                                item_name: "ssm_agent_enhanced".to_string(),
                                                status: overall_status,
                                                message: summary_message,
                                                details: Some(serde_json::Value::Object(all_details)),
                                                duration: start_time.elapsed(),
                                                severity: highest_severity,
                                                auto_fixable: error_count > 0 || warning_count > 0,
                                            };
                                            
                                            // Add detailed messages as additional context
                                            if let Some(details) = result.details.as_mut() {
                                                if let Some(details_obj) = details.as_object_mut() {
                                                    details_obj.insert(
                                                        "detailed_messages".to_string(),
                                                        serde_json::Value::Array(
                                                            all_messages.into_iter()
                                                                .map(serde_json::Value::String)
                                                                .collect()
                                                        )
                                                    );
                                                }
                                            }
                                            
                                            result
                                        }
                                        Err(e) => {
                                            error!("Enhanced SSM agent diagnostics failed: {}", e);
                                            DiagnosticResult::error(
                                                "ssm_agent_enhanced".to_string(),
                                                format!("Enhanced SSM agent diagnostics failed: {}", e),
                                                start_time.elapsed(),
                                                Severity::High,
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to create SSM agent diagnostics: {}", e);
                                    DiagnosticResult::error(
                                        "ssm_agent_enhanced".to_string(),
                                        format!("Failed to create SSM agent diagnostics: {}", e),
                                        start_time.elapsed(),
                                        Severity::High,
                                    )
                                }
                            }
                        },
                        "iam_permissions" => {
                            info!("Checking IAM permissions for: {}", config_clone.instance_id);
                            
                            match DefaultIamDiagnostics::with_default_aws().await {
                                Ok(iam_diagnostics) => {
                                    match iam_diagnostics.diagnose_iam_configuration(&config_clone.instance_id).await {
                                        Ok(results) => {
                                            // Aggregate results into a single diagnostic result
                                            let mut has_errors = false;
                                            let mut has_warnings = false;
                                            let mut messages = Vec::new();
                                            
                                            for result in &results {
                                                match result.status {
                                                    DiagnosticStatus::Error => {
                                                        has_errors = true;
                                                        messages.push(format!("❌ {}: {}", result.item_name, result.message));
                                                    }
                                                    DiagnosticStatus::Warning => {
                                                        has_warnings = true;
                                                        messages.push(format!("⚠️ {}: {}", result.item_name, result.message));
                                                    }
                                                    DiagnosticStatus::Success => {
                                                        messages.push(format!("✅ {}: {}", result.item_name, result.message));
                                                    }
                                                    DiagnosticStatus::Skipped => {
                                                        messages.push(format!("⏭️ {}: {}", result.item_name, result.message));
                                                    }
                                                }
                                            }
                                            
                                            let combined_message = messages.join("\n");
                                            let details = serde_json::json!({
                                                "individual_results": results,
                                                "summary": {
                                                    "total_checks": results.len(),
                                                    "errors": results.iter().filter(|r| r.status == DiagnosticStatus::Error).count(),
                                                    "warnings": results.iter().filter(|r| r.status == DiagnosticStatus::Warning).count(),
                                                    "successes": results.iter().filter(|r| r.status == DiagnosticStatus::Success).count(),
                                                }
                                            });
                                            
                                            if has_errors {
                                                DiagnosticResult::error(
                                                    "iam_permissions".to_string(),
                                                    format!("IAM configuration issues detected:\n{}", combined_message),
                                                    start_time.elapsed(),
                                                    Severity::Critical,
                                                ).with_details(details)
                                            } else if has_warnings {
                                                DiagnosticResult::warning(
                                                    "iam_permissions".to_string(),
                                                    format!("IAM configuration warnings:\n{}", combined_message),
                                                    start_time.elapsed(),
                                                    Severity::Medium,
                                                ).with_details(details)
                                            } else {
                                                DiagnosticResult::success(
                                                    "iam_permissions".to_string(),
                                                    format!("IAM configuration verified:\n{}", combined_message),
                                                    start_time.elapsed(),
                                                ).with_details(details)
                                            }
                                        }
                                        Err(e) => {
                                            error!("IAM diagnostics failed: {}", e);
                                            DiagnosticResult::error(
                                                "iam_permissions".to_string(),
                                                format!("IAM diagnostics failed: {}", e),
                                                start_time.elapsed(),
                                                Severity::High,
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to create IAM diagnostics: {}", e);
                                    DiagnosticResult::error(
                                        "iam_permissions".to_string(),
                                        format!("Failed to create IAM diagnostics: {}", e),
                                        start_time.elapsed(),
                                        Severity::High,
                                    )
                                }
                            }
                        },
                        "vpc_endpoints" => {
                            info!("Checking VPC endpoints for: {}", config_clone.instance_id);
                            
                            match DefaultNetworkDiagnostics::with_default_aws().await {
                                Ok(network_diagnostics) => {
                                    match network_diagnostics.check_vpc_endpoints(&config_clone.instance_id).await {
                                        Ok(result) => result,
                                        Err(e) => {
                                            error!("VPC endpoints check failed: {}", e);
                                            DiagnosticResult::error(
                                                "vpc_endpoints".to_string(),
                                                format!("VPC endpoints check failed: {}", e),
                                                start_time.elapsed(),
                                                Severity::High,
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to create network diagnostics: {}", e);
                                    DiagnosticResult::error(
                                        "vpc_endpoints".to_string(),
                                        format!("Failed to create network diagnostics: {}", e),
                                        start_time.elapsed(),
                                        Severity::High,
                                    )
                                }
                            }
                        },
                        "security_groups" => {
                            info!("Checking security groups for: {}", config_clone.instance_id);
                            
                            match DefaultNetworkDiagnostics::with_default_aws().await {
                                Ok(network_diagnostics) => {
                                    match network_diagnostics.check_security_group_rules(&config_clone.instance_id).await {
                                        Ok(result) => result,
                                        Err(e) => {
                                            error!("Security groups check failed: {}", e);
                                            DiagnosticResult::error(
                                                "security_groups".to_string(),
                                                format!("Security groups check failed: {}", e),
                                                start_time.elapsed(),
                                                Severity::High,
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to create network diagnostics: {}", e);
                                    DiagnosticResult::error(
                                        "security_groups".to_string(),
                                        format!("Failed to create network diagnostics: {}", e),
                                        start_time.elapsed(),
                                        Severity::High,
                                    )
                                }
                            }
                        },
                        "network_connectivity" => {
                            info!("Checking network connectivity for: {}", config_clone.instance_id);
                            
                            match DefaultNetworkDiagnostics::with_default_aws().await {
                                Ok(network_diagnostics) => {
                                    match network_diagnostics.test_network_connectivity(&config_clone.instance_id).await {
                                        Ok(result) => result,
                                        Err(e) => {
                                            error!("Network connectivity check failed: {}", e);
                                            DiagnosticResult::error(
                                                "network_connectivity".to_string(),
                                                format!("Network connectivity check failed: {}", e),
                                                start_time.elapsed(),
                                                Severity::High,
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to create network diagnostics: {}", e);
                                    DiagnosticResult::error(
                                        "network_connectivity".to_string(),
                                        format!("Failed to create network diagnostics: {}", e),
                                        start_time.elapsed(),
                                        Severity::High,
                                    )
                                }
                            }
                        },
                        "local_port_availability" => {
                            if let Some(port) = config_clone.local_port {
                                info!("Checking local port availability: {}", port);
                                
                                let port_diagnostics = DefaultPortDiagnostics::new();
                                port_diagnostics.diagnose_port(port).await
                            } else {
                                info!("Skipping local port check - no port specified");
                                DiagnosticResult::success(
                                    "local_port_availability".to_string(),
                                    "No local port specified - skipping port availability check".to_string(),
                                    start_time.elapsed(),
                                )
                            }
                        },
                        _ => {
                            warn!("Unknown diagnostic item: {}", item_clone);
                            DiagnosticResult::error(
                                item_clone.clone(),
                                format!("Unknown diagnostic item: {}", item_clone),
                                start_time.elapsed(),
                                Severity::Low,
                            )
                        }
                    };

                    debug!("Completed diagnostic item: {} in {:?}", item_clone, result.duration);
                    (index, result)
                };
                
                tasks.push(task);
            }
            
            // Execute all tasks concurrently
            let task_results = futures::future::join_all(tasks).await;
            
            // Sort by index to maintain order
            let mut indexed_results: Vec<_> = task_results.into_iter().collect();
            indexed_results.sort_by_key(|(index, _)| *index);
            results = indexed_results.into_iter().map(|(_, result)| {
                // Report each result to feedback system
                self.report_result(&result);
                result
            }).collect();
            
        } else {
            // Run diagnostics sequentially
            info!("Running diagnostics in sequential mode");
            
            for (completed, item) in self.diagnostic_items.iter().enumerate() {
                // Report progress
                let progress = DiagnosticProgress::new(
                    item.clone(),
                    completed,
                    total_items,
                    start_time.elapsed(),
                );
                self.report_progress(progress);
                
                // Execute diagnostic
                let result = self.execute_diagnostic_item(item, &config).await;
                results.push(result);
            }
        }
        
        // Report final progress
        let final_progress = DiagnosticProgress::new(
            "Completed".to_string(),
            total_items,
            total_items,
            start_time.elapsed(),
        );
        self.report_progress(final_progress);
        
        info!("Full diagnostics completed in {:?}", start_time.elapsed());
        Ok(results)
    }

    async fn run_precheck(&mut self, config: DiagnosticConfig) -> Result<Vec<DiagnosticResult>, Box<dyn std::error::Error>> {
        info!("Starting precheck diagnostics for instance: {}", config.instance_id);
        
        // For precheck, run only essential items
        let precheck_items = vec![
            "instance_state",
            "local_port_availability",
            "iam_permissions",
        ];
        
        let start_time = Instant::now();
        let mut results = Vec::new();
        
        for (completed, item) in precheck_items.iter().enumerate() {
            // Report progress
            let progress = DiagnosticProgress::new(
                item.to_string(),
                completed,
                precheck_items.len(),
                start_time.elapsed(),
            );
            self.report_progress(progress);
            
            // Execute diagnostic
            let result = self.execute_diagnostic_item(item, &config).await;
            results.push(result);
        }
        
        // Report final progress
        let final_progress = DiagnosticProgress::new(
            "Precheck Completed".to_string(),
            precheck_items.len(),
            precheck_items.len(),
            start_time.elapsed(),
        );
        self.report_progress(final_progress);
        
        info!("Precheck diagnostics completed in {:?}", start_time.elapsed());
        Ok(results)
    }

    async fn run_specific_diagnostic(&mut self, item: &str, config: DiagnosticConfig) -> Result<DiagnosticResult, Box<dyn std::error::Error>> {
        info!("Running specific diagnostic: {} for instance: {}", item, config.instance_id);
        
        if !self.diagnostic_items.contains(&item.to_string()) {
            return Err(format!("Unknown diagnostic item: {}", item).into());
        }
        
        let result = self.execute_diagnostic_item(item, &config).await;
        
        info!("Specific diagnostic {} completed in {:?}", item, result.duration);
        Ok(result)
    }

    fn register_progress_callback(&mut self, callback: Box<dyn Fn(DiagnosticProgress) + Send + Sync>) {
        info!("Registering progress callback");
        self.progress_callback = Some(std::sync::Arc::new(std::sync::Mutex::new(callback)));
    }

    async fn run_aws_config_validation(&mut self, config: DiagnosticConfig) -> Result<AwsConfigValidationResult, Box<dyn std::error::Error>> {
        info!("Starting integrated AWS configuration validation for instance: {}", config.instance_id);
        
        // Create AWS configuration validation config from diagnostic config
        let validation_config = AwsConfigValidationConfig::new(config.instance_id.clone())
            .with_aws_config(config.region.clone(), config.profile.clone())
            .with_checks(true, true, true, true) // Enable all checks
            .with_minimum_compliance_score(75.0); // Default minimum score

        // Create AWS config validator
        let validator = if let (Some(region), Some(profile)) = (&config.region, &config.profile) {
            DefaultAwsConfigValidator::with_aws_config(Some(region.clone()), Some(profile.clone())).await?
        } else {
            DefaultAwsConfigValidator::new().await?
        };

        // Run integrated AWS configuration validation with enhanced cross-validation
        let validation_result = validator.validate_integrated_aws_configuration(validation_config).await?;

        info!("Integrated AWS configuration validation completed with compliance score: {:.1}%", 
              validation_result.overall_compliance_score);

        Ok(validation_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_config_creation() {
        let config = DiagnosticConfig::new("i-1234567890abcdef0".to_string())
            .with_ports(8080, 80)
            .with_aws_config(Some("us-east-1".to_string()), Some("default".to_string()))
            .with_timeout(Duration::from_secs(60))
            .with_parallel_execution(false);

        assert_eq!(config.instance_id, "i-1234567890abcdef0");
        assert_eq!(config.local_port, Some(8080));
        assert_eq!(config.remote_port, Some(80));
        assert_eq!(config.region, Some("us-east-1".to_string()));
        assert_eq!(config.profile, Some("default".to_string()));
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(!config.parallel_execution);
    }

    #[test]
    fn test_diagnostic_result_creation() {
        let result = DiagnosticResult::success(
            "test_item".to_string(),
            "Test successful".to_string(),
            Duration::from_millis(100),
        );

        assert_eq!(result.item_name, "test_item");
        assert_eq!(result.status, DiagnosticStatus::Success);
        assert_eq!(result.message, "Test successful");
        assert_eq!(result.severity, Severity::Info);
        assert!(!result.auto_fixable);
    }

    #[test]
    fn test_diagnostic_progress() {
        let progress = DiagnosticProgress::new(
            "test_item".to_string(),
            3,
            10,
            Duration::from_secs(30),
        );

        assert_eq!(progress.current_item, "test_item");
        assert_eq!(progress.completed, 3);
        assert_eq!(progress.total, 10);
        assert_eq!(progress.progress_percentage(), 30.0);
        assert!(progress.estimated_remaining.is_some());
    }

    #[tokio::test]
    async fn test_diagnostic_manager_creation() {
        let manager = DefaultDiagnosticManager::new().await.expect("Failed to create diagnostic manager");
        let items = manager.get_diagnostic_items();
        
        assert!(!items.is_empty());
        assert!(items.contains(&"instance_state".to_string()));
        assert!(items.contains(&"ssm_agent_enhanced".to_string()));
        assert!(items.contains(&"iam_permissions".to_string()));
    }
}