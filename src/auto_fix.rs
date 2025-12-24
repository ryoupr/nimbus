use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, error, debug};
use async_trait::async_trait;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_ssm::Client as SsmClient;
use aws_config::{BehaviorVersion, Region};
use std::process::Command;

use crate::diagnostic::{DiagnosticResult, DiagnosticStatus};
use crate::error::{Ec2ConnectError, Result, AwsError};

/// SSM Agent state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsmAgentState {
    pub registered: bool,
    pub ping_status: String,
    pub agent_version: String,
    pub last_ping_time: String,
}

/// SSM Agent health status after restart
#[derive(Debug, Clone)]
pub enum SsmAgentHealthStatus {
    Healthy,
    Unhealthy(String),
    Unknown,
}

/// Instance details for fix analysis
#[derive(Debug, Clone)]
pub struct InstanceDetails {
    pub instance_id: String,
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
    pub iam_instance_profile: Option<String>,
    pub security_groups: Vec<SecurityGroupInfo>,
}

/// Security group information
#[derive(Debug, Clone)]
pub struct SecurityGroupInfo {
    pub group_id: String,
    pub group_name: String,
}

/// Fix verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixVerificationResult {
    pub fix_type: String,
    pub instance_id: String,
    pub verified: bool,
    pub verification_details: Vec<String>,
    pub connectivity_restored: bool,
    pub remaining_issues: Vec<String>,
    pub verification_timestamp: String,
}

/// Fix effectiveness report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixEffectivenessReport {
    pub instance_id: String,
    pub total_fixes_applied: u32,
    pub successful_fixes: u32,
    pub failed_fixes: u32,
    pub verification_results: Vec<FixVerificationResult>,
    pub overall_connectivity_status: bool,
    pub recommendations: Vec<String>,
    pub report_timestamp: String,
}

/// Types of fix actions that can be performed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FixActionType {
    StartInstance,
    RestartSsmAgent,
    UpdateCredentials,
    RestoreConfig,
    TerminateProcess,
    CreateVpcEndpoint,
    UpdateSecurityGroup,
    SuggestManualFix,
}

/// Risk level associated with a fix action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

/// A fix action that can be performed to resolve a diagnostic issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixAction {
    pub action_type: FixActionType,
    pub description: String,
    pub command: Option<String>,
    pub requires_confirmation: bool,
    pub risk_level: RiskLevel,
    pub estimated_duration: Duration,
    pub target_resource: String,
    pub prerequisites: Vec<String>,
}

impl FixAction {
    pub fn new(
        action_type: FixActionType,
        description: String,
        target_resource: String,
    ) -> Self {
        let (requires_confirmation, risk_level, estimated_duration) = match action_type {
            FixActionType::StartInstance => (true, RiskLevel::Low, Duration::from_secs(60)),
            FixActionType::RestartSsmAgent => (true, RiskLevel::Medium, Duration::from_secs(30)),
            FixActionType::UpdateCredentials => (false, RiskLevel::Safe, Duration::from_secs(5)),
            FixActionType::RestoreConfig => (true, RiskLevel::Medium, Duration::from_secs(10)),
            FixActionType::TerminateProcess => (true, RiskLevel::High, Duration::from_secs(5)),
            FixActionType::CreateVpcEndpoint => (true, RiskLevel::High, Duration::from_secs(300)),
            FixActionType::UpdateSecurityGroup => (true, RiskLevel::High, Duration::from_secs(30)),
            FixActionType::SuggestManualFix => (false, RiskLevel::Safe, Duration::from_secs(0)),
        };

        Self {
            action_type,
            description,
            command: None,
            requires_confirmation,
            risk_level,
            estimated_duration,
            target_resource,
            prerequisites: Vec::new(),
        }
    }

    pub fn with_command(mut self, command: String) -> Self {
        self.command = Some(command);
        self
    }

    pub fn with_prerequisites(mut self, prerequisites: Vec<String>) -> Self {
        self.prerequisites = prerequisites;
        self
    }

    pub fn is_safe_to_auto_execute(&self) -> bool {
        matches!(self.risk_level, RiskLevel::Safe | RiskLevel::Low) && !self.requires_confirmation
    }
}

/// Result of executing a fix action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixResult {
    pub action: FixAction,
    pub success: bool,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub duration: Duration,
    pub retry_count: u32,
}

impl FixResult {
    pub fn success(action: FixAction, message: String, duration: Duration) -> Self {
        Self {
            action,
            success: true,
            message,
            details: None,
            duration,
            retry_count: 0,
        }
    }

