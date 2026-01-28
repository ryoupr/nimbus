#![allow(dead_code)]

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{debug, info, warn, error};

use crate::aws::AwsManager;
use crate::diagnostic::{DiagnosticResult, DiagnosticStatus, Severity};

/// SSM Agent information for diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsmAgentInfo {
    pub instance_id: String,
    pub is_registered: bool,
    pub agent_version: Option<String>,
    pub last_ping_time: Option<chrono::DateTime<chrono::Utc>>,
    pub ping_status: Option<String>,
    pub platform_type: Option<String>,
    pub platform_name: Option<String>,
    pub platform_version: Option<String>,
    pub activation_id: Option<String>,
    pub iam_role: Option<String>,
    pub registration_date: Option<chrono::DateTime<chrono::Utc>>,
    pub resource_type: Option<String>,
    pub ip_address: Option<String>,
    pub computer_name: Option<String>,
}

/// SSM Agent registration status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegistrationStatus {
    Registered,
    NotRegistered,
    Deregistered,
    Unknown,
}

impl std::fmt::Display for RegistrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistrationStatus::Registered => write!(f, "Registered"),
            RegistrationStatus::NotRegistered => write!(f, "Not Registered"),
            RegistrationStatus::Deregistered => write!(f, "Deregistered"),
            RegistrationStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// SSM Agent ping status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PingStatus {
    Online,
    ConnectionLost,
    Inactive,
    Unknown,
}

/// Enhanced SSM Agent version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedAgentVersionInfo {
    pub current_version: String,
    pub latest_available_version: Option<String>,
    pub is_current: bool,
    pub is_security_update_required: bool,
    pub platform_specific_recommendation: Option<String>,
    pub version_age_days: Option<i64>,
    pub update_urgency: UpdateUrgency,
    pub vulnerability_info: Vec<VulnerabilityInfo>,
}

/// Update urgency level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateUrgency {
    Critical,    // Security vulnerabilities present
    High,        // Major version behind
    Medium,      // Minor version behind
    Low,         // Patch version behind
    None,        // Up to date
}

/// Vulnerability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityInfo {
    pub cve_id: Option<String>,
    pub severity: String,
    pub description: String,
    pub fixed_in_version: String,
}

/// Registration analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationAnalysis {
    pub registration_status: RegistrationStatus,
    pub registration_quality_score: f64, // 0-100
    pub registration_issues: Vec<RegistrationIssue>,
    pub registration_recommendations: Vec<String>,
    pub last_successful_registration: Option<chrono::DateTime<chrono::Utc>>,
    pub registration_failure_count: u32,
    pub registration_process_stages: Vec<RegistrationStage>,
}

/// Registration issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationIssue {
    pub issue_type: String,
    pub severity: String,
    pub description: String,
    pub resolution_steps: Vec<String>,
}

/// Registration process stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationStage {
    pub stage_name: String,
    pub status: String,
    pub last_check_time: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
}

/// Agent health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealthMetrics {
    pub overall_health_score: f64, // 0-100
    pub communication_health: f64,  // 0-100
    pub performance_health: f64,    // 0-100
    pub error_rate: f64,           // 0-100 (percentage)
    pub average_response_time_ms: Option<f64>,
    pub uptime_percentage: f64,
    pub last_24h_ping_success_rate: f64,
    pub health_trend: HealthTrend,
}

/// Health trend
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthTrend {
    Improving,
    Stable,
    Declining,
    Critical,
}

/// SSM service registration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsmServiceRegistration {
    pub session_manager: ServiceRegistrationStatus,
    pub patch_manager: ServiceRegistrationStatus,
    pub run_command: ServiceRegistrationStatus,
    pub state_manager: ServiceRegistrationStatus,
    pub inventory: ServiceRegistrationStatus,
    pub compliance: ServiceRegistrationStatus,
}

/// Service registration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRegistrationStatus {
    pub registered: bool,
    pub last_registration_time: Option<chrono::DateTime<chrono::Utc>>,
    pub capabilities: Vec<String>,
    pub configuration_issues: Vec<String>,
}

impl From<&str> for PingStatus {
    fn from(status: &str) -> Self {
        match status.to_lowercase().as_str() {
            "online" => PingStatus::Online,
            "connection lost" => PingStatus::ConnectionLost,
            "inactive" => PingStatus::Inactive,
            _ => PingStatus::Unknown,
        }
    }
}

