use async_trait::async_trait;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_ssm::Client as SsmClient;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{debug, error, info};

use crate::aws::load_aws_config;
use crate::diagnostic::{DiagnosticResult, DiagnosticStatus};
use crate::error::{AwsError, NimbusError, Result};

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
    pub fn new(action_type: FixActionType, description: String, target_resource: String) -> Self {
        let (requires_confirmation, risk_level, estimated_duration) = match action_type {
            // Task 26.1: instance start is auto-executed without user approval.
            FixActionType::StartInstance => (false, RiskLevel::Low, Duration::from_secs(300)),
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
        let config = load_aws_config(None, None).await;
        let ec2_client = Ec2Client::new(&config);
        let ssm_client = SsmClient::new(&config);

        Ok(Self {
            ec2_client,
            ssm_client,
            dry_run: false,
        })
    }

    /// Create a new auto-fix manager with custom AWS configuration
    pub async fn with_aws_config(
        region: Option<String>,
        profile: Option<String>,
    ) -> anyhow::Result<Self> {
        let config = load_aws_config(region.as_deref(), profile.as_deref()).await;
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

        debug!(
            "analyze_instance_fixes: status={:?}, message={}",
            result.status, result.message
        );

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("stopped") || result.message.contains("stopping") {
                debug!("Found stopped/stopping instance, extracting instance_id from details");
                let instance_id = result
                    .details
                    .as_ref()
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
                    "Instance is terminated and cannot be recovered. Launch a new instance."
                        .to_string(),
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
                let instance_id = result
                    .details
                    .as_ref()
                    .and_then(|d| d.get("instance_id"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("unknown");

                let fix = FixAction::new(
                    FixActionType::RestartSsmAgent,
                    format!("Restart SSM agent on instance: {}", instance_id),
                    instance_id.to_string(),
                )
                .with_command("sudo systemctl restart amazon-ssm-agent".to_string());
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
            } else if result.message.contains("permissions")
                || result.message.contains("access denied")
            {
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
                    "Security group rules need to be updated to allow HTTPS outbound traffic"
                        .to_string(),
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
                        let process_name = process_info
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown");
                        let pid = process_info
                            .get("pid")
                            .and_then(|p| p.as_u64())
                            .unwrap_or(0);

                        let fix = FixAction::new(
                            FixActionType::TerminateProcess,
                            format!(
                                "Terminate process {} (PID: {}) that is using the port",
                                process_name, pid
                            ),
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
        self.execute_start_instance_with_monitoring(instance_id, false)
            .await
    }

    /// Execute instance start fix with user approval and progress monitoring
    async fn execute_start_instance_with_monitoring(
        &mut self,
        instance_id: &str,
        user_approved: bool,
    ) -> Result<FixResult> {
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

        // Task 26.1: user approval is not required for instance start.
        // The parameter is retained for API compatibility.
        let _ = user_approved;

        // First, get current instance state
        let describe_request = self
            .ec2_client
            .describe_instances()
            .instance_ids(instance_id);

        let current_state = match describe_request.send().await {
            Ok(response) => {
                if let Some(reservation) = response.reservations().first() {
                    if let Some(instance) = reservation.instances().first() {
                        instance
                            .state()
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

        // If instance is already running, proceed to SSM registration wait (task 26.2)
        if current_state.contains("running") {
            match self
                .wait_for_ssm_registration_with_progress(instance_id)
                .await
            {
                Ok(state) => {
                    return Ok(FixResult::success(
                        action,
                        format!(
                            "Instance is already running; SSM registration confirmed (ping_status: {}, agent_version: {})",
                            state.ping_status, state.agent_version
                        ),
                        start_time.elapsed(),
                    ));
                }
                Err(e) => {
                    return Ok(FixResult::failure(
                        action,
                        format!(
                            "Instance is already running, but SSM registration did not complete: {}\n\n{}",
                            e,
                            self.ssm_registration_troubleshooting_instructions(instance_id)
                        ),
                        start_time.elapsed(),
                    ));
                }
            }
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
        let request = self.ec2_client.start_instances().instance_ids(instance_id);

        match request.send().await {
            Ok(response) => {
                let starting_instances = response.starting_instances();
                if let Some(instance) = starting_instances.first() {
                    let new_state = instance
                        .current_state()
                        .and_then(|s| s.name())
                        .map(|n| format!("{:?}", n))
                        .unwrap_or_else(|| "unknown".to_string());

                    info!(
                        "Instance {} start initiated, new state: {}",
                        instance_id, new_state
                    );

                    // Monitor startup progress
                    let monitoring_result = self.monitor_instance_startup(instance_id).await;

                    match monitoring_result {
                        Ok(final_state) => {
                            // Task 26.2: wait for SSM managed instance registration after start
                            match self.wait_for_ssm_registration_with_progress(instance_id).await {
                                Ok(state) => Ok(FixResult::success(
                                    action,
                                    format!(
                                        "Instance started successfully (state: {}). SSM registration confirmed (ping_status: {}, agent_version: {})",
                                        final_state, state.ping_status, state.agent_version
                                    ),
                                    start_time.elapsed(),
                                )),
                                Err(e) => Ok(FixResult::failure(
                                    action,
                                    format!(
                                        "Instance started successfully (state: {}), but SSM registration did not complete: {}\n\n{}",
                                        final_state,
                                        e,
                                        self.ssm_registration_troubleshooting_instructions(instance_id)
                                    ),
                                    start_time.elapsed(),
                                )),
                            }
                        }
                        Err(e) => Ok(FixResult::success(
                            action,
                            format!("Instance start initiated but monitoring failed: {}", e),
                            start_time.elapsed(),
                        )),
                    }
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
        let mut last_state: Option<String> = None;

        loop {
            if start_time.elapsed() > max_wait_time {
                return Err(NimbusError::Aws(AwsError::Timeout {
                    operation: "Instance startup monitoring".to_string(),
                })
                .into());
            }

            let describe_request = self
                .ec2_client
                .describe_instances()
                .instance_ids(instance_id);

            match describe_request.send().await {
                Ok(response) => {
                    if let Some(reservation) = response.reservations().first() {
                        if let Some(instance) = reservation.instances().first() {
                            let state = instance
                                .state()
                                .and_then(|s| s.name())
                                .map(|n| format!("{:?}", n))
                                .unwrap_or_else(|| "unknown".to_string());

                            // Show progress at info-level (task 26.1 requirement: progress display)
                            if last_state.as_deref() != Some(state.as_str()) {
                                info!(
                                    "Instance {} state: {} (elapsed: {}s)",
                                    instance_id,
                                    state,
                                    start_time.elapsed().as_secs()
                                );
                                last_state = Some(state.clone());
                            } else {
                                debug!("Instance {} current state: {}", instance_id, state);
                            }

                            if state.contains("running") {
                                info!("Instance {} is now running", instance_id);
                                return Ok(state);
                            } else if state.contains("stopped") || state.contains("stopping") {
                                return Err(NimbusError::Aws(AwsError::Ec2ServiceError {
                                    message: format!(
                                        "Instance startup failed, current state: {}",
                                        state
                                    ),
                                })
                                .into());
                            }

                            // Continue monitoring for pending/starting states
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to check instance state during monitoring: {}", e);
                    return Err(NimbusError::Aws(AwsError::Ec2ServiceError {
                        message: format!("Failed to check instance state: {}", e),
                    })
                    .into());
                }
            }

            tokio::time::sleep(check_interval).await;
        }
    }

    /// Task 26.2: Wait for SSM managed instance registration after instance start.
    /// - Check interval: 3 seconds
    /// - Default timeout: 5 minutes
    /// - Emits progress with elapsed time
    async fn wait_for_ssm_registration_with_progress(
        &self,
        instance_id: &str,
    ) -> Result<SsmAgentState> {
        let timeout = Duration::from_secs(300);
        let check_interval = Duration::from_secs(3);
        let start_time = Instant::now();

        info!(
            "Waiting for SSM registration for instance: {} (timeout: {}s, interval: {}s)",
            instance_id,
            timeout.as_secs(),
            check_interval.as_secs()
        );

        loop {
            if start_time.elapsed() > timeout {
                return Err(NimbusError::Aws(AwsError::Timeout {
                    operation: "SSM registration wait".to_string(),
                })
                .into());
            }

            let state = self.check_ssm_agent_state(instance_id).await?;

            if state.registered {
                info!(
                    "SSM registration confirmed for {} (ping_status: {}, agent_version: {}, elapsed: {}s)",
                    instance_id,
                    state.ping_status,
                    state.agent_version,
                    start_time.elapsed().as_secs()
                );
                return Ok(state);
            }

            info!(
                "Waiting for SSM registration... instance: {}, elapsed: {}s",
                instance_id,
                start_time.elapsed().as_secs()
            );

            tokio::time::sleep(check_interval).await;
        }
    }

    fn ssm_registration_troubleshooting_instructions(&self, instance_id: &str) -> String {
        format!(
            "Troubleshooting steps for SSM registration timeout (instance: {}):\n\
             1) Verify the instance has an IAM role with AmazonSSMManagedInstanceCore attached\n\
             2) Verify outbound HTTPS (TCP/443) is allowed (Security Group / NACL / proxy)\n\
             3) If in a private subnet, verify VPC Endpoints exist for: ssm, ssmmessages, ec2messages\n\
             4) Verify the SSM agent is installed and running (Linux):\n\
                - sudo systemctl status amazon-ssm-agent\n\
                - sudo systemctl restart amazon-ssm-agent\n\
             5) Verify registration via CLI:\n\
                - aws ssm describe-instance-information --filters Key=InstanceIds,Values={}\n\
             6) Check instance time sync and DNS resolution (SSM requires correct time/DNS)\n\
             7) Review CloudWatch / system logs for amazon-ssm-agent errors",
            instance_id,
            instance_id
        )
    }

    /// Execute SSM agent restart fix with enhanced verification
    async fn execute_restart_ssm_agent(&mut self, instance_id: &str) -> Result<FixResult> {
        self.execute_restart_ssm_agent_with_verification(instance_id)
            .await
    }

    /// Execute SSM agent restart fix with detailed state checking and verification
    async fn execute_restart_ssm_agent_with_verification(
        &mut self,
        instance_id: &str,
    ) -> Result<FixResult> {
        let start_time = Instant::now();
        let action = FixAction::new(
            FixActionType::RestartSsmAgent,
            format!("Restarting SSM agent on instance: {}", instance_id),
            instance_id.to_string(),
        );

        info!(
            "Executing enhanced SSM agent restart fix for: {}",
            instance_id
        );

        if self.dry_run {
            info!(
                "DRY RUN: Would restart SSM agent on instance {}",
                instance_id
            );
            return Ok(FixResult::success(
                action,
                format!(
                    "DRY RUN: Would restart SSM agent on instance {}",
                    instance_id
                ),
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
                format!(
                    "SSM agent restarted successfully and is healthy. Command ID: {}",
                    restart_result
                )
            }
            SsmAgentHealthStatus::Unhealthy(reason) => {
                format!(
                    "SSM agent restart completed but agent is unhealthy: {}. Command ID: {}",
                    reason, restart_result
                )
            }
            SsmAgentHealthStatus::Unknown => {
                format!(
                    "SSM agent restart completed but health status unknown. Command ID: {}",
                    restart_result
                )
            }
        };

        let success = matches!(verification_result, SsmAgentHealthStatus::Healthy);

        if success {
            Ok(FixResult::success(
                action,
                final_message,
                start_time.elapsed(),
            ))
        } else {
            Ok(FixResult::failure(
                action,
                final_message,
                start_time.elapsed(),
            ))
        }
    }

    /// Check SSM agent state on the instance
    async fn check_ssm_agent_state(&self, instance_id: &str) -> Result<SsmAgentState> {
        info!("Checking SSM agent state for instance: {}", instance_id);

        // Check if instance is registered with SSM
        let describe_request = self
            .ssm_client
            .describe_instance_information()
            .instance_information_filter_list(
                aws_sdk_ssm::types::InstanceInformationFilter::builder()
                    .key(aws_sdk_ssm::types::InstanceInformationFilterKey::InstanceIds)
                    .value_set(instance_id)
                    .build()
                    .map_err(|e| {
                        NimbusError::Aws(AwsError::SsmServiceError {
                            message: format!("Failed to build instance filter: {}", e),
                        })
                    })?,
            );

        match describe_request.send().await {
            Ok(response) => {
                if let Some(instance_info) = response.instance_information_list().first() {
                    let ping_status = instance_info
                        .ping_status()
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_else(|| "Unknown".to_string());

                    let agent_version = instance_info
                        .agent_version()
                        .unwrap_or("Unknown")
                        .to_string();

                    let last_ping_time = instance_info
                        .last_ping_date_time()
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
                error!(
                    "Failed to check SSM agent state for instance {}: {}",
                    instance_id, e
                );
                Err(NimbusError::Aws(AwsError::SsmServiceError {
                    message: format!("Failed to check SSM agent state: {}", e),
                })
                .into())
            }
        }
    }

    /// Send SSM restart command to the instance
    async fn send_ssm_restart_command(&self, instance_id: &str) -> Result<String> {
        info!("Sending SSM restart command to instance: {}", instance_id);

        let request = self
            .ssm_client
            .send_command()
            .instance_ids(instance_id)
            .document_name("AWS-RunShellScript")
            .parameters(
                "commands",
                vec![
                    "sudo systemctl restart amazon-ssm-agent".to_string(),
                    "sleep 5".to_string(),
                    "sudo systemctl status amazon-ssm-agent".to_string(),
                ],
            );

        match request.send().await {
            Ok(response) => {
                if let Some(command) = response.command() {
                    let command_id = command.command_id().unwrap_or("unknown");
                    info!(
                        "SSM restart command sent successfully, command ID: {}",
                        command_id
                    );
                    Ok(command_id.to_string())
                } else {
                    error!("No command information returned");
                    Err(NimbusError::Aws(AwsError::SsmServiceError {
                        message: "No command information returned".to_string(),
                    })
                    .into())
                }
            }
            Err(e) => {
                error!(
                    "Failed to send SSM restart command to instance {}: {}",
                    instance_id, e
                );
                Err(NimbusError::Aws(AwsError::SsmServiceError {
                    message: format!("Failed to send SSM restart command: {}", e),
                })
                .into())
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
                        debug!(
                            "SSM agent registered but not online yet: {}",
                            state.ping_status
                        );
                    } else {
                        debug!("SSM agent not registered yet");
                    }
                }
                Err(e) => {
                    debug!("Error checking SSM agent state during verification: {}", e);
                    return Ok(SsmAgentHealthStatus::Unhealthy(format!(
                        "Health check failed: {}",
                        e
                    )));
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
        let config = load_aws_config(None, None).await;

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
        info!(
            "Analyzing {} diagnostic results for potential fixes",
            diagnostics.len()
        );

        let mut all_fixes = Vec::new();

        for result in diagnostics {
            debug!(
                "Analyzing fixes for diagnostic item: {} (status: {:?}, message: {})",
                result.item_name, result.status, result.message
            );

            let fixes = match result.item_name.as_str() {
                "instance_state" | "detailed_instance_state" => {
                    debug!("Matched instance state diagnostic, calling analyze_instance_fixes");
                    let instance_fixes = self.analyze_instance_fixes(result);
                    debug!("Generated {} instance fixes", instance_fixes.len());
                    instance_fixes
                }
                "ssm_agent" | "ssm_agent_enhanced" => self.analyze_ssm_agent_fixes(result),
                "iam_permissions" => self.analyze_iam_fixes(result),
                "vpc_endpoints" | "security_groups" | "network_connectivity" => {
                    self.analyze_network_fixes(result)
                }
                "local_port_availability" => self.analyze_port_fixes(result),
                _ => {
                    debug!(
                        "No specific fix analysis for diagnostic item: {}",
                        result.item_name
                    );
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
        info!(
            "Executing fix action: {:?} for target: {}",
            action.action_type, action.target_resource
        );

        match action.action_type {
            FixActionType::StartInstance => {
                self.execute_start_instance(&action.target_resource).await
            }
            FixActionType::RestartSsmAgent => {
                self.execute_restart_ssm_agent(&action.target_resource)
                    .await
            }
            FixActionType::UpdateCredentials => self.execute_update_credentials().await,
            FixActionType::TerminateProcess => {
                self.execute_terminate_process(&action.target_resource)
                    .await
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
            FixActionType::CreateVpcEndpoint
            | FixActionType::UpdateSecurityGroup
            | FixActionType::SuggestManualFix => {
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
                info!(
                    "Skipping non-safe fix: {:?} (requires confirmation or high risk)",
                    action.action_type
                );
                let start_time = Instant::now();
                let result = FixResult::success(
                    action,
                    "Skipped - requires manual confirmation".to_string(),
                    start_time.elapsed(),
                );
                results.push(result);
            }
        }

        info!(
            "Completed execution of safe fixes, {} results",
            results.len()
        );
        Ok(results)
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
        assert!(!action.requires_confirmation);
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
        // Task 26.1: instance start is safe to auto-execute (low risk, no confirmation)
        assert!(unsafe_action.is_safe_to_auto_execute());
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