    pub fn failure(action: FixAction, message: String, duration: Duration) -> Self {
        Self {
            action,
            success: false,
            message,
            details: None,
            duration,
            retry_count: 0,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_retry_count(mut self, retry_count: u32) -> Self {
        self.retry_count = retry_count;
        self
    }
}

/// Trait for auto-fix manager implementations
#[async_trait]
pub trait AutoFixManager {
    /// Analyze diagnostic results and generate fix actions
    async fn analyze_fixes(&self, diagnostics: &[DiagnosticResult]) -> Result<Vec<FixAction>>;
    
    /// Execute a single fix action
    async fn execute_fix(&mut self, action: FixAction) -> Result<FixResult>;
    
    /// Execute only safe fixes automatically
    async fn execute_safe_fixes(&mut self, actions: Vec<FixAction>) -> Result<Vec<FixResult>>;
    
    /// Generate manual instructions for a fix action
    fn generate_manual_instructions(&self, action: &FixAction) -> String;
    
    /// Fix instance state with user approval (requirement 10.1)
    async fn fix_instance_state(&mut self, instance_id: &str, user_approved: bool) -> Result<FixResult>;
    
    /// Fix SSM agent with detailed verification (requirement 10.2)
    async fn fix_ssm_agent(&mut self, instance_id: &str) -> Result<FixResult>;
    
    /// Suggest IAM permission fixes with detailed analysis (requirement 10.3)
    async fn suggest_iam_fixes(&self, instance_id: &str) -> Result<Vec<String>>;
    
    /// Suggest security group fixes with specific recommendations (requirement 10.4)
    async fn suggest_security_group_fixes(&self, instance_id: &str) -> Result<Vec<String>>;
    
    /// Verify fix results and re-evaluate connection possibility (requirement 10.5)
    async fn verify_fix(&self, instance_id: &str, fix_type: &str) -> Result<bool>;
    
    /// Generate fix effectiveness report
    async fn generate_fix_effectiveness_report(&self, instance_id: &str, applied_fixes: &[FixResult]) -> Result<FixEffectivenessReport>;
    
    /// Generate fix recommendations based on verification results
    async fn generate_fix_recommendations(&self, verification_results: &[FixVerificationResult]) -> Result<Vec<String>>;
    
    /// Get detailed instance information for fix analysis
    async fn get_instance_details(&self, instance_id: &str) -> Result<InstanceDetails>;
    
    /// Verify instance start fix effectiveness
    async fn verify_instance_start_fix(&self, instance_id: &str) -> Result<FixVerificationResult>;
    
    /// Verify SSM agent fix effectiveness
    async fn verify_ssm_agent_fix(&self, instance_id: &str) -> Result<FixVerificationResult>;
    
    /// Verify IAM permissions fix effectiveness
    async fn verify_iam_permissions_fix(&self, instance_id: &str) -> Result<FixVerificationResult>;
    
    /// Verify security group fix effectiveness
    async fn verify_security_group_fix(&self, instance_id: &str) -> Result<FixVerificationResult>;
    
    /// Check security group rules for HTTPS outbound access
    async fn check_security_group_rules(&self, group_id: &str) -> Result<bool>;
    
    /// Verify general connectivity to AWS services
    async fn verify_general_connectivity(&self, instance_id: &str) -> Result<bool>;
}

/// Default implementation of the auto-fix manager
pub struct DefaultAutoFixManager {
    ec2_client: Ec2Client,
    ssm_client: SsmClient,
    dry_run: bool,
}

impl DefaultAutoFixManager {
    /// Create a new auto-fix manager with default AWS configuration
    pub async fn with_default_aws() -> anyhow::Result<Self> {
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let ec2_client = Ec2Client::new(&config);
        let ssm_client = SsmClient::new(&config);
        
        Ok(Self {
            ec2_client,
            ssm_client,
            dry_run: false,
        })
    }

    /// Create a new auto-fix manager with custom AWS configuration
    pub async fn with_aws_config(region: Option<String>, profile: Option<String>) -> anyhow::Result<Self> {
        let mut config_loader = aws_config::defaults(BehaviorVersion::latest());
        
        if let Some(region) = region {
            config_loader = config_loader.region(Region::new(region));
        }
        
        if let Some(profile) = profile {
            config_loader = config_loader.profile_name(profile);
        }
        
        let config = config_loader.load().await;
        let ec2_client = Ec2Client::new(&config);
        let ssm_client = SsmClient::new(&config);
        
        Ok(Self {
            ec2_client,
            ssm_client,
            dry_run: false,
        })
    }

    /// Enable or disable dry-run mode
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
        if dry_run {
            info!("Auto-fix manager set to dry-run mode - no actual changes will be made");
        } else {
            info!("Auto-fix manager set to live mode - changes will be applied");
        }
    }

    /// Analyze instance state issues and generate fix actions
    fn analyze_instance_fixes(&self, result: &DiagnosticResult) -> Vec<FixAction> {
        let mut fixes = Vec::new();

        debug!("analyze_instance_fixes: status={:?}, message={}", result.status, result.message);
        
        if result.status == DiagnosticStatus::Error {
            if result.message.contains("stopped") || result.message.contains("stopping") {
                debug!("Found stopped/stopping instance, extracting instance_id from details");
                let instance_id = result.details.as_ref()
                    .and_then(|d| d.get("instance_id"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("unknown");
                debug!("Extracted instance_id: {}", instance_id);
                
                let fix = FixAction::new(
                    FixActionType::StartInstance,
                    format!("Start the stopped EC2 instance: {}", instance_id),
                    instance_id.to_string(),
                );
                debug!("Created StartInstance fix action for: {}", instance_id);
                fixes.push(fix);
            } else if result.message.contains("terminated") {
                let fix = FixAction::new(
                    FixActionType::SuggestManualFix,
                    "Instance is terminated and cannot be recovered. Launch a new instance.".to_string(),
                    "manual".to_string(),
                );
                fixes.push(fix);
            }
        }

        debug!("analyze_instance_fixes returning {} fixes", fixes.len());
        fixes
    }

    /// Analyze SSM agent issues and generate fix actions
    fn analyze_ssm_agent_fixes(&self, result: &DiagnosticResult) -> Vec<FixAction> {
        let mut fixes = Vec::new();

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("not registered") || result.message.contains("offline") {
                let instance_id = result.details.as_ref()
                    .and_then(|d| d.get("instance_id"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("unknown");

                let fix = FixAction::new(
                    FixActionType::RestartSsmAgent,
                    format!("Restart SSM agent on instance: {}", instance_id),
                    instance_id.to_string(),
                ).with_command("sudo systemctl restart amazon-ssm-agent".to_string());
                fixes.push(fix);
            }
        }

        fixes
    }

    /// Analyze IAM permission issues and generate fix actions
    fn analyze_iam_fixes(&self, result: &DiagnosticResult) -> Vec<FixAction> {
        let mut fixes = Vec::new();

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("credentials") || result.message.contains("authentication") {
                let fix = FixAction::new(
                    FixActionType::UpdateCredentials,
                    "Update AWS credentials configuration".to_string(),
                    "credentials".to_string(),
                );
                fixes.push(fix);
            } else if result.message.contains("permissions") || result.message.contains("access denied") {
                let fix = FixAction::new(
                    FixActionType::SuggestManualFix,
                    "IAM permissions need to be updated manually by an administrator".to_string(),
                    "iam".to_string(),
                );
                fixes.push(fix);
            }
        }

        fixes
    }

    /// Analyze network issues and generate fix actions
    fn analyze_network_fixes(&self, result: &DiagnosticResult) -> Vec<FixAction> {
        let mut fixes = Vec::new();

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("VPC endpoint") {
                let fix = FixAction::new(
                    FixActionType::SuggestManualFix,
                    "VPC endpoints need to be created manually for SSM connectivity".to_string(),
                    "vpc".to_string(),
                );
                fixes.push(fix);
            } else if result.message.contains("security group") {
                let fix = FixAction::new(
                    FixActionType::SuggestManualFix,
                    "Security group rules need to be updated to allow HTTPS outbound traffic".to_string(),
                    "security_group".to_string(),
                );
                fixes.push(fix);
            }
        }

        fixes
    }

    /// Analyze port availability issues and generate fix actions
    fn analyze_port_fixes(&self, result: &DiagnosticResult) -> Vec<FixAction> {
        let mut fixes = Vec::new();

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("port in use") || result.message.contains("already bound") {
                if let Some(details) = &result.details {
                    if let Some(process_info) = details.get("process_info") {
                        let process_name = process_info.get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown");
                        let pid = process_info.get("pid")
                            .and_then(|p| p.as_u64())
                            .unwrap_or(0);

                        let fix = FixAction::new(
                            FixActionType::TerminateProcess,
                            format!("Terminate process {} (PID: {}) that is using the port", process_name, pid),
                            format!("process:{}", pid),
                        );
                        fixes.push(fix);
                    }
                }

                // Also suggest using alternative port
                let fix = FixAction::new(
                    FixActionType::SuggestManualFix,
                    "Use an alternative port for the connection".to_string(),
                    "port".to_string(),
                );
                fixes.push(fix);
            }
        }

        fixes
    }

    /// Execute instance start fix with progress monitoring
    async fn execute_start_instance(&mut self, instance_id: &str) -> Result<FixResult> {
        self.execute_start_instance_with_monitoring(instance_id, false).await
    }

    /// Execute instance start fix with user approval and progress monitoring
    async fn execute_start_instance_with_monitoring(&mut self, instance_id: &str, user_approved: bool) -> Result<FixResult> {
        let start_time = Instant::now();
        let action = FixAction::new(
            FixActionType::StartInstance,
            format!("Starting instance: {}", instance_id),
            instance_id.to_string(),
        );

        info!("Executing start instance fix for: {}", instance_id);

        if self.dry_run {
            info!("DRY RUN: Would start instance {}", instance_id);
            return Ok(FixResult::success(
                action,
                format!("DRY RUN: Would start instance {}", instance_id),
                start_time.elapsed(),
            ));
        }

        // Check if user approval is required and not provided
        if action.requires_confirmation && !user_approved {
            return Ok(FixResult::failure(
                action,
                "User approval required to start instance".to_string(),
                start_time.elapsed(),
            ));
        }

        // First, get current instance state
        let describe_request = self.ec2_client
            .describe_instances()
            .instance_ids(instance_id);

        let current_state = match describe_request.send().await {
            Ok(response) => {
                if let Some(reservation) = response.reservations().first() {
                    if let Some(instance) = reservation.instances().first() {
                        instance.state()
                            .and_then(|s| s.name())
                            .map(|n| format!("{:?}", n))
                            .unwrap_or_else(|| "unknown".to_string())
                    } else {
                        return Ok(FixResult::failure(
                            action,
                            "Instance not found".to_string(),
                            start_time.elapsed(),
                        ));
                    }
                } else {
                    return Ok(FixResult::failure(
                        action,
                        "Instance not found".to_string(),
                        start_time.elapsed(),
                    ));
                }
            }
            Err(e) => {
                error!("Failed to describe instance {}: {}", instance_id, e);
                return Ok(FixResult::failure(
                    action,
                    format!("Failed to describe instance: {}", e),
                    start_time.elapsed(),
                ));
            }
        };

        info!("Current instance state: {}", current_state);

        // Check if instance is already running
        if current_state.contains("running") {
            return Ok(FixResult::success(
                action,
                "Instance is already running".to_string(),
                start_time.elapsed(),
            ));
        }

        // Check if instance can be started
        if current_state.contains("terminated") || current_state.contains("terminating") {
            return Ok(FixResult::failure(
                action,
                "Cannot start terminated instance".to_string(),
                start_time.elapsed(),
            ));
        }

        // Start the instance
        let request = self.ec2_client
            .start_instances()
            .instance_ids(instance_id);

        match request.send().await {
            Ok(response) => {
                let starting_instances = response.starting_instances();
                if let Some(instance) = starting_instances.first() {
                    let new_state = instance.current_state()
                        .and_then(|s| s.name())
                        .map(|n| format!("{:?}", n))
                        .unwrap_or_else(|| "unknown".to_string());
                    
                    info!("Instance {} start initiated, new state: {}", instance_id, new_state);
                    
                    // Monitor startup progress
                    let monitoring_result = self.monitor_instance_startup(instance_id).await;
                    
                    let final_message = match monitoring_result {
                        Ok(final_state) => {
                            format!("Instance started successfully. Final state: {}", final_state)
                        }
                        Err(e) => {
                            format!("Instance start initiated but monitoring failed: {}", e)
                        }
                    };
                    
                    Ok(FixResult::success(
                        action,
                        final_message,
                        start_time.elapsed(),
                    ))
                } else {
                    error!("No instance information returned");
                    Ok(FixResult::failure(
                        action,
                        "No instance information returned".to_string(),
                        start_time.elapsed(),
                    ))
                }
            }
            Err(e) => {
                error!("Failed to start instance {}: {}", instance_id, e);
                Ok(FixResult::failure(
                    action,
                    format!("Failed to start instance: {}", e),
                    start_time.elapsed(),
                ))
            }
        }
    }

    /// Monitor instance startup progress
    async fn monitor_instance_startup(&self, instance_id: &str) -> Result<String> {
        info!("Monitoring startup progress for instance: {}", instance_id);
        
        let max_wait_time = Duration::from_secs(300); // 5 minutes max wait
        let check_interval = Duration::from_secs(10); // Check every 10 seconds
        let start_time = Instant::now();
        
        loop {
            if start_time.elapsed() > max_wait_time {
                return Err(Ec2ConnectError::Aws(AwsError::Timeout {
                    operation: "Instance startup monitoring".to_string()
                }).into());
            }
            
            let describe_request = self.ec2_client
                .describe_instances()
                .instance_ids(instance_id);

            match describe_request.send().await {
                Ok(response) => {
                    if let Some(reservation) = response.reservations().first() {
                        if let Some(instance) = reservation.instances().first() {
                            let state = instance.state()
                                .and_then(|s| s.name())
                                .map(|n| format!("{:?}", n))
                                .unwrap_or_else(|| "unknown".to_string());
                            
                            debug!("Instance {} current state: {}", instance_id, state);
                            
                            if state.contains("running") {
                                info!("Instance {} is now running", instance_id);
                                return Ok(state);
                            } else if state.contains("stopped") || state.contains("stopping") {
                                return Err(Ec2ConnectError::Aws(AwsError::Ec2ServiceError {
                                    message: format!("Instance startup failed, current state: {}", state)
                                }).into());
                            }
                            
                            // Continue monitoring for pending/starting states
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to check instance state during monitoring: {}", e);
                    return Err(Ec2ConnectError::Aws(AwsError::Ec2ServiceError {
                        message: format!("Failed to check instance state: {}", e)
                    }).into());
                }
            }
            
            tokio::time::sleep(check_interval).await;
        }
    }

    /// Execute SSM agent restart fix with enhanced verification
    async fn execute_restart_ssm_agent(&mut self, instance_id: &str) -> Result<FixResult> {
        self.execute_restart_ssm_agent_with_verification(instance_id).await
    }

    /// Execute SSM agent restart fix with detailed state checking and verification
    async fn execute_restart_ssm_agent_with_verification(&mut self, instance_id: &str) -> Result<FixResult> {
        let start_time = Instant::now();
        let action = FixAction::new(
            FixActionType::RestartSsmAgent,
            format!("Restarting SSM agent on instance: {}", instance_id),
            instance_id.to_string(),
        );

        info!("Executing enhanced SSM agent restart fix for: {}", instance_id);

        if self.dry_run {
            info!("DRY RUN: Would restart SSM agent on instance {}", instance_id);
            return Ok(FixResult::success(
                action,
                format!("DRY RUN: Would restart SSM agent on instance {}", instance_id),
                start_time.elapsed(),
            ));
        }

        // Step 1: Check initial SSM agent state
        let initial_state = self.check_ssm_agent_state(instance_id).await?;
        info!("Initial SSM agent state: {:?}", initial_state);

        // Step 2: Send restart command
        let restart_result = self.send_ssm_restart_command(instance_id).await?;
        
        // Step 3: Wait for agent to restart
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Step 4: Verify agent is running and healthy
        let verification_result = self.verify_ssm_agent_health(instance_id).await?;
        
        let final_message = match &verification_result {
            SsmAgentHealthStatus::Healthy => {
                format!("SSM agent restarted successfully and is healthy. Command ID: {}", restart_result)
            }
            SsmAgentHealthStatus::Unhealthy(reason) => {
                format!("SSM agent restart completed but agent is unhealthy: {}. Command ID: {}", reason, restart_result)
            }
            SsmAgentHealthStatus::Unknown => {
                format!("SSM agent restart completed but health status unknown. Command ID: {}", restart_result)
            }
        };
        
        let success = matches!(verification_result, SsmAgentHealthStatus::Healthy);
        
        if success {
            Ok(FixResult::success(action, final_message, start_time.elapsed()))
        } else {
            Ok(FixResult::failure(action, final_message, start_time.elapsed()))
        }
    }

    /// Check SSM agent state on the instance
    async fn check_ssm_agent_state(&self, instance_id: &str) -> Result<SsmAgentState> {
        info!("Checking SSM agent state for instance: {}", instance_id);
        
        // Check if instance is registered with SSM
        let describe_request = self.ssm_client
            .describe_instance_information()
            .instance_information_filter_list(
                aws_sdk_ssm::types::InstanceInformationFilter::builder()
                    .key(aws_sdk_ssm::types::InstanceInformationFilterKey::InstanceIds)
                    .value_set(instance_id)
                    .build()
                    .map_err(|e| Ec2ConnectError::Aws(AwsError::SsmServiceError {
                        message: format!("Failed to build instance filter: {}", e)
                    }))?
            );

        match describe_request.send().await {
            Ok(response) => {
                if let Some(instance_info) = response.instance_information_list().first() {
                    let ping_status = instance_info.ping_status()
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_else(|| "Unknown".to_string());
                    
                    let agent_version = instance_info.agent_version()
                        .unwrap_or("Unknown")
                        .to_string();
                    
                    let last_ping_time = instance_info.last_ping_date_time()
                        .map(|dt| dt.to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    
                    Ok(SsmAgentState {
                        registered: true,
                        ping_status,
                        agent_version,
                        last_ping_time,
                    })
                } else {
                    Ok(SsmAgentState {
                        registered: false,
                        ping_status: "Not registered".to_string(),
                        agent_version: "Unknown".to_string(),
                        last_ping_time: "Never".to_string(),
                    })
                }
            }
            Err(e) => {
                error!("Failed to check SSM agent state for instance {}: {}", instance_id, e);
                Err(Ec2ConnectError::Aws(AwsError::SsmServiceError {
                    message: format!("Failed to check SSM agent state: {}", e)
                }).into())
            }
        }
    }

    /// Send SSM restart command to the instance
    async fn send_ssm_restart_command(&self, instance_id: &str) -> Result<String> {
        info!("Sending SSM restart command to instance: {}", instance_id);
        
        let request = self.ssm_client
            .send_command()
            .instance_ids(instance_id)
            .document_name("AWS-RunShellScript")
            .parameters("commands", vec![
                "sudo systemctl restart amazon-ssm-agent".to_string(),
                "sleep 5".to_string(),
                "sudo systemctl status amazon-ssm-agent".to_string()
            ]);

        match request.send().await {
            Ok(response) => {
                if let Some(command) = response.command() {
                    let command_id = command.command_id().unwrap_or("unknown");
                    info!("SSM restart command sent successfully, command ID: {}", command_id);
                    Ok(command_id.to_string())
                } else {
                    error!("No command information returned");
                    Err(Ec2ConnectError::Aws(AwsError::SsmServiceError {
                        message: "No command information returned".to_string()
                    }).into())
                }
            }
            Err(e) => {
                error!("Failed to send SSM restart command to instance {}: {}", instance_id, e);
                Err(Ec2ConnectError::Aws(AwsError::SsmServiceError {
                    message: format!("Failed to send SSM restart command: {}", e)
                }).into())
            }
        }
    }

    /// Verify SSM agent health after restart
    async fn verify_ssm_agent_health(&self, instance_id: &str) -> Result<SsmAgentHealthStatus> {
        info!("Verifying SSM agent health for instance: {}", instance_id);
        
        let max_wait_time = Duration::from_secs(60); // 1 minute max wait
        let check_interval = Duration::from_secs(5); // Check every 5 seconds
        let start_time = Instant::now();
        
        loop {
            if start_time.elapsed() > max_wait_time {
                return Ok(SsmAgentHealthStatus::Unknown);
            }
            
            match self.check_ssm_agent_state(instance_id).await {
                Ok(state) => {
                    if state.registered && state.ping_status.contains("Online") {
                        info!("SSM agent is healthy for instance: {}", instance_id);
                        return Ok(SsmAgentHealthStatus::Healthy);
                    } else if state.registered {
                        debug!("SSM agent registered but not online yet: {}", state.ping_status);
                    } else {
                        debug!("SSM agent not registered yet");
                    }
                }
                Err(e) => {
                    debug!("Error checking SSM agent state during verification: {}", e);
                    return Ok(SsmAgentHealthStatus::Unhealthy(format!("Health check failed: {}", e)));
                }
            }
            
            tokio::time::sleep(check_interval).await;
        }
    }

    /// Execute credentials update fix
    async fn execute_update_credentials(&mut self) -> Result<FixResult> {
        let start_time = Instant::now();
        let action = FixAction::new(
            FixActionType::UpdateCredentials,
            "Updating AWS credentials configuration".to_string(),
            "credentials".to_string(),
        );

        info!("Executing credentials update fix");

        if self.dry_run {
            info!("DRY RUN: Would update AWS credentials");
            return Ok(FixResult::success(
                action,
                "DRY RUN: Would update AWS credentials".to_string(),
                start_time.elapsed(),
            ));
        }

        // Try to refresh credentials by creating a new config
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        
        // Update our clients with the new config
        self.ec2_client = Ec2Client::new(&config);
        self.ssm_client = SsmClient::new(&config);
        
        info!("Successfully refreshed AWS credentials");
        Ok(FixResult::success(
            action,
            "AWS credentials refreshed successfully".to_string(),
            start_time.elapsed(),
        ))
    }

    /// Execute process termination fix
    async fn execute_terminate_process(&mut self, process_target: &str) -> Result<FixResult> {
        let start_time = Instant::now();
        let action = FixAction::new(
            FixActionType::TerminateProcess,
            format!("Terminating process: {}", process_target),
            process_target.to_string(),
        );

        info!("Executing process termination fix for: {}", process_target);

        if self.dry_run {
            info!("DRY RUN: Would terminate process {}", process_target);
            return Ok(FixResult::success(
                action,
                format!("DRY RUN: Would terminate process {}", process_target),
                start_time.elapsed(),
            ));
        }

        // Extract PID from process target (format: "process:1234")
        if let Some(pid_str) = process_target.strip_prefix("process:") {
            if let Ok(pid) = pid_str.parse::<u32>() {
                // Try to terminate the process
                #[cfg(unix)]
                {
                    let output = Command::new("kill")
                        .arg("-TERM")
                        .arg(pid.to_string())
                        .output();

                    match output {
                        Ok(result) => {
                            if result.status.success() {
                                info!("Successfully terminated process with PID: {}", pid);
                                Ok(FixResult::success(
                                    action,
                                    format!("Successfully terminated process with PID: {}", pid),
                                    start_time.elapsed(),
                                ))
                            } else {
                                let error_msg = String::from_utf8_lossy(&result.stderr);
                                error!("Failed to terminate process {}: {}", pid, error_msg);
                                Ok(FixResult::failure(
                                    action,
                                    format!("Failed to terminate process {}: {}", pid, error_msg),
                                    start_time.elapsed(),
                                ))
                            }
                        }
                        Err(e) => {
                            error!("Failed to execute kill command: {}", e);
                            Ok(FixResult::failure(
                                action,
                                format!("Failed to execute kill command: {}", e),
                                start_time.elapsed(),
                            ))
                        }
                    }
                }

                #[cfg(windows)]
                {
                    let output = Command::new("taskkill")
                        .arg("/PID")
                        .arg(pid.to_string())
                        .arg("/F")
                        .output();

                    match output {
                        Ok(result) => {
                            if result.status.success() {
                                info!("Successfully terminated process with PID: {}", pid);
                                Ok(FixResult::success(
                                    action,
                                    format!("Successfully terminated process with PID: {}", pid),
                                    start_time.elapsed(),
                                ))
                            } else {
                                let error_msg = String::from_utf8_lossy(&result.stderr);
                                error!("Failed to terminate process {}: {}", pid, error_msg);
                                Ok(FixResult::failure(
                                    action,
                                    format!("Failed to terminate process {}: {}", pid, error_msg),
                                    start_time.elapsed(),
                                ))
                            }
                        }
                        Err(e) => {
                            error!("Failed to execute taskkill command: {}", e);
                            Ok(FixResult::failure(
                                action,
                                format!("Failed to execute taskkill command: {}", e),
                                start_time.elapsed(),
                            ))
                        }
                    }
                }
            } else {
                error!("Invalid PID format: {}", pid_str);
                Ok(FixResult::failure(
                    action,
                    format!("Invalid PID format: {}", pid_str),
                    start_time.elapsed(),
                ))
            }
        } else {
            error!("Invalid process target format: {}", process_target);
            Ok(FixResult::failure(
                action,
                format!("Invalid process target format: {}", process_target),
                start_time.elapsed(),
            ))
        }
    }
}

#[async_trait]
impl AutoFixManager for DefaultAutoFixManager {
    async fn analyze_fixes(&self, diagnostics: &[DiagnosticResult]) -> Result<Vec<FixAction>> {
        info!("Analyzing {} diagnostic results for potential fixes", diagnostics.len());
        
        let mut all_fixes = Vec::new();
        
        for result in diagnostics {
            debug!("Analyzing fixes for diagnostic item: {} (status: {:?}, message: {})", 
                result.item_name, result.status, result.message);
            
            let fixes = match result.item_name.as_str() {
                "instance_state" | "detailed_instance_state" => {
                    debug!("Matched instance state diagnostic, calling analyze_instance_fixes");
                    let instance_fixes = self.analyze_instance_fixes(result);
                    debug!("Generated {} instance fixes", instance_fixes.len());
                    instance_fixes
                },
                "ssm_agent" | "ssm_agent_enhanced" => self.analyze_ssm_agent_fixes(result),
                "iam_permissions" => self.analyze_iam_fixes(result),
                "vpc_endpoints" | "security_groups" | "network_connectivity" => self.analyze_network_fixes(result),
                "local_port_availability" => self.analyze_port_fixes(result),
                _ => {
                    debug!("No specific fix analysis for diagnostic item: {}", result.item_name);
                    Vec::new()
                }
            };
            
            all_fixes.extend(fixes);
        }
        
        // Sort fixes by risk level (safest first)
        all_fixes.sort_by(|a, b| a.risk_level.cmp(&b.risk_level));
        
        info!("Generated {} potential fix actions", all_fixes.len());
        Ok(all_fixes)
    }

    async fn execute_fix(&mut self, action: FixAction) -> Result<FixResult> {
        info!("Executing fix action: {:?} for target: {}", action.action_type, action.target_resource);
        
        match action.action_type {
            FixActionType::StartInstance => {
                self.execute_start_instance(&action.target_resource).await
            }
            FixActionType::RestartSsmAgent => {
                self.execute_restart_ssm_agent(&action.target_resource).await
            }
            FixActionType::UpdateCredentials => {
                self.execute_update_credentials().await
            }
            FixActionType::TerminateProcess => {
                self.execute_terminate_process(&action.target_resource).await
            }
            FixActionType::RestoreConfig => {
                // For now, return a manual suggestion
                let start_time = Instant::now();
                Ok(FixResult::success(
                    action,
                    "Configuration restoration requires manual intervention".to_string(),
                    start_time.elapsed(),
                ))
            }
            FixActionType::CreateVpcEndpoint | 
            FixActionType::UpdateSecurityGroup | 
            FixActionType::SuggestManualFix => {
                // These are manual fixes
                let start_time = Instant::now();
                Ok(FixResult::success(
                    action,
                    "Manual fix action - see instructions".to_string(),
                    start_time.elapsed(),
                ))
            }
        }
    }

    async fn execute_safe_fixes(&mut self, actions: Vec<FixAction>) -> Result<Vec<FixResult>> {
        info!("Executing {} safe fix actions", actions.len());
        
        let mut results = Vec::new();
        
        for action in actions {
            if action.is_safe_to_auto_execute() {
                info!("Auto-executing safe fix: {:?}", action.action_type);
                let result = self.execute_fix(action).await?;
                results.push(result);
            } else {
                info!("Skipping non-safe fix: {:?} (requires confirmation or high risk)", action.action_type);
                let start_time = Instant::now();
                let result = FixResult::success(
                    action,
                    "Skipped - requires manual confirmation".to_string(),
                    start_time.elapsed(),
                );
                results.push(result);
            }
        }
        
        info!("Completed execution of safe fixes, {} results", results.len());
        Ok(results)
    }

    fn generate_manual_instructions(&self, action: &FixAction) -> String {
        match action.action_type {
            FixActionType::StartInstance => {
                format!(
                    "To start the instance manually:\n\
                    1. Open the AWS EC2 Console\n\
                    2. Navigate to Instances\n\
                    3. Select instance: {}\n\
                    4. Click 'Instance State' -> 'Start instance'\n\
                    5. Wait for the instance to reach 'running' state\n\n\
                    Or use AWS CLI:\n\
                    aws ec2 start-instances --instance-ids {}",
                    action.target_resource, action.target_resource
                )
            }
            FixActionType::RestartSsmAgent => {
                format!(
                    "To restart the SSM agent manually:\n\
                    1. Connect to the instance using SSH or Session Manager\n\
                    2. Run the following command:\n\
                    sudo systemctl restart amazon-ssm-agent\n\
                    3. Verify the agent is running:\n\
                    sudo systemctl status amazon-ssm-agent\n\n\
                    For Windows instances:\n\
                    Restart-Service AmazonSSMAgent"
                )
            }
            FixActionType::UpdateCredentials => {
                "To update AWS credentials manually:\n\
                1. Run 'aws configure' to set up credentials\n\
                2. Or set environment variables:\n\
                   export AWS_ACCESS_KEY_ID=your_access_key\n\
                   export AWS_SECRET_ACCESS_KEY=your_secret_key\n\
                3. Or use AWS SSO: aws sso login\n\
                4. Verify credentials: aws sts get-caller-identity".to_string()
            }
            FixActionType::CreateVpcEndpoint => {
                "To create VPC endpoints for SSM:\n\
                1. Open the VPC Console\n\
                2. Navigate to Endpoints\n\
                3. Create endpoints for:\n\
                   - com.amazonaws.region.ssm\n\
                   - com.amazonaws.region.ssmmessages\n\
                   - com.amazonaws.region.ec2messages\n\
                4. Associate with the appropriate VPC and subnets\n\
                5. Configure security groups to allow HTTPS traffic".to_string()
            }
            FixActionType::UpdateSecurityGroup => {
                "To update security group rules:\n\
                1. Open the EC2 Console\n\
                2. Navigate to Security Groups\n\
                3. Select the security group attached to your instance\n\
                4. Add outbound rule:\n\
                   - Type: HTTPS\n\
                   - Protocol: TCP\n\
                   - Port: 443\n\
                   - Destination: 0.0.0.0/0\n\
                5. Save the changes".to_string()
            }
            FixActionType::TerminateProcess => {
                format!(
                    "To terminate the process manually:\n\
                    On Linux/macOS:\n\
                    kill -TERM <PID>\n\
                    or\n\
                    kill -9 <PID> (force kill)\n\n\
                    On Windows:\n\
                    taskkill /PID <PID> /F\n\n\
                    Target: {}",
                    action.target_resource
                )
            }
            FixActionType::RestoreConfig => {
                "To restore configuration:\n\
                1. Backup current configuration files\n\
                2. Restore from a known good backup\n\
                3. Or reset to default configuration\n\
                4. Restart relevant services\n\
                5. Test the configuration".to_string()
            }
            FixActionType::SuggestManualFix => {
                format!("Manual intervention required: {}", action.description)
            }
        }
    }

    async fn fix_instance_state(&mut self, instance_id: &str, user_approved: bool) -> Result<FixResult> {
        info!("Fixing instance state for: {} (user_approved: {})", instance_id, user_approved);
        self.execute_start_instance_with_monitoring(instance_id, user_approved).await
    }

    async fn fix_ssm_agent(&mut self, instance_id: &str) -> Result<FixResult> {
        info!("Fixing SSM agent for: {}", instance_id);
        self.execute_restart_ssm_agent_with_verification(instance_id).await
    }

    async fn suggest_iam_fixes(&self, instance_id: &str) -> Result<Vec<String>> {
        info!("Analyzing IAM permission fixes for instance: {}", instance_id);
        
        let mut suggestions = Vec::new();
        
        // Get instance details for more specific recommendations
        let instance_details = self.get_instance_details(instance_id).await?;
        
        // Basic SSM permissions
        suggestions.push("Ensure the EC2 instance has an IAM role attached with the following managed policies:".to_string());
        suggestions.push("  - AmazonSSMManagedInstanceCore (required for basic SSM functionality)".to_string());
        suggestions.push("  - AmazonSSMPatchAssociation (for patch management)".to_string());
        
        // VPC-specific permissions if instance is in VPC
        if let Some(vpc_id) = &instance_details.vpc_id {
            suggestions.push(format!("For VPC instance ({}), ensure the following:", vpc_id));
            suggestions.push("  - VPC endpoints are configured for SSM services".to_string());
            suggestions.push("  - Security groups allow HTTPS outbound traffic (port 443)".to_string());
        }
        
        // Instance profile check
        if instance_details.iam_instance_profile.is_none() {
            suggestions.push("⚠️  CRITICAL: No IAM instance profile attached to the instance".to_string());
            suggestions.push("  1. Create an IAM role with SSM permissions".to_string());
            suggestions.push("  2. Create an instance profile for the role".to_string());
            suggestions.push("  3. Attach the instance profile to the EC2 instance".to_string());
            suggestions.push("  4. Restart the SSM agent after attaching the profile".to_string());
        }
        
        // Detailed permission analysis
        suggestions.push("Required IAM permissions for SSM connectivity:".to_string());
        suggestions.push("  - ssm:UpdateInstanceInformation".to_string());
        suggestions.push("  - ssm:SendCommand".to_string());
        suggestions.push("  - ssm:ListCommands".to_string());
        suggestions.push("  - ssm:ListCommandInvocations".to_string());
        suggestions.push("  - ssm:DescribeInstanceInformation".to_string());
        suggestions.push("  - ssm:GetDeployablePatchSnapshotForInstance".to_string());
        suggestions.push("  - ec2messages:AcknowledgeMessage".to_string());
        suggestions.push("  - ec2messages:DeleteMessage".to_string());
        suggestions.push("  - ec2messages:FailMessage".to_string());
        suggestions.push("  - ec2messages:GetEndpoint".to_string());
        suggestions.push("  - ec2messages:GetMessages".to_string());
        suggestions.push("  - ec2messages:SendReply".to_string());
        
        // AWS CLI commands for verification
        suggestions.push("Verification commands:".to_string());
        suggestions.push(format!("  aws ec2 describe-instances --instance-ids {}", instance_id));
        suggestions.push(format!("  aws ssm describe-instance-information --filters Key=InstanceIds,Values={}", instance_id));
        suggestions.push("  aws sts get-caller-identity".to_string());
        
        Ok(suggestions)
    }

    async fn suggest_security_group_fixes(&self, instance_id: &str) -> Result<Vec<String>> {
        info!("Analyzing security group fixes for instance: {}", instance_id);
        
        let mut suggestions = Vec::new();
        
        // Get instance details including security groups
        let instance_details = self.get_instance_details(instance_id).await?;
        
        suggestions.push("Security Group Requirements for SSM Connectivity:".to_string());
        suggestions.push("".to_string());
        
        // Outbound rules
        suggestions.push("Required OUTBOUND rules:".to_string());
        suggestions.push("  - Type: HTTPS".to_string());
        suggestions.push("  - Protocol: TCP".to_string());
        suggestions.push("  - Port: 443".to_string());
        suggestions.push("  - Destination: 0.0.0.0/0 (or specific AWS service endpoints)".to_string());
        suggestions.push("".to_string());
        
        // VPC endpoint specific recommendations
        if instance_details.vpc_id.is_some() {
            suggestions.push("For VPC instances, create VPC endpoints for:".to_string());
            suggestions.push("  - com.amazonaws.<region>.ssm".to_string());
            suggestions.push("  - com.amazonaws.<region>.ssmmessages".to_string());
            suggestions.push("  - com.amazonaws.<region>.ec2messages".to_string());
            suggestions.push("".to_string());
            
            suggestions.push("VPC endpoint security group requirements:".to_string());
            suggestions.push("  - Allow inbound HTTPS (port 443) from instance security groups".to_string());
            suggestions.push("".to_string());
        }
        
        // Current security groups analysis
        if !instance_details.security_groups.is_empty() {
            suggestions.push("Current security groups attached to instance:".to_string());
            for sg in &instance_details.security_groups {
                suggestions.push(format!("  - {} ({})", sg.group_name, sg.group_id));
            }
            suggestions.push("".to_string());
            
            suggestions.push("Verification steps:".to_string());
            for sg in &instance_details.security_groups {
                suggestions.push(format!("  aws ec2 describe-security-groups --group-ids {}", sg.group_id));
            }
        }
        
        // Common issues and solutions
        suggestions.push("Common security group issues and solutions:".to_string());
        suggestions.push("".to_string());
        suggestions.push("1. No outbound HTTPS rule:".to_string());
        suggestions.push("   - Add outbound rule: HTTPS (443) to 0.0.0.0/0".to_string());
        suggestions.push("".to_string());
        suggestions.push("2. Restrictive outbound rules:".to_string());
        suggestions.push("   - Ensure AWS service endpoints are reachable".to_string());
        suggestions.push("   - Consider using VPC endpoints for better security".to_string());
        suggestions.push("".to_string());
        suggestions.push("3. Network ACL restrictions:".to_string());
        suggestions.push("   - Check subnet-level Network ACLs".to_string());
        suggestions.push("   - Ensure they allow HTTPS traffic".to_string());
        
        // AWS CLI commands for fixes
        suggestions.push("".to_string());
        suggestions.push("Example AWS CLI commands to add HTTPS outbound rule:".to_string());
        if let Some(sg) = instance_details.security_groups.first() {
            suggestions.push(format!(
                "  aws ec2 authorize-security-group-egress --group-id {} --protocol tcp --port 443 --cidr 0.0.0.0/0",
                sg.group_id
            ));
        }
        
        Ok(suggestions)
    }

    /// Get instance details (private helper method)
    async fn get_instance_details(&self, instance_id: &str) -> Result<InstanceDetails> {
        info!("Getting instance details for: {}", instance_id);
        
        let describe_request = self.ec2_client
            .describe_instances()
            .instance_ids(instance_id);

        match describe_request.send().await {
            Ok(response) => {
                if let Some(reservation) = response.reservations().first() {
                    if let Some(instance) = reservation.instances().first() {
                        let vpc_id = instance.vpc_id().map(|s| s.to_string());
                        let subnet_id = instance.subnet_id().map(|s| s.to_string());
                        let iam_instance_profile = instance.iam_instance_profile()
                            .and_then(|profile| profile.arn())
                            .map(|s| s.to_string());
                        
                        let security_groups: Vec<SecurityGroupInfo> = instance.security_groups()
                            .iter()
                            .map(|sg| SecurityGroupInfo {
                                group_id: sg.group_id().unwrap_or("unknown").to_string(),
                                group_name: sg.group_name().unwrap_or("unknown").to_string(),
                            })
                            .collect();
                        
                        Ok(InstanceDetails {
                            instance_id: instance_id.to_string(),
                            vpc_id,
                            subnet_id,
                            iam_instance_profile,
                            security_groups,
                        })
                    } else {
                        Err(Ec2ConnectError::Aws(AwsError::InstanceNotFound {
                            instance_id: instance_id.to_string()
                        }).into())
                    }
                } else {
                    Err(Ec2ConnectError::Aws(AwsError::InstanceNotFound {
                        instance_id: instance_id.to_string()
                    }).into())
                }
            }
            Err(e) => {
                error!("Failed to describe instance {}: {}", instance_id, e);
                Err(Ec2ConnectError::Aws(AwsError::Ec2ServiceError {
                    message: format!("Failed to describe instance: {}", e)
                }).into())
            }
        }
    }



    async fn generate_fix_effectiveness_report(&self, instance_id: &str, applied_fixes: &[FixResult]) -> Result<FixEffectivenessReport> {
        info!("Generating fix effectiveness report for instance: {}", instance_id);
        
        let mut verification_results = Vec::new();
        let mut successful_fixes = 0;
        let mut failed_fixes = 0;
        
        // Verify each applied fix
        for fix_result in applied_fixes {
            let fix_type = match fix_result.action.action_type {
                FixActionType::StartInstance => "instance_start",
                FixActionType::RestartSsmAgent => "ssm_agent",
                FixActionType::UpdateCredentials => "iam_permissions",
                FixActionType::UpdateSecurityGroup => "security_group",
                _ => "general",
            };
            
            if fix_result.success {
                successful_fixes += 1;
                
                // Perform verification for successful fixes
                match self.verify_fix(instance_id, fix_type).await {
                    Ok(verified) => {
                        let verification_result = FixVerificationResult {
                            fix_type: fix_type.to_string(),
                            instance_id: instance_id.to_string(),
                            verified,
                            verification_details: vec![format!("Fix {} verification completed", fix_type)],
                            connectivity_restored: verified,
                            remaining_issues: if verified { Vec::new() } else { vec!["Verification failed".to_string()] },
                            verification_timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        verification_results.push(verification_result);
                    }
                    Err(e) => {
                        let verification_result = FixVerificationResult {
                            fix_type: fix_type.to_string(),
                            instance_id: instance_id.to_string(),
                            verified: false,
                            verification_details: vec![format!("Verification error: {}", e)],
                            connectivity_restored: false,
                            remaining_issues: vec![format!("Verification failed: {}", e)],
                            verification_timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        verification_results.push(verification_result);
                    }
                }
            } else {
                failed_fixes += 1;
            }
        }
        
        // Check overall connectivity status
        let overall_connectivity_status = self.verify_general_connectivity(instance_id).await.unwrap_or(false);
        
        // Generate recommendations based on verification results
        let recommendations = self.generate_fix_recommendations(&verification_results).await?;
        
        Ok(FixEffectivenessReport {
            instance_id: instance_id.to_string(),
            total_fixes_applied: applied_fixes.len() as u32,
            successful_fixes,
            failed_fixes,
            verification_results,
            overall_connectivity_status,
            recommendations,
            report_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    async fn generate_fix_recommendations(&self, verification_results: &[FixVerificationResult]) -> Result<Vec<String>> {
        info!("Generating fix recommendations based on {} verification results", verification_results.len());
        
        let mut recommendations = Vec::new();
        
        // Analyze verification results to generate recommendations
        let mut has_connectivity = false;
        let mut failed_verifications = Vec::new();
        
        for result in verification_results {
            if result.connectivity_restored {
                has_connectivity = true;
            } else {
                failed_verifications.push(result);
            }
        }
        
        if has_connectivity {
            recommendations.push("✅ Connection successfully restored for this instance".to_string());
        } else {
            recommendations.push("⚠️  Connection not yet restored - additional fixes may be needed".to_string());
        }
        
        // Generate specific recommendations based on failed verifications
        for failed in &failed_verifications {
            match failed.fix_type.as_str() {
                "instance_start" => {
                    recommendations.push("🔄 Instance Start Issues:".to_string());
                    recommendations.push("  - Verify instance is in 'running' state".to_string());
                    recommendations.push("  - Check instance system logs for startup errors".to_string());
                    recommendations.push("  - Ensure instance has sufficient resources".to_string());
                }
                "ssm_agent" => {
                    recommendations.push("🔄 SSM Agent Issues:".to_string());
                    recommendations.push("  - Wait 2-3 minutes for SSM agent to fully restart".to_string());
                    recommendations.push("  - Check IAM instance profile permissions".to_string());
                    recommendations.push("  - Verify VPC endpoints or internet gateway configuration".to_string());
                    recommendations.push("  - Consider manual SSM agent reinstallation".to_string());
                }
                "iam_permissions" => {
                    recommendations.push("🔄 IAM Permission Issues:".to_string());
                    recommendations.push("  - Attach AmazonSSMManagedInstanceCore policy to instance role".to_string());
                    recommendations.push("  - Verify instance profile is properly attached".to_string());
                    recommendations.push("  - Check trust relationship allows EC2 service".to_string());
                    recommendations.push("  - Restart SSM agent after IAM changes".to_string());
                }
                "security_group" => {
                    recommendations.push("🔄 Security Group Issues:".to_string());
                    recommendations.push("  - Add HTTPS outbound rule (port 443) to security groups".to_string());
                    recommendations.push("  - Create VPC endpoints for SSM services if in private subnet".to_string());
                    recommendations.push("  - Check Network ACL rules for subnet".to_string());
                    recommendations.push("  - Verify route table has internet gateway or NAT gateway".to_string());
                }
                _ => {
                    recommendations.push(format!("🔄 General Issues ({}): Check remaining issues in verification details", failed.fix_type));
                }
            }
            
            // Add specific remaining issues
            for issue in &failed.remaining_issues {
                recommendations.push(format!("  - {}", issue));
            }
        }
        
        // Add general troubleshooting recommendations
        if !failed_verifications.is_empty() {
            recommendations.push("".to_string());
            recommendations.push("📋 General Troubleshooting Steps:".to_string());
            recommendations.push("  1. Wait 5-10 minutes for all changes to propagate".to_string());
            recommendations.push("  2. Check AWS CloudTrail logs for any permission errors".to_string());
            recommendations.push("  3. Verify AWS region consistency across all resources".to_string());
            recommendations.push("  4. Test connectivity from a different local port".to_string());
            recommendations.push("  5. Consider using AWS Systems Manager Session Manager console for direct testing".to_string());
        }
        
        Ok(recommendations)
    }

    /// Verify instance start fix effectiveness
    async fn verify_instance_start_fix(&self, instance_id: &str) -> Result<FixVerificationResult> {
        info!("Verifying instance start fix for: {}", instance_id);
        
        let mut verification_details = Vec::new();
        let mut remaining_issues = Vec::new();
        let mut verified = false;
        let mut connectivity_restored = false;
        
        // Check current instance state
        match self.ec2_client.describe_instances().instance_ids(instance_id).send().await {
            Ok(response) => {
                if let Some(reservation) = response.reservations().first() {
                    if let Some(instance) = reservation.instances().first() {
                        let state = instance.state()
                            .and_then(|s| s.name())
                            .map(|n| format!("{:?}", n))
                            .unwrap_or_else(|| "unknown".to_string());
                        
                        verification_details.push(format!("Instance state: {}", state));
                        
                        if state.contains("running") {
                            verified = true;
                            verification_details.push("Instance is running successfully".to_string());
                            
                            // Check SSM connectivity
                            match self.check_ssm_agent_state(instance_id).await {
                                Ok(ssm_state) => {
                                    if ssm_state.registered && ssm_state.ping_status.contains("Online") {
                                        connectivity_restored = true;
                                        verification_details.push("SSM connectivity verified".to_string());
                                    } else {
                                        remaining_issues.push("SSM agent not online yet".to_string());
                                    }
                                }
                                Err(_) => {
                                    remaining_issues.push("Unable to verify SSM connectivity".to_string());
                                }
                            }
                        } else {
                            remaining_issues.push(format!("Instance not in running state: {}", state));
                        }
                    }
                }
            }
            Err(e) => {
                remaining_issues.push(format!("Failed to verify instance state: {}", e));
            }
        }
        
        Ok(FixVerificationResult {
            fix_type: "instance_start".to_string(),
            instance_id: instance_id.to_string(),
            verified,
            verification_details,
            connectivity_restored,
            remaining_issues,
            verification_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Verify SSM agent fix effectiveness
    async fn verify_ssm_agent_fix(&self, instance_id: &str) -> Result<FixVerificationResult> {
        info!("Verifying SSM agent fix for: {}", instance_id);
        
        let mut verification_details = Vec::new();
        let mut remaining_issues = Vec::new();
        let mut verified = false;
        let mut connectivity_restored = false;
        
        // Check SSM agent state
        match self.check_ssm_agent_state(instance_id).await {
            Ok(ssm_state) => {
                verification_details.push(format!("SSM registration: {}", ssm_state.registered));
                verification_details.push(format!("Ping status: {}", ssm_state.ping_status));
                verification_details.push(format!("Agent version: {}", ssm_state.agent_version));
                verification_details.push(format!("Last ping: {}", ssm_state.last_ping_time));
                
                if ssm_state.registered {
                    verified = true;
                    if ssm_state.ping_status.contains("Online") {
                        connectivity_restored = true;
                        verification_details.push("SSM agent is healthy and online".to_string());
                    } else {
                        remaining_issues.push(format!("SSM agent registered but not online: {}", ssm_state.ping_status));
                    }
                } else {
                    remaining_issues.push("SSM agent not registered with Systems Manager".to_string());
                }
            }
            Err(e) => {
                remaining_issues.push(format!("Failed to verify SSM agent state: {}", e));
            }
        }
        
        Ok(FixVerificationResult {
            fix_type: "ssm_agent".to_string(),
            instance_id: instance_id.to_string(),
            verified,
            verification_details,
            connectivity_restored,
            remaining_issues,
            verification_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Verify IAM permissions fix effectiveness
    async fn verify_iam_permissions_fix(&self, instance_id: &str) -> Result<FixVerificationResult> {
        info!("Verifying IAM permissions fix for: {}", instance_id);
        
        let mut verification_details = Vec::new();
        let mut remaining_issues = Vec::new();
        let mut verified = false;
        let mut connectivity_restored = false;
        
        // Get instance details to check IAM instance profile
        match self.get_instance_details(instance_id).await {
            Ok(instance_details) => {
                if let Some(iam_profile) = &instance_details.iam_instance_profile {
                    verification_details.push(format!("IAM instance profile attached: {}", iam_profile));
                    verified = true;
                    
                    // Check if SSM agent can communicate (indicates proper permissions)
                    match self.check_ssm_agent_state(instance_id).await {
                        Ok(ssm_state) => {
                            if ssm_state.registered {
                                connectivity_restored = true;
                                verification_details.push("SSM registration successful - IAM permissions working".to_string());
                            } else {
                                remaining_issues.push("IAM profile attached but SSM registration failed".to_string());
                            }
                        }
                        Err(_) => {
                            remaining_issues.push("Unable to verify SSM connectivity after IAM fix".to_string());
                        }
                    }
                } else {
                    remaining_issues.push("No IAM instance profile attached".to_string());
                }
            }
            Err(e) => {
                remaining_issues.push(format!("Failed to verify instance details: {}", e));
            }
        }
        
        Ok(FixVerificationResult {
            fix_type: "iam_permissions".to_string(),
            instance_id: instance_id.to_string(),
            verified,
            verification_details,
            connectivity_restored,
            remaining_issues,
            verification_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Verify security group fix effectiveness
    async fn verify_security_group_fix(&self, instance_id: &str) -> Result<FixVerificationResult> {
        info!("Verifying security group fix for: {}", instance_id);
        
        let mut verification_details = Vec::new();
        let mut remaining_issues = Vec::new();
        let mut verified = false;
        let mut connectivity_restored = false;
        
        // Get instance details to check security groups
        match self.get_instance_details(instance_id).await {
            Ok(instance_details) => {
                verification_details.push(format!("Security groups count: {}", instance_details.security_groups.len()));
                
                for sg in &instance_details.security_groups {
                    verification_details.push(format!("Security group: {} ({})", sg.group_name, sg.group_id));
                    
                    // Check security group rules
                    match self.check_security_group_rules(&sg.group_id).await {
                        Ok(has_https_outbound) => {
                            if has_https_outbound {
                                verification_details.push(format!("HTTPS outbound rule found in {}", sg.group_id));
                                verified = true;
                            } else {
                                remaining_issues.push(format!("No HTTPS outbound rule in {}", sg.group_id));
                            }
                        }
                        Err(e) => {
                            remaining_issues.push(format!("Failed to check rules for {}: {}", sg.group_id, e));
                        }
                    }
                }
                
                // Test general connectivity if security groups look good
                if verified {
                    match self.verify_general_connectivity(instance_id).await {
                        Ok(connected) => {
                            if connected {
                                connectivity_restored = true;
                                verification_details.push("General connectivity test passed".to_string());
                            } else {
                                remaining_issues.push("Security groups updated but connectivity test failed".to_string());
                            }
                        }
                        Err(e) => {
                            remaining_issues.push(format!("Connectivity test error: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                remaining_issues.push(format!("Failed to verify instance details: {}", e));
            }
        }
        
        Ok(FixVerificationResult {
            fix_type: "security_group".to_string(),
            instance_id: instance_id.to_string(),
            verified,
            verification_details,
            connectivity_restored,
            remaining_issues,
            verification_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Check security group rules for HTTPS outbound access
    async fn check_security_group_rules(&self, group_id: &str) -> Result<bool> {
        debug!("Checking security group rules for: {}", group_id);
        
        match self.ec2_client.describe_security_groups().group_ids(group_id).send().await {
            Ok(response) => {
                if let Some(sg) = response.security_groups().first() {
                    // Check egress rules for HTTPS (port 443)
                    for rule in sg.ip_permissions_egress() {
                        if let Some(from_port) = rule.from_port() {
                            if let Some(to_port) = rule.to_port() {
                                if (from_port <= 443 && to_port >= 443) || (from_port == -1 && to_port == -1) {
                                    debug!("Found HTTPS outbound rule in security group {}", group_id);
                                    return Ok(true);
                                }
                            }
                        }
                    }
                }
                Ok(false)
            }
            Err(e) => {
                error!("Failed to describe security group {}: {}", group_id, e);
                Err(Ec2ConnectError::Aws(AwsError::Ec2ServiceError {
                    message: format!("Failed to describe security group: {}", e)
                }).into())
            }
        }
    }

    /// Verify general connectivity to AWS services
    async fn verify_general_connectivity(&self, instance_id: &str) -> Result<bool> {
        debug!("Verifying general connectivity for instance: {}", instance_id);
        
        // Try to get instance information as a connectivity test
        match self.ec2_client.describe_instances().instance_ids(instance_id).send().await {
            Ok(_) => {
                // Try SSM connectivity test
                match self.ssm_client.describe_instance_information()
                    .instance_information_filter_list(
                        aws_sdk_ssm::types::InstanceInformationFilter::builder()
                            .key(aws_sdk_ssm::types::InstanceInformationFilterKey::InstanceIds)
                            .value_set(instance_id)
                            .build()
                            .map_err(|e| Ec2ConnectError::Aws(AwsError::SsmServiceError {
                                message: format!("Failed to build instance filter: {}", e)
                            }))?
                    ).send().await {
                    Ok(_) => {
                        debug!("General connectivity test passed for instance: {}", instance_id);
                        Ok(true)
                    }
                    Err(e) => {
                        debug!("SSM connectivity test failed: {}", e);
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                debug!("EC2 connectivity test failed: {}", e);
                Ok(false)
            }
        }
    }
    
    async fn verify_fix(&self, instance_id: &str, fix_type: &str) -> Result<bool> {
        info!("Verifying fix for instance {} with fix type: {}", instance_id, fix_type);
        
        let verification_result = match fix_type {
            "instance_start" => self.verify_instance_start_fix(instance_id).await?,
            "ssm_agent" => self.verify_ssm_agent_fix(instance_id).await?,
            "iam_permissions" => self.verify_iam_permissions_fix(instance_id).await?,
            "security_group" => self.verify_security_group_fix(instance_id).await?,
            _ => {
                info!("Unknown fix type: {}, performing general connectivity test", fix_type);
                let connectivity = self.verify_general_connectivity(instance_id).await?;
                return Ok(connectivity);
            }
        };
        
        info!("Verification result for {} ({}): verified={}, connectivity_restored={}", 
              instance_id, fix_type, verification_result.verified, verification_result.connectivity_restored);
        
        Ok(verification_result.verified && verification_result.connectivity_restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_action_creation() {
        let action = FixAction::new(
            FixActionType::StartInstance,
            "Start the instance".to_string(),
            "i-1234567890abcdef0".to_string(),
        );

        assert_eq!(action.action_type, FixActionType::StartInstance);
        assert_eq!(action.description, "Start the instance");
        assert_eq!(action.target_resource, "i-1234567890abcdef0");
        assert_eq!(action.risk_level, RiskLevel::Low);
        assert!(action.requires_confirmation);
    }

    #[test]
    fn test_fix_action_safety() {
        let safe_action = FixAction::new(
            FixActionType::UpdateCredentials,
            "Update credentials".to_string(),
            "credentials".to_string(),
        );
        assert!(safe_action.is_safe_to_auto_execute());

        let unsafe_action = FixAction::new(
            FixActionType::StartInstance,
            "Start instance".to_string(),
            "i-1234567890abcdef0".to_string(),
        );
        assert!(!unsafe_action.is_safe_to_auto_execute());
    }

    #[test]
    fn test_fix_result_creation() {
        let action = FixAction::new(
            FixActionType::UpdateCredentials,
            "Update credentials".to_string(),
            "credentials".to_string(),
        );

        let result = FixResult::success(
            action.clone(),
            "Credentials updated".to_string(),
            Duration::from_millis(100),
        );

        assert!(result.success);
        assert_eq!(result.message, "Credentials updated");
        assert_eq!(result.action.action_type, FixActionType::UpdateCredentials);
    }

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Safe < RiskLevel::Low);
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }
}