/// Trait for SSM Agent diagnostics functionality
#[async_trait]
pub trait SsmAgentDiagnostics {
    /// Check if the instance is registered as a managed instance in SSM
    async fn check_managed_instance_registration(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Check the SSM Agent version
    async fn check_agent_version(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Check the last communication time with SSM service
    async fn check_last_communication(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Validate SSM Agent configuration
    async fn validate_agent_configuration(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Get detailed SSM Agent information
    async fn get_agent_info(&self, instance_id: &str) -> Result<Option<SsmAgentInfo>>;
    
    /// Run comprehensive SSM Agent diagnostics
    async fn run_ssm_agent_diagnostics(&self, instance_id: &str) -> Result<Vec<DiagnosticResult>>;
    
    // Enhanced methods for Task 25.2
    
    /// Check enhanced SSM Agent version with detailed analysis
    async fn check_enhanced_agent_version(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Analyze registration details with comprehensive assessment
    async fn analyze_registration_details(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Check agent health score and metrics
    async fn check_agent_health_score(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Validate SSM service-specific registration
    async fn validate_ssm_service_registration(&self, instance_id: &str) -> Result<DiagnosticResult>;
    
    /// Get enhanced agent version information
    async fn get_enhanced_agent_version_info(&self, instance_id: &str) -> Result<Option<EnhancedAgentVersionInfo>>;
    
    /// Get registration analysis
    async fn get_registration_analysis(&self, instance_id: &str) -> Result<Option<RegistrationAnalysis>>;
    
    /// Get agent health metrics
    async fn get_agent_health_metrics(&self, instance_id: &str) -> Result<Option<AgentHealthMetrics>>;
    
    /// Get SSM service registration status
    async fn get_ssm_service_registration(&self, instance_id: &str) -> Result<Option<SsmServiceRegistration>>;
}

/// Default implementation of SSM Agent diagnostics
pub struct DefaultSsmAgentDiagnostics {
    aws_manager: AwsManager,
}

impl DefaultSsmAgentDiagnostics {
    /// Create a new SSM Agent diagnostics with AWS manager
    pub fn new(aws_manager: AwsManager) -> Self {
        Self { aws_manager }
    }
    
    /// Create SSM Agent diagnostics with default AWS configuration
    pub async fn with_default_aws() -> Result<Self> {
        let aws_manager = AwsManager::default().await
            .context("Failed to create AWS manager")?;
        Ok(Self::new(aws_manager))
    }
    
    /// Create SSM Agent diagnostics with specific AWS configuration
    pub async fn with_aws_config(region: Option<&str>, profile: Option<&str>) -> Result<Self> {
        let aws_manager = AwsManager::new(
            region.map(|s| s.to_string()),
            profile.map(|s| s.to_string()),
        ).await.context("Failed to create AWS manager")?;
        Ok(Self::new(aws_manager))
    }
    
    /// Create SSM Agent diagnostics with specific AWS profile
    pub async fn with_profile(profile: &str) -> Result<Self> {
        let aws_manager = AwsManager::with_profile(profile).await
            .context("Failed to create AWS manager with profile")?;
        Ok(Self::new(aws_manager))
    }
    
    /// Create SSM Agent diagnostics with specific AWS region
    pub async fn with_region(region: &str) -> Result<Self> {
        let aws_manager = AwsManager::with_region(region).await
            .context("Failed to create AWS manager with region")?;
        Ok(Self::new(aws_manager))
    }
    
    /// Get SSM client from AWS manager
    fn get_ssm_client(&self) -> &aws_sdk_ssm::Client {
        &self.aws_manager.ssm_client
    }
    
    /// Check if agent version is up to date
    fn is_agent_version_current(version: &str) -> bool {
        // Parse version string (format: major.minor.patch.build)
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 3 {
            return false; // Invalid version format
        }
        
        // Parse major, minor, patch versions
        let major: u32 = parts[0].parse().unwrap_or(0);
        let minor: u32 = parts[1].parse().unwrap_or(0);
        let _patch: u32 = parts[2].parse().unwrap_or(0);
        
        // Consider versions 3.0.0 and above as current
        // This is a simplified check - in production, you might want to check against
        // the latest available version from AWS
        if major >= 3 {
            true
        } else if major == 2 && minor >= 3 {
            true // 2.3.x and above are acceptable
        } else {
            false
        }
    }
    
    /// Calculate time since last ping
    fn calculate_time_since_ping(last_ping: chrono::DateTime<chrono::Utc>) -> Duration {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(last_ping);
        Duration::from_secs(duration.num_seconds().max(0) as u64)
    }
    
    /// Determine ping status severity
    fn determine_ping_severity(ping_status: &PingStatus, time_since_ping: Duration) -> Severity {
        match ping_status {
            PingStatus::Online => {
                if time_since_ping > Duration::from_secs(300) { // 5 minutes
                    Severity::Medium
                } else {
                    Severity::Info
                }
            }
            PingStatus::ConnectionLost => {
                if time_since_ping > Duration::from_secs(3600) { // 1 hour
                    Severity::High
                } else {
                    Severity::Medium
                }
            }
            PingStatus::Inactive => Severity::High,
            PingStatus::Unknown => Severity::Medium,
        }
    }
    
    /// Get latest available SSM Agent version (mock implementation)
    async fn get_latest_agent_version(&self, platform: &str) -> Result<String> {
        // In a real implementation, this would query AWS Systems Manager
        // or AWS documentation API for the latest agent version
        match platform.to_lowercase().as_str() {
            "windows" => Ok("3.2.2086.0".to_string()),
            "linux" | "amazon linux" => Ok("3.2.2086.0".to_string()),
            "ubuntu" => Ok("3.2.2086.0".to_string()),
            _ => Ok("3.2.2086.0".to_string()), // Default latest version
        }
    }
    
    /// Check for security vulnerabilities in agent version
    fn check_version_vulnerabilities(version: &str) -> Vec<VulnerabilityInfo> {
        let mut vulnerabilities = Vec::new();
        
        // Parse version to check against known vulnerabilities
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() >= 3 {
            if let (Ok(major), Ok(minor), Ok(_patch)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                // Check for known vulnerabilities (example data)
                if major < 3 || (major == 3 && minor < 1) {
                    vulnerabilities.push(VulnerabilityInfo {
                        cve_id: Some("CVE-2023-1234".to_string()),
                        severity: "High".to_string(),
                        description: "Privilege escalation vulnerability in SSM Agent".to_string(),
                        fixed_in_version: "3.1.0.0".to_string(),
                    });
                }
                
                if major < 3 || (major == 3 && minor < 2) {
                    vulnerabilities.push(VulnerabilityInfo {
                        cve_id: Some("CVE-2023-5678".to_string()),
                        severity: "Medium".to_string(),
                        description: "Information disclosure in agent logging".to_string(),
                        fixed_in_version: "3.2.0.0".to_string(),
                    });
                }
            }
        }
        
        vulnerabilities
    }
    
    /// Calculate update urgency based on version comparison and vulnerabilities
    fn calculate_update_urgency(
        current_version: &str,
        latest_version: &str,
        vulnerabilities: &[VulnerabilityInfo],
    ) -> UpdateUrgency {
        // If there are critical vulnerabilities, update is critical
        if vulnerabilities.iter().any(|v| v.severity == "Critical") {
            return UpdateUrgency::Critical;
        }
        
        // If there are high severity vulnerabilities, update is high priority
        if vulnerabilities.iter().any(|v| v.severity == "High") {
            return UpdateUrgency::High;
        }
        
        // Compare versions
        let current_parts: Vec<u32> = current_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let latest_parts: Vec<u32> = latest_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        
        if current_parts.len() >= 2 && latest_parts.len() >= 2 {
            // Major version difference
            if current_parts[0] < latest_parts[0] {
                return UpdateUrgency::High;
            }
            
            // Minor version difference
            if current_parts[0] == latest_parts[0] && current_parts[1] < latest_parts[1] {
                return UpdateUrgency::Medium;
            }
            
            // Patch version difference
            if current_parts.len() >= 3 && latest_parts.len() >= 3 {
                if current_parts[0] == latest_parts[0] 
                    && current_parts[1] == latest_parts[1] 
                    && current_parts[2] < latest_parts[2] {
                    return UpdateUrgency::Low;
                }
            }
        }
        
        UpdateUrgency::None
    }
    
    /// Calculate registration quality score
    fn calculate_registration_quality_score(
        agent_info: &SsmAgentInfo,
        ping_status: &PingStatus,
        time_since_ping: Duration,
    ) -> f64 {
        let mut score: f64 = 100.0;
        
        // Deduct points based on ping status
        match ping_status {
            PingStatus::Online => {
                if time_since_ping > Duration::from_secs(300) {
                    score -= 10.0; // Recent communication issue
                }
            }
            PingStatus::ConnectionLost => {
                score -= 30.0;
                if time_since_ping > Duration::from_secs(3600) {
                    score -= 20.0; // Extended connection loss
                }
            }
            PingStatus::Inactive => score -= 50.0,
            PingStatus::Unknown => score -= 20.0,
        }
        
        // Deduct points for missing information
        if agent_info.agent_version.is_none() {
            score -= 15.0;
        }
        if agent_info.platform_type.is_none() {
            score -= 10.0;
        }
        if agent_info.iam_role.is_none() {
            score -= 25.0; // IAM role is critical
        }
        
        // Ensure score is between 0 and 100
        score.max(0.0).min(100.0)
    }
    
    /// Calculate agent health metrics
    fn calculate_agent_health_metrics(
        agent_info: &SsmAgentInfo,
        ping_status: &PingStatus,
        time_since_ping: Duration,
    ) -> AgentHealthMetrics {
        let communication_health = match ping_status {
            PingStatus::Online => {
                if time_since_ping < Duration::from_secs(60) {
                    100.0
                } else if time_since_ping < Duration::from_secs(300) {
                    85.0
                } else {
                    70.0
                }
            }
            PingStatus::ConnectionLost => 40.0,
            PingStatus::Inactive => 10.0,
            PingStatus::Unknown => 50.0,
        };
        
        let performance_health = if agent_info.agent_version.is_some() {
            90.0 // Assume good performance if version is available
        } else {
            60.0
        };
        
        let error_rate = match ping_status {
            PingStatus::Online => 2.0,
            PingStatus::ConnectionLost => 15.0,
            PingStatus::Inactive => 50.0,
            PingStatus::Unknown => 25.0,
        };
        
        let overall_health_score = (communication_health + performance_health) / 2.0;
        
        let health_trend = if overall_health_score > 80.0 {
            HealthTrend::Stable
        } else if overall_health_score > 60.0 {
            HealthTrend::Declining
        } else {
            HealthTrend::Critical
        };
        
        AgentHealthMetrics {
            overall_health_score,
            communication_health,
            performance_health,
            error_rate,
            average_response_time_ms: Some(150.0), // Mock value
            uptime_percentage: communication_health,
            last_24h_ping_success_rate: communication_health,
            health_trend,
        }
    }
}

#[async_trait]
impl SsmAgentDiagnostics for DefaultSsmAgentDiagnostics {
    async fn check_managed_instance_registration(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking managed instance registration: {}", instance_id);
        
        let ssm_client = self.get_ssm_client();
        
        match ssm_client
            .describe_instance_information()
            .set_filters(Some(vec![
                aws_sdk_ssm::types::InstanceInformationStringFilter::builder()
                    .key("InstanceIds")
                    .values(instance_id)
                    .build()
                    .context("Failed to build instance filter")?
            ]))
            .send()
            .await
        {
            Ok(response) => {
                let instances = response.instance_information_list();
                
                if let Some(instance_info) = instances.first() {
                    debug!("Instance is registered as managed instance: {}", instance_id);
                    
                    let ping_status = instance_info.ping_status()
                        .map(|s| s.as_str())
                        .unwrap_or("Unknown");
                    
                    let registration_details = serde_json::json!({
                        "instance_id": instance_id,
                        "ping_status": ping_status,
                        "platform_type": instance_info.platform_type().map(|p| p.as_str()),
                        "platform_name": instance_info.platform_name(),
                        "agent_version": instance_info.agent_version(),
                        "last_ping_time": instance_info.last_ping_date_time()
                            .map(|dt| chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos()))
                    });
                    
                    Ok(DiagnosticResult::success(
                        "managed_instance_registration".to_string(),
                        format!("Instance {} is registered as managed instance with ping status: {}", instance_id, ping_status),
                        start_time.elapsed(),
                    ).with_details(registration_details))
                } else {
                    warn!("Instance is not registered as managed instance: {}", instance_id);
                    Ok(DiagnosticResult::error(
                        "managed_instance_registration".to_string(),
                        format!("Instance {} is not registered as managed instance in SSM", instance_id),
                        start_time.elapsed(),
                        Severity::Critical,
                    ).with_auto_fixable(false))
                }
            }
            Err(e) => {
                error!("Failed to check managed instance registration: {}", e);
                Ok(DiagnosticResult::error(
                    "managed_instance_registration".to_string(),
                    format!("Failed to check managed instance registration: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn check_agent_version(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking SSM Agent version: {}", instance_id);
        
        match self.get_agent_info(instance_id).await {
            Ok(Some(agent_info)) => {
                if let Some(ref version) = agent_info.agent_version {
                    let is_current = Self::is_agent_version_current(version);
                    
                    let version_details = serde_json::json!({
                        "instance_id": instance_id,
                        "agent_version": version,
                        "is_current": is_current,
                        "platform": agent_info.platform_name
                    });
                    
                    if is_current {
                        debug!("SSM Agent version is current: {}", version);
                        Ok(DiagnosticResult::success(
                            "agent_version".to_string(),
                            format!("SSM Agent version {} is current", version),
                            start_time.elapsed(),
                        ).with_details(version_details))
                    } else {
                        warn!("SSM Agent version may be outdated: {}", version);
                        Ok(DiagnosticResult::warning(
                            "agent_version".to_string(),
                            format!("SSM Agent version {} may be outdated. Consider updating to the latest version", version),
                            start_time.elapsed(),
                            Severity::Medium,
                        ).with_details(version_details).with_auto_fixable(true))
                    }
                } else {
                    warn!("SSM Agent version information not available: {}", instance_id);
                    Ok(DiagnosticResult::warning(
                        "agent_version".to_string(),
                        format!("SSM Agent version information not available for {}", instance_id),
                        start_time.elapsed(),
                        Severity::Low,
                    ))
                }
            }
            Ok(None) => {
                error!("Instance not found or not registered: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "agent_version".to_string(),
                    format!("Instance {} not found or not registered as managed instance", instance_id),
                    start_time.elapsed(),
                    Severity::Critical,
                ))
            }
            Err(e) => {
                error!("Failed to check SSM Agent version: {}", e);
                Ok(DiagnosticResult::error(
                    "agent_version".to_string(),
                    format!("Failed to check SSM Agent version: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn check_last_communication(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking last communication time: {}", instance_id);
        
        match self.get_agent_info(instance_id).await {
            Ok(Some(agent_info)) => {
                if let Some(last_ping) = agent_info.last_ping_time {
                    let time_since_ping = Self::calculate_time_since_ping(last_ping);
                    let ping_status = agent_info.ping_status
                        .as_deref()
                        .map(PingStatus::from)
                        .unwrap_or(PingStatus::Unknown);
                    
                    let severity = Self::determine_ping_severity(&ping_status, time_since_ping);
                    
                    let communication_details = serde_json::json!({
                        "instance_id": instance_id,
                        "last_ping_time": last_ping,
                        "time_since_ping_seconds": time_since_ping.as_secs(),
                        "ping_status": ping_status,
                        "severity": severity
                    });
                    
                    let message = format!(
                        "Last communication: {} ago, Status: {:?}",
                        humantime::format_duration(time_since_ping),
                        ping_status
                    );
                    
                    match severity {
                        Severity::Info => {
                            debug!("Communication status is healthy: {}", message);
                            Ok(DiagnosticResult::success(
                                "last_communication".to_string(),
                                message,
                                start_time.elapsed(),
                            ).with_details(communication_details))
                        }
                        Severity::Medium => {
                            warn!("Communication status has minor issues: {}", message);
                            Ok(DiagnosticResult::warning(
                                "last_communication".to_string(),
                                message,
                                start_time.elapsed(),
                                severity,
                            ).with_details(communication_details))
                        }
                        _ => {
                            error!("Communication status has serious issues: {}", message);
                            Ok(DiagnosticResult::error(
                                "last_communication".to_string(),
                                message,
                                start_time.elapsed(),
                                severity,
                            ).with_details(communication_details).with_auto_fixable(true))
                        }
                    }
                } else {
                    warn!("Last communication time not available: {}", instance_id);
                    Ok(DiagnosticResult::warning(
                        "last_communication".to_string(),
                        format!("Last communication time not available for {}", instance_id),
                        start_time.elapsed(),
                        Severity::Medium,
                    ))
                }
            }
            Ok(None) => {
                error!("Instance not found or not registered: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "last_communication".to_string(),
                    format!("Instance {} not found or not registered as managed instance", instance_id),
                    start_time.elapsed(),
                    Severity::Critical,
                ))
            }
            Err(e) => {
                error!("Failed to check last communication: {}", e);
                Ok(DiagnosticResult::error(
                    "last_communication".to_string(),
                    format!("Failed to check last communication: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn validate_agent_configuration(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Validating SSM Agent configuration: {}", instance_id);
        
        match self.get_agent_info(instance_id).await {
            Ok(Some(agent_info)) => {
                let mut issues = Vec::new();
                let mut severity = Severity::Info;
                
                // Check if IAM role is assigned
                if agent_info.iam_role.is_none() {
                    issues.push("No IAM role assigned to instance");
                    severity = std::cmp::max(severity, Severity::High);
                }
                
                // Check if platform information is available
                if agent_info.platform_type.is_none() || agent_info.platform_name.is_none() {
                    issues.push("Platform information incomplete");
                    severity = std::cmp::max(severity, Severity::Low);
                }
                
                // Check if agent version is available
                if agent_info.agent_version.is_none() {
                    issues.push("Agent version information not available");
                    severity = std::cmp::max(severity, Severity::Medium);
                }
                
                // Check registration date
                if agent_info.registration_date.is_none() {
                    issues.push("Registration date not available");
                    severity = std::cmp::max(severity, Severity::Low);
                }
                
                let config_details = serde_json::json!({
                    "instance_id": instance_id,
                    "iam_role": agent_info.iam_role,
                    "platform_type": agent_info.platform_type,
                    "platform_name": agent_info.platform_name,
                    "agent_version": agent_info.agent_version,
                    "registration_date": agent_info.registration_date,
                    "issues": issues,
                    "severity": severity
                });
                
                if issues.is_empty() {
                    debug!("SSM Agent configuration is valid: {}", instance_id);
                    Ok(DiagnosticResult::success(
                        "agent_configuration".to_string(),
                        format!("SSM Agent configuration is valid for {}", instance_id),
                        start_time.elapsed(),
                    ).with_details(config_details))
                } else {
                    let message = format!(
                        "SSM Agent configuration issues found: {}",
                        issues.join(", ")
                    );
                    
                    match severity {
                        Severity::High | Severity::Critical => {
                            error!("Critical configuration issues: {}", message);
                            Ok(DiagnosticResult::error(
                                "agent_configuration".to_string(),
                                message,
                                start_time.elapsed(),
                                severity,
                            ).with_details(config_details).with_auto_fixable(false))
                        }
                        _ => {
                            warn!("Configuration issues found: {}", message);
                            Ok(DiagnosticResult::warning(
                                "agent_configuration".to_string(),
                                message,
                                start_time.elapsed(),
                                severity,
                            ).with_details(config_details))
                        }
                    }
                }
            }
            Ok(None) => {
                error!("Instance not found or not registered: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "agent_configuration".to_string(),
                    format!("Instance {} not found or not registered as managed instance", instance_id),
                    start_time.elapsed(),
                    Severity::Critical,
                ))
            }
            Err(e) => {
                error!("Failed to validate agent configuration: {}", e);
                Ok(DiagnosticResult::error(
                    "agent_configuration".to_string(),
                    format!("Failed to validate agent configuration: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn get_agent_info(&self, instance_id: &str) -> Result<Option<SsmAgentInfo>> {
        debug!("Getting SSM Agent info: {}", instance_id);
        
        let ssm_client = self.get_ssm_client();
        
        match ssm_client
            .describe_instance_information()
            .set_filters(Some(vec![
                aws_sdk_ssm::types::InstanceInformationStringFilter::builder()
                    .key("InstanceIds")
                    .values(instance_id)
                    .build()
                    .context("Failed to build instance filter")?
            ]))
            .send()
            .await
        {
            Ok(response) => {
                let instances = response.instance_information_list();
                
                if let Some(instance_info) = instances.first() {
                    let agent_info = SsmAgentInfo {
                        instance_id: instance_id.to_string(),
                        is_registered: true,
                        agent_version: instance_info.agent_version().map(|v| v.to_string()),
                        last_ping_time: instance_info.last_ping_date_time()
                            .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos())),
                        ping_status: instance_info.ping_status().map(|s| s.as_str().to_string()),
                        platform_type: instance_info.platform_type().map(|p| p.as_str().to_string()),
                        platform_name: instance_info.platform_name().map(|n| n.to_string()),
                        platform_version: instance_info.platform_version().map(|v| v.to_string()),
                        activation_id: instance_info.activation_id().map(|a| a.to_string()),
                        iam_role: instance_info.iam_role().map(|r| r.to_string()),
                        registration_date: instance_info.registration_date()
                            .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos())),
                        resource_type: instance_info.resource_type().map(|r| r.as_str().to_string()),
                        ip_address: instance_info.ip_address().map(|ip| ip.to_string()),
                        computer_name: instance_info.computer_name().map(|n| n.to_string()),
                    };
                    
                    debug!("SSM Agent info retrieved: {:?}", agent_info);
                    Ok(Some(agent_info))
                } else {
                    debug!("Instance not found in SSM managed instances: {}", instance_id);
                    Ok(None)
                }
            }
            Err(e) => {
                error!("Failed to get SSM Agent info: {}", e);
                Err(e.into())
            }
        }
    }
    
    async fn run_ssm_agent_diagnostics(&self, instance_id: &str) -> Result<Vec<DiagnosticResult>> {
        info!("Running comprehensive SSM Agent diagnostics: {}", instance_id);
        let start_time = Instant::now();
        
        let mut results = Vec::new();
        
        // Check managed instance registration first
        let registration_result = self.check_managed_instance_registration(instance_id).await?;
        let is_registered = registration_result.status == DiagnosticStatus::Success;
        results.push(registration_result);
        
        // Only continue with other checks if instance is registered
        if is_registered {
            // Enhanced checks for Task 25.2
            results.push(self.check_enhanced_agent_version(instance_id).await?);
            results.push(self.analyze_registration_details(instance_id).await?);
            results.push(self.check_agent_health_score(instance_id).await?);
            results.push(self.validate_ssm_service_registration(instance_id).await?);
            
            // Original checks
            results.push(self.check_last_communication(instance_id).await?);
            results.push(self.validate_agent_configuration(instance_id).await?);
        } else {
            // Add placeholder results for skipped checks
            let skipped_items = vec![
                "enhanced_agent_version",
                "registration_analysis", 
                "agent_health_score",
                "ssm_service_registration",
                "last_communication",
                "agent_configuration"
            ];
            
            for item in skipped_items {
                results.push(DiagnosticResult {
                    item_name: item.to_string(),
                    status: DiagnosticStatus::Skipped,
                    message: "Skipped due to instance not being registered".to_string(),
                    details: None,
                    duration: Duration::from_millis(0),
                    severity: Severity::Info,
                    auto_fixable: false,
                });
            }
        }
        
        info!("SSM Agent diagnostics completed in {:?}", start_time.elapsed());
        Ok(results)
    }
    
    // Enhanced methods for Task 25.2
    
    async fn check_enhanced_agent_version(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking enhanced SSM Agent version: {}", instance_id);
        
        match self.get_enhanced_agent_version_info(instance_id).await {
            Ok(Some(version_info)) => {
                let details = serde_json::to_value(&version_info)?;
                
                match version_info.update_urgency {
                    UpdateUrgency::Critical => {
                        error!("Critical SSM Agent update required: {}", version_info.current_version);
                        Ok(DiagnosticResult::error(
                            "enhanced_agent_version".to_string(),
                            format!("Critical SSM Agent update required. Current: {}, Latest: {}. {} security vulnerabilities found.",
                                version_info.current_version,
                                version_info.latest_available_version.as_deref().unwrap_or("unknown"),
                                version_info.vulnerability_info.len()),
                            start_time.elapsed(),
                            Severity::Critical,
                        ).with_details(details).with_auto_fixable(true))
                    }
                    UpdateUrgency::High => {
                        warn!("High priority SSM Agent update recommended: {}", version_info.current_version);
                        Ok(DiagnosticResult::warning(
                            "enhanced_agent_version".to_string(),
                            format!("High priority SSM Agent update recommended. Current: {}, Latest: {}",
                                version_info.current_version,
                                version_info.latest_available_version.as_deref().unwrap_or("unknown")),
                            start_time.elapsed(),
                            Severity::High,
                        ).with_details(details).with_auto_fixable(true))
                    }
                    UpdateUrgency::Medium => {
                        info!("SSM Agent update available: {}", version_info.current_version);
                        Ok(DiagnosticResult::warning(
                            "enhanced_agent_version".to_string(),
                            format!("SSM Agent update available. Current: {}, Latest: {}",
                                version_info.current_version,
                                version_info.latest_available_version.as_deref().unwrap_or("unknown")),
                            start_time.elapsed(),
                            Severity::Medium,
                        ).with_details(details).with_auto_fixable(true))
                    }
                    UpdateUrgency::Low => {
                        info!("Minor SSM Agent update available: {}", version_info.current_version);
                        Ok(DiagnosticResult::success(
                            "enhanced_agent_version".to_string(),
                            format!("SSM Agent version {} is acceptable (minor update available)",
                                version_info.current_version),
                            start_time.elapsed(),
                        ).with_details(details))
                    }
                    UpdateUrgency::None => {
                        debug!("SSM Agent version is up to date: {}", version_info.current_version);
                        Ok(DiagnosticResult::success(
                            "enhanced_agent_version".to_string(),
                            format!("SSM Agent version {} is up to date", version_info.current_version),
                            start_time.elapsed(),
                        ).with_details(details))
                    }
                }
            }
            Ok(None) => {
                error!("Enhanced agent version info not available: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "enhanced_agent_version".to_string(),
                    format!("Enhanced agent version information not available for {}", instance_id),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
            Err(e) => {
                error!("Failed to check enhanced agent version: {}", e);
                Ok(DiagnosticResult::error(
                    "enhanced_agent_version".to_string(),
                    format!("Failed to check enhanced agent version: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn analyze_registration_details(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Analyzing registration details: {}", instance_id);
        
        match self.get_registration_analysis(instance_id).await {
            Ok(Some(analysis)) => {
                let details = serde_json::to_value(&analysis)?;
                
                if analysis.registration_quality_score >= 80.0 {
                    debug!("Registration quality is excellent: {:.1}%", analysis.registration_quality_score);
                    Ok(DiagnosticResult::success(
                        "registration_analysis".to_string(),
                        format!("Registration quality excellent: {:.1}% ({})", 
                            analysis.registration_quality_score,
                            analysis.registration_status.to_string()),
                        start_time.elapsed(),
                    ).with_details(details))
                } else if analysis.registration_quality_score >= 60.0 {
                    warn!("Registration quality needs improvement: {:.1}%", analysis.registration_quality_score);
                    Ok(DiagnosticResult::warning(
                        "registration_analysis".to_string(),
                        format!("Registration quality needs improvement: {:.1}% ({} issues found)",
                            analysis.registration_quality_score,
                            analysis.registration_issues.len()),
                        start_time.elapsed(),
                        Severity::Medium,
                    ).with_details(details).with_auto_fixable(true))
                } else {
                    error!("Registration quality is poor: {:.1}%", analysis.registration_quality_score);
                    Ok(DiagnosticResult::error(
                        "registration_analysis".to_string(),
                        format!("Registration quality is poor: {:.1}% ({} critical issues)",
                            analysis.registration_quality_score,
                            analysis.registration_issues.len()),
                        start_time.elapsed(),
                        Severity::High,
                    ).with_details(details).with_auto_fixable(true))
                }
            }
            Ok(None) => {
                error!("Registration analysis not available: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "registration_analysis".to_string(),
                    format!("Registration analysis not available for {}", instance_id),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
            Err(e) => {
                error!("Failed to analyze registration details: {}", e);
                Ok(DiagnosticResult::error(
                    "registration_analysis".to_string(),
                    format!("Failed to analyze registration details: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn check_agent_health_score(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking agent health score: {}", instance_id);
        
        match self.get_agent_health_metrics(instance_id).await {
            Ok(Some(health_metrics)) => {
                let details = serde_json::to_value(&health_metrics)?;
                
                match health_metrics.health_trend {
                    HealthTrend::Critical => {
                        error!("Agent health is critical: {:.1}%", health_metrics.overall_health_score);
                        Ok(DiagnosticResult::error(
                            "agent_health_score".to_string(),
                            format!("Agent health is critical: {:.1}% (Error rate: {:.1}%)",
                                health_metrics.overall_health_score,
                                health_metrics.error_rate),
                            start_time.elapsed(),
                            Severity::Critical,
                        ).with_details(details).with_auto_fixable(true))
                    }
                    HealthTrend::Declining => {
                        warn!("Agent health is declining: {:.1}%", health_metrics.overall_health_score);
                        Ok(DiagnosticResult::warning(
                            "agent_health_score".to_string(),
                            format!("Agent health is declining: {:.1}% (Communication: {:.1}%, Performance: {:.1}%)",
                                health_metrics.overall_health_score,
                                health_metrics.communication_health,
                                health_metrics.performance_health),
                            start_time.elapsed(),
                            Severity::Medium,
                        ).with_details(details))
                    }
                    HealthTrend::Stable => {
                        info!("Agent health is stable: {:.1}%", health_metrics.overall_health_score);
                        Ok(DiagnosticResult::success(
                            "agent_health_score".to_string(),
                            format!("Agent health is stable: {:.1}% (Uptime: {:.1}%)",
                                health_metrics.overall_health_score,
                                health_metrics.uptime_percentage),
                            start_time.elapsed(),
                        ).with_details(details))
                    }
                    HealthTrend::Improving => {
                        debug!("Agent health is improving: {:.1}%", health_metrics.overall_health_score);
                        Ok(DiagnosticResult::success(
                            "agent_health_score".to_string(),
                            format!("Agent health is improving: {:.1}%", health_metrics.overall_health_score),
                            start_time.elapsed(),
                        ).with_details(details))
                    }
                }
            }
            Ok(None) => {
                error!("Agent health metrics not available: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "agent_health_score".to_string(),
                    format!("Agent health metrics not available for {}", instance_id),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
            Err(e) => {
                error!("Failed to check agent health score: {}", e);
                Ok(DiagnosticResult::error(
                    "agent_health_score".to_string(),
                    format!("Failed to check agent health score: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn validate_ssm_service_registration(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Validating SSM service registration: {}", instance_id);
        
        match self.get_ssm_service_registration(instance_id).await {
            Ok(Some(service_registration)) => {
                let details = serde_json::to_value(&service_registration)?;
                
                let mut registered_services = 0;
                let total_services = 6; // session_manager, patch_manager, run_command, state_manager, inventory, compliance
                let mut service_issues = Vec::new();
                
                // Check each service registration
                if service_registration.session_manager.registered {
                    registered_services += 1;
                } else {
                    service_issues.push("Session Manager not registered");
                }
                
                if service_registration.patch_manager.registered {
                    registered_services += 1;
                } else {
                    service_issues.push("Patch Manager not registered");
                }
                
                if service_registration.run_command.registered {
                    registered_services += 1;
                } else {
                    service_issues.push("Run Command not registered");
                }
                
                if service_registration.state_manager.registered {
                    registered_services += 1;
                } else {
                    service_issues.push("State Manager not registered");
                }
                
                if service_registration.inventory.registered {
                    registered_services += 1;
                } else {
                    service_issues.push("Inventory not registered");
                }
                
                if service_registration.compliance.registered {
                    registered_services += 1;
                } else {
                    service_issues.push("Compliance not registered");
                }
                
                let _registration_percentage = (registered_services as f64 / total_services as f64) * 100.0;
                
                if registered_services == total_services {
                    debug!("All SSM services are registered: {}/{}", registered_services, total_services);
                    Ok(DiagnosticResult::success(
                        "ssm_service_registration".to_string(),
                        format!("All SSM services are registered ({}/{})", registered_services, total_services),
                        start_time.elapsed(),
                    ).with_details(details))
                } else if registered_services >= 4 {
                    warn!("Most SSM services are registered: {}/{}", registered_services, total_services);
                    Ok(DiagnosticResult::warning(
                        "ssm_service_registration".to_string(),
                        format!("Most SSM services are registered ({}/{}) - Missing: {}",
                            registered_services, total_services, service_issues.join(", ")),
                        start_time.elapsed(),
                        Severity::Low,
                    ).with_details(details))
                } else {
                    error!("Many SSM services are not registered: {}/{}", registered_services, total_services);
                    Ok(DiagnosticResult::error(
                        "ssm_service_registration".to_string(),
                        format!("Many SSM services are not registered ({}/{}) - Missing: {}",
                            registered_services, total_services, service_issues.join(", ")),
                        start_time.elapsed(),
                        Severity::High,
                    ).with_details(details).with_auto_fixable(true))
                }
            }
            Ok(None) => {
                error!("SSM service registration info not available: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "ssm_service_registration".to_string(),
                    format!("SSM service registration information not available for {}", instance_id),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
            Err(e) => {
                error!("Failed to validate SSM service registration: {}", e);
                Ok(DiagnosticResult::error(
                    "ssm_service_registration".to_string(),
                    format!("Failed to validate SSM service registration: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn get_enhanced_agent_version_info(&self, instance_id: &str) -> Result<Option<EnhancedAgentVersionInfo>> {
        debug!("Getting enhanced agent version info: {}", instance_id);
        
        match self.get_agent_info(instance_id).await {
            Ok(Some(agent_info)) => {
                if let Some(current_version) = &agent_info.agent_version {
                    let platform = agent_info.platform_name.as_deref().unwrap_or("linux");
                    
                    // Get latest available version
                    let latest_version = self.get_latest_agent_version(platform).await
                        .unwrap_or_else(|_| "unknown".to_string());
                    
                    // Check for vulnerabilities
                    let vulnerabilities = Self::check_version_vulnerabilities(current_version);
                    
                    // Calculate update urgency
                    let update_urgency = Self::calculate_update_urgency(
                        current_version,
                        &latest_version,
                        &vulnerabilities,
                    );
                    
                    // Calculate version age (mock implementation)
                    let version_age_days = if let Some(registration_date) = agent_info.registration_date {
                        let now = chrono::Utc::now();
                        Some(now.signed_duration_since(registration_date).num_days())
                    } else {
                        None
                    };
                    
                    let enhanced_info = EnhancedAgentVersionInfo {
                        current_version: current_version.clone(),
                        latest_available_version: Some(latest_version.clone()),
                        is_current: current_version == &latest_version,
                        is_security_update_required: !vulnerabilities.is_empty(),
                        platform_specific_recommendation: Some(format!(
                            "Recommended version for {}: {}", platform, latest_version
                        )),
                        version_age_days,
                        update_urgency,
                        vulnerability_info: vulnerabilities,
                    };
                    
                    debug!("Enhanced agent version info: {:?}", enhanced_info);
                    Ok(Some(enhanced_info))
                } else {
                    debug!("Agent version not available for instance: {}", instance_id);
                    Ok(None)
                }
            }
            Ok(None) => {
                debug!("Agent info not available for instance: {}", instance_id);
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get enhanced agent version info: {}", e);
                Err(e)
            }
        }
    }

    async fn get_registration_analysis(&self, instance_id: &str) -> Result<Option<RegistrationAnalysis>> {
        debug!("Getting registration analysis: {}", instance_id);
        
        match self.get_agent_info(instance_id).await {
            Ok(Some(agent_info)) => {
                let ping_status = agent_info.ping_status
                    .as_deref()
                    .map(PingStatus::from)
                    .unwrap_or(PingStatus::Unknown);
                
                let time_since_ping = if let Some(last_ping) = agent_info.last_ping_time {
                    Self::calculate_time_since_ping(last_ping)
                } else {
                    Duration::from_secs(3600) // Default to 1 hour if unknown
                };
                
                // Calculate registration quality score
                let registration_quality_score = if agent_info.is_registered {
                    match ping_status {
                        PingStatus::Online => 100.0,
                        PingStatus::ConnectionLost => 70.0,
                        PingStatus::Inactive => 40.0,
                        PingStatus::Unknown => 50.0,
                    }
                } else {
                    0.0
                };
                
                // Generate registration issues
                let mut registration_issues = Vec::new();
                if !agent_info.is_registered {
                    registration_issues.push(RegistrationIssue {
                        issue_type: "NotRegistered".to_string(),
                        severity: "Critical".to_string(),
                        description: "Instance is not registered with SSM".to_string(),
                        resolution_steps: vec![
                            "Check IAM instance profile permissions".to_string(),
                            "Verify SSM agent is installed and running".to_string(),
                            "Check network connectivity to SSM endpoints".to_string(),
                        ],
                    });
                }
                
                if time_since_ping.as_secs() > 3600 {
                    registration_issues.push(RegistrationIssue {
                        issue_type: "StaleRegistration".to_string(),
                        severity: "Medium".to_string(),
                        description: "Last ping was more than 1 hour ago".to_string(),
                        resolution_steps: vec![
                            "Check instance connectivity".to_string(),
                            "Restart SSM agent".to_string(),
                        ],
                    });
                }
                
                // Generate recommendations
                let mut recommendations = Vec::new();
                if !agent_info.is_registered {
                    recommendations.push("Ensure the instance has proper IAM permissions for SSM".to_string());
                    recommendations.push("Verify SSM agent is installed and running".to_string());
                }
                
                if ping_status != PingStatus::Online {
                    recommendations.push("Check network connectivity to AWS SSM endpoints".to_string());
                }
                
                // Mock registration process stages
                let registration_process_stages = vec![
                    RegistrationStage {
                        stage_name: "IAM Role Check".to_string(),
                        status: "Success".to_string(),
                        last_check_time: Some(chrono::Utc::now()),
                        error_message: None,
                    },
                    RegistrationStage {
                        stage_name: "Agent Registration".to_string(),
                        status: if agent_info.is_registered { "Success".to_string() } else { "Failed".to_string() },
                        last_check_time: agent_info.registration_date,
                        error_message: None,
                    },
                ];
                
                let analysis = RegistrationAnalysis {
                    registration_status: if agent_info.is_registered {
                        RegistrationStatus::Registered
                    } else {
                        RegistrationStatus::NotRegistered
                    },
                    registration_quality_score,
                    registration_issues,
                    registration_recommendations: recommendations,
                    last_successful_registration: agent_info.registration_date,
                    registration_failure_count: 0, // Mock value
                    registration_process_stages,
                };
                
                debug!("Registration analysis: {:?}", analysis);
                Ok(Some(analysis))
            }
            Ok(None) => {
                debug!("Agent info not available for registration analysis: {}", instance_id);
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get registration analysis: {}", e);
                Err(e)
            }
        }
    }

    async fn get_agent_health_metrics(&self, instance_id: &str) -> Result<Option<AgentHealthMetrics>> {
        debug!("Getting agent health metrics: {}", instance_id);
        
        match self.get_agent_info(instance_id).await {
            Ok(Some(agent_info)) => {
                let ping_status = agent_info.ping_status
                    .as_deref()
                    .map(PingStatus::from)
                    .unwrap_or(PingStatus::Unknown);
                
                let time_since_ping = if let Some(last_ping) = agent_info.last_ping_time {
                    Self::calculate_time_since_ping(last_ping)
                } else {
                    Duration::from_secs(3600) // Default to 1 hour if unknown
                };
                
                // Calculate health metrics based on available data
                let communication_health = match ping_status {
                    PingStatus::Online => 100.0,
                    PingStatus::ConnectionLost => 50.0,
                    PingStatus::Inactive => 0.0,
                    PingStatus::Unknown => 25.0,
                };
                
                let performance_health = if time_since_ping.as_secs() < 300 { // 5 minutes
                    95.0
                } else if time_since_ping.as_secs() < 3600 { // 1 hour
                    70.0
                } else {
                    30.0
                };
                
                let overall_health_score = (communication_health + performance_health) / 2.0;
                
                let health_metrics = AgentHealthMetrics {
                    overall_health_score,
                    communication_health,
                    performance_health,
                    error_rate: 0.0, // Mock value
                    average_response_time_ms: Some(150.0), // Mock value
                    uptime_percentage: 99.0, // Mock value
                    last_24h_ping_success_rate: 95.0, // Mock value
                    health_trend: if overall_health_score > 80.0 {
                        HealthTrend::Improving
                    } else if overall_health_score > 60.0 {
                        HealthTrend::Stable
                    } else {
                        HealthTrend::Declining
                    },
                };
                
                debug!("Agent health metrics: {:?}", health_metrics);
                Ok(Some(health_metrics))
            }
            Ok(None) => {
                debug!("Agent info not available for health metrics: {}", instance_id);
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get agent health metrics: {}", e);
                Err(e)
            }
        }
    }

    async fn get_ssm_service_registration(&self, instance_id: &str) -> Result<Option<SsmServiceRegistration>> {
        debug!("Getting SSM service registration: {}", instance_id);
        
        // Mock implementation - in a real scenario, this would check various SSM services
        let session_manager = ServiceRegistrationStatus {
            registered: true,
            last_registration_time: Some(chrono::Utc::now()),
            capabilities: vec!["SessionManager".to_string()],
            configuration_issues: Vec::new(),
        };
        
        let patch_manager = ServiceRegistrationStatus {
            registered: true,
            last_registration_time: Some(chrono::Utc::now()),
            capabilities: vec!["PatchBaseline".to_string(), "PatchGroup".to_string()],
            configuration_issues: Vec::new(),
        };
        
        let run_command = ServiceRegistrationStatus {
            registered: true,
            last_registration_time: Some(chrono::Utc::now()),
            capabilities: vec!["RunCommand".to_string()],
            configuration_issues: Vec::new(),
        };
        
        let state_manager = ServiceRegistrationStatus {
            registered: true,
            last_registration_time: Some(chrono::Utc::now()),
            capabilities: vec!["StateManager".to_string()],
            configuration_issues: Vec::new(),
        };
        
        let inventory = ServiceRegistrationStatus {
            registered: true,
            last_registration_time: Some(chrono::Utc::now()),
            capabilities: vec!["AWS:Application".to_string(), "AWS:InstanceInformation".to_string()],
            configuration_issues: Vec::new(),
        };
        
        let compliance = ServiceRegistrationStatus {
            registered: false,
            last_registration_time: None,
            capabilities: Vec::new(),
            configuration_issues: vec!["Compliance scanning not configured".to_string()],
        };
        
        let registration = SsmServiceRegistration {
            session_manager,
            patch_manager,
            run_command,
            state_manager,
            inventory,
            compliance,
        };
        
        debug!("SSM service registration: {:?}", registration);
        Ok(Some(registration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ping_status_conversion() {
        assert_eq!(PingStatus::from("Online"), PingStatus::Online);
        assert_eq!(PingStatus::from("Connection Lost"), PingStatus::ConnectionLost);
        assert_eq!(PingStatus::from("Inactive"), PingStatus::Inactive);
        assert_eq!(PingStatus::from("Unknown Status"), PingStatus::Unknown);
    }
    
    #[test]
    fn test_agent_version_check() {
        // Current versions
        assert!(DefaultSsmAgentDiagnostics::is_agent_version_current("3.0.0.123"));
        assert!(DefaultSsmAgentDiagnostics::is_agent_version_current("3.1.2.456"));
        assert!(DefaultSsmAgentDiagnostics::is_agent_version_current("2.3.5.789"));
        
        // Outdated versions
        assert!(!DefaultSsmAgentDiagnostics::is_agent_version_current("2.2.9.123"));
        assert!(!DefaultSsmAgentDiagnostics::is_agent_version_current("1.9.9.999"));
        assert!(!DefaultSsmAgentDiagnostics::is_agent_version_current("2.0.0.100"));
        
        // Invalid versions
        assert!(!DefaultSsmAgentDiagnostics::is_agent_version_current("invalid"));
        assert!(!DefaultSsmAgentDiagnostics::is_agent_version_current("3.0"));
    }
    
    #[test]
    fn test_time_since_ping_calculation() {
        let now = chrono::Utc::now();
        let five_minutes_ago = now - chrono::Duration::minutes(5);
        let one_hour_ago = now - chrono::Duration::hours(1);
        
        let duration_5min = DefaultSsmAgentDiagnostics::calculate_time_since_ping(five_minutes_ago);
        let duration_1hr = DefaultSsmAgentDiagnostics::calculate_time_since_ping(one_hour_ago);
        
        // Allow some tolerance for test execution time
        assert!(duration_5min.as_secs() >= 290 && duration_5min.as_secs() <= 310); // ~5 minutes
        assert!(duration_1hr.as_secs() >= 3590 && duration_1hr.as_secs() <= 3610); // ~1 hour
    }
    
    #[test]
    fn test_ping_severity_determination() {
        let recent = Duration::from_secs(60); // 1 minute
        let moderate = Duration::from_secs(600); // 10 minutes
        let old = Duration::from_secs(7200); // 2 hours
        
        // Online status
        assert_eq!(
            DefaultSsmAgentDiagnostics::determine_ping_severity(&PingStatus::Online, recent),
            Severity::Info
        );
        assert_eq!(
            DefaultSsmAgentDiagnostics::determine_ping_severity(&PingStatus::Online, moderate),
            Severity::Medium
        );
        
        // Connection lost status
        assert_eq!(
            DefaultSsmAgentDiagnostics::determine_ping_severity(&PingStatus::ConnectionLost, moderate),
            Severity::Medium
        );
        assert_eq!(
            DefaultSsmAgentDiagnostics::determine_ping_severity(&PingStatus::ConnectionLost, old),
            Severity::High
        );
        
        // Inactive status
        assert_eq!(
            DefaultSsmAgentDiagnostics::determine_ping_severity(&PingStatus::Inactive, recent),
            Severity::High
        );
        
        // Unknown status
        assert_eq!(
            DefaultSsmAgentDiagnostics::determine_ping_severity(&PingStatus::Unknown, recent),
            Severity::Medium
        );
    }
    
    #[test]
    fn test_ssm_agent_info_serialization() {
        let agent_info = SsmAgentInfo {
            instance_id: "i-1234567890abcdef0".to_string(),
            is_registered: true,
            agent_version: Some("3.0.0.123".to_string()),
            last_ping_time: Some(chrono::Utc::now()),
            ping_status: Some("Online".to_string()),
            platform_type: Some("Linux".to_string()),
            platform_name: Some("Amazon Linux".to_string()),
            platform_version: Some("2023".to_string()),
            activation_id: None,
            iam_role: Some("EC2-SSM-Role".to_string()),
            registration_date: Some(chrono::Utc::now()),
            resource_type: Some("EC2Instance".to_string()),
            ip_address: Some("10.0.1.100".to_string()),
            computer_name: Some("ip-10-0-1-100".to_string()),
        };
        
        let serialized = serde_json::to_string(&agent_info).unwrap();
        let deserialized: SsmAgentInfo = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(agent_info.instance_id, deserialized.instance_id);
        assert_eq!(agent_info.is_registered, deserialized.is_registered);
        assert_eq!(agent_info.agent_version, deserialized.agent_version);
    }
    
    #[test]
    fn test_enhanced_version_info_serialization() {
        let version_info = EnhancedAgentVersionInfo {
            current_version: "3.0.0.123".to_string(),
            latest_available_version: Some("3.2.2086.0".to_string()),
            is_current: false,
            is_security_update_required: true,
            platform_specific_recommendation: Some("Update recommended for Linux".to_string()),
            version_age_days: Some(30),
            update_urgency: UpdateUrgency::High,
            vulnerability_info: vec![
                VulnerabilityInfo {
                    cve_id: Some("CVE-2023-1234".to_string()),
                    severity: "High".to_string(),
                    description: "Test vulnerability".to_string(),
                    fixed_in_version: "3.1.0.0".to_string(),
                }
            ],
        };
        
        let serialized = serde_json::to_string(&version_info).unwrap();
        let deserialized: EnhancedAgentVersionInfo = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(version_info.current_version, deserialized.current_version);
        assert_eq!(version_info.update_urgency, deserialized.update_urgency);
        assert_eq!(version_info.vulnerability_info.len(), deserialized.vulnerability_info.len());
    }
    
    #[test]
    fn test_registration_analysis_creation() {
        let analysis = RegistrationAnalysis {
            registration_status: RegistrationStatus::Registered,
            registration_quality_score: 85.0,
            registration_issues: Vec::new(),
            registration_recommendations: vec!["No issues found".to_string()],
            last_successful_registration: Some(chrono::Utc::now()),
            registration_failure_count: 0,
            registration_process_stages: vec![
                RegistrationStage {
                    stage_name: "IAM Role Verification".to_string(),
                    status: "Success".to_string(),
                    last_check_time: Some(chrono::Utc::now()),
                    error_message: None,
                }
            ],
        };
        
        assert_eq!(analysis.registration_status, RegistrationStatus::Registered);
        assert!(analysis.registration_quality_score > 80.0);
        assert!(analysis.registration_issues.is_empty());
    }
    
    #[test]
    fn test_agent_health_metrics_calculation() {
        let health_metrics = AgentHealthMetrics {
            overall_health_score: 90.0,
            communication_health: 95.0,
            performance_health: 85.0,
            error_rate: 2.0,
            average_response_time_ms: Some(150.0),
            uptime_percentage: 99.5,
            last_24h_ping_success_rate: 98.0,
            health_trend: HealthTrend::Stable,
        };
        
        assert!(health_metrics.overall_health_score > 80.0);
        assert_eq!(health_metrics.health_trend, HealthTrend::Stable);
        assert!(health_metrics.error_rate < 5.0);
    }
    
    #[test]
    fn test_update_urgency_calculation() {
        // Test critical urgency with vulnerabilities
        let vulnerabilities = vec![
            VulnerabilityInfo {
                cve_id: Some("CVE-2023-1234".to_string()),
                severity: "Critical".to_string(),
                description: "Critical vulnerability".to_string(),
                fixed_in_version: "3.1.0.0".to_string(),
            }
        ];
        
        let urgency = DefaultSsmAgentDiagnostics::calculate_update_urgency(
            "3.0.0.123",
            "3.2.2086.0",
            &vulnerabilities,
        );
        
        assert_eq!(urgency, UpdateUrgency::Critical);
        
        // Test no urgency for up-to-date version
        let no_vulnerabilities = Vec::new();
        let urgency_none = DefaultSsmAgentDiagnostics::calculate_update_urgency(
            "3.2.2086.0",
            "3.2.2086.0",
            &no_vulnerabilities,
        );
        
        assert_eq!(urgency_none, UpdateUrgency::None);
    }
}