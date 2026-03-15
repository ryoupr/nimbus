use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

use crate::aws::AwsManager;
use crate::diagnostic::{DiagnosticResult, Severity};

/// EC2 instance information for diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub instance_type: Option<String>,
    pub state: Option<String>,
    pub state_code: Option<i32>,
    pub state_transition_reason: Option<String>,
    pub state_transition_time: Option<chrono::DateTime<chrono::Utc>>,
    pub platform: Option<String>,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
    pub availability_zone: Option<String>,
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
    pub launch_time: Option<chrono::DateTime<chrono::Utc>>,
    pub architecture: Option<String>,
    pub hypervisor: Option<String>,
    pub virtualization_type: Option<String>,
    pub cpu_options: Option<CpuOptions>,
    pub memory_info: Option<MemoryInfo>,
    pub storage_info: Option<StorageInfo>,
    pub network_info: Option<NetworkInfo>,
    pub monitoring: Option<MonitoringInfo>,
    pub tags: Vec<InstanceTag>,
}

/// CPU options information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuOptions {
    pub core_count: Option<i32>,
    pub threads_per_core: Option<i32>,
    pub total_vcpus: Option<i32>,
}

/// Memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub size_in_mib: Option<i64>,
    pub size_in_gb: Option<f64>,
}

/// Storage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub root_device_name: Option<String>,
    pub root_device_type: Option<String>,
    pub block_device_mappings: Vec<BlockDeviceMapping>,
    pub ebs_optimized: Option<bool>,
}

/// Block device mapping information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDeviceMapping {
    pub device_name: String,
    pub volume_id: Option<String>,
    pub volume_size: Option<i32>,
    pub volume_type: Option<String>,
    pub iops: Option<i32>,
    pub throughput: Option<i32>,
    pub encrypted: Option<bool>,
}

/// Network information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub network_interfaces: Vec<NetworkInterface>,
    pub source_dest_check: Option<bool>,
    pub ena_support: Option<bool>,
    pub sriov_net_support: Option<String>,
}

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub network_interface_id: String,
    pub subnet_id: String,
    pub vpc_id: String,
    pub private_ip_address: Option<String>,
    pub public_ip_address: Option<String>,
    pub security_groups: Vec<String>,
    pub status: String,
}

/// Monitoring information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringInfo {
    pub state: String,
    pub detailed_monitoring_enabled: bool,
}

/// Instance tag information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceTag {
    pub key: String,
    pub value: String,
}
/// Trait for instance diagnostics functionality
#[async_trait]
pub trait InstanceDiagnostics {
    /// Check if the EC2 instance exists
    async fn check_instance_exists(&self, instance_id: &str) -> Result<DiagnosticResult>;

    /// Check the current state of the EC2 instance
    async fn check_instance_state(&self, instance_id: &str) -> Result<DiagnosticResult>;
    /// Get enhanced instance information with detailed resource info
    async fn get_enhanced_instance_info(&self, instance_id: &str) -> Result<Option<InstanceInfo>>;
    /// Check detailed instance state with transition information
    async fn check_detailed_instance_state(&self, instance_id: &str) -> Result<DiagnosticResult>;
}

/// Default implementation of instance diagnostics
pub struct DefaultInstanceDiagnostics {
    aws_manager: AwsManager,
}

impl DefaultInstanceDiagnostics {
    /// Create a new instance diagnostics with AWS manager
    pub fn new(aws_manager: AwsManager) -> Self {
        Self { aws_manager }
    }

    /// Create instance diagnostics with default AWS configuration
    pub async fn with_default_aws() -> Result<Self> {
        let aws_manager = AwsManager::default()
            .await
            .context("Failed to create AWS manager")?;
        Ok(Self::new(aws_manager))
    }

    /// Create instance diagnostics with specific AWS configuration
    pub async fn with_aws_config(region: Option<&str>, profile: Option<&str>) -> Result<Self> {
        let aws_manager = AwsManager::new(
            region.map(|s| s.to_string()),
            profile.map(|s| s.to_string()),
        )
        .await
        .context("Failed to create AWS manager")?;
        Ok(Self::new(aws_manager))
    }
    /// Get memory information for instance type
    async fn get_instance_type_memory_info(&self, instance_type: &str) -> Option<MemoryInfo> {
        // This is a simplified mapping - in a real implementation, you might want to
        // use the EC2 DescribeInstanceTypes API or maintain a comprehensive mapping
        let memory_gb = match instance_type {
            // T3 instances
            t if t.starts_with("t3.nano") => 0.5,
            t if t.starts_with("t3.micro") => 1.0,
            t if t.starts_with("t3.small") => 2.0,
            t if t.starts_with("t3.medium") => 4.0,
            t if t.starts_with("t3.large") => 8.0,
            t if t.starts_with("t3.xlarge") => 16.0,
            t if t.starts_with("t3.2xlarge") => 32.0,

            // M5 instances
            t if t.starts_with("m5.large") => 8.0,
            t if t.starts_with("m5.xlarge") => 16.0,
            t if t.starts_with("m5.2xlarge") => 32.0,
            t if t.starts_with("m5.4xlarge") => 64.0,
            t if t.starts_with("m5.8xlarge") => 128.0,
            t if t.starts_with("m5.12xlarge") => 192.0,
            t if t.starts_with("m5.16xlarge") => 256.0,
            t if t.starts_with("m5.24xlarge") => 384.0,

            // C5 instances
            t if t.starts_with("c5.large") => 4.0,
            t if t.starts_with("c5.xlarge") => 8.0,
            t if t.starts_with("c5.2xlarge") => 16.0,
            t if t.starts_with("c5.4xlarge") => 32.0,
            t if t.starts_with("c5.9xlarge") => 72.0,
            t if t.starts_with("c5.12xlarge") => 96.0,
            t if t.starts_with("c5.18xlarge") => 144.0,
            t if t.starts_with("c5.24xlarge") => 192.0,

            // Add more instance types as needed
            _ => {
                debug!(
                    "Unknown instance type for memory calculation: {}",
                    instance_type
                );
                return None;
            }
        };

        Some(MemoryInfo {
            size_in_mib: Some((memory_gb * 1024.0) as i64),
            size_in_gb: Some(memory_gb),
        })
    }
}

#[async_trait]
impl InstanceDiagnostics for DefaultInstanceDiagnostics {
    async fn check_instance_exists(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking if instance exists: {}", instance_id);

        match self.aws_manager.get_instance_info(instance_id).await {
            Ok(Some(_)) => {
                debug!("Instance exists: {}", instance_id);
                Ok(DiagnosticResult::success(
                    "instance_exists".to_string(),
                    format!("Instance {} exists", instance_id),
                    start_time.elapsed(),
                ))
            }
            Ok(None) => {
                warn!("Instance not found: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "instance_exists".to_string(),
                    format!("Instance {} does not exist", instance_id),
                    start_time.elapsed(),
                    Severity::Critical,
                ))
            }
            Err(e) => {
                error!("Failed to check instance existence: {}", e);
                Ok(DiagnosticResult::error(
                    "instance_exists".to_string(),
                    format!("Failed to check instance existence: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }

    async fn check_instance_state(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let _start_time = Instant::now();
        info!("Checking instance state: {}", instance_id);

        // Use the enhanced detailed state check
        self.check_detailed_instance_state(instance_id).await
    }
    async fn get_enhanced_instance_info(&self, instance_id: &str) -> Result<Option<InstanceInfo>> {
        debug!(
            "Getting enhanced instance info with detailed resource information: {}",
            instance_id
        );

        // Get detailed instance information directly from EC2 API
        let response = self
            .aws_manager
            .ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .context("Failed to describe instance for enhanced info")?;

        for reservation in response.reservations() {
            for instance in reservation.instances() {
                if instance.instance_id.as_deref() == Some(instance_id) {
                    // Build enhanced instance info
                    let mut info = InstanceInfo {
                        instance_id: instance_id.to_string(),
                        instance_type: instance
                            .instance_type
                            .as_ref()
                            .map(|t| t.as_str().to_string()),
                        state: instance
                            .state
                            .as_ref()
                            .and_then(|s| s.name.as_ref())
                            .map(|n| n.as_str().to_string()),
                        state_code: instance.state.as_ref().and_then(|s| s.code),
                        state_transition_reason: instance.state_transition_reason.clone(),
                        state_transition_time: None, // Parse from state_transition_reason if needed
                        platform: instance.platform.as_ref().map(|p| p.as_str().to_string()),
                        private_ip: instance.private_ip_address.clone(),
                        public_ip: instance.public_ip_address.clone(),
                        availability_zone: instance
                            .placement
                            .as_ref()
                            .and_then(|p| p.availability_zone.clone()),
                        vpc_id: instance.vpc_id.clone(),
                        subnet_id: instance.subnet_id.clone(),
                        launch_time: instance.launch_time.map(|t| {
                            // Convert AWS SDK DateTime to chrono DateTime
                            let timestamp = t.as_secs_f64();
                            chrono::DateTime::from_timestamp(
                                timestamp as i64,
                                (timestamp.fract() * 1_000_000_000.0) as u32,
                            )
                            .unwrap_or_else(chrono::Utc::now)
                        }),
                        architecture: instance
                            .architecture
                            .as_ref()
                            .map(|a| a.as_str().to_string()),
                        hypervisor: instance.hypervisor.as_ref().map(|h| h.as_str().to_string()),
                        virtualization_type: instance
                            .virtualization_type
                            .as_ref()
                            .map(|v| v.as_str().to_string()),
                        cpu_options: None,
                        memory_info: None,
                        storage_info: None,
                        network_info: None,
                        monitoring: None,
                        tags: Vec::new(),
                    };

                    // Get CPU options
                    if let Some(cpu_options) = &instance.cpu_options {
                        info.cpu_options = Some(CpuOptions {
                            core_count: cpu_options.core_count,
                            threads_per_core: cpu_options.threads_per_core,
                            total_vcpus: cpu_options.core_count.and_then(|cores| {
                                cpu_options.threads_per_core.map(|threads| cores * threads)
                            }),
                        });
                    }

                    // Get instance type info for memory
                    if let Some(instance_type) = &info.instance_type {
                        info.memory_info = self.get_instance_type_memory_info(instance_type).await;
                    }

                    // Get storage information
                    info.storage_info = Some(StorageInfo {
                        root_device_name: instance.root_device_name.clone(),
                        root_device_type: instance
                            .root_device_type
                            .as_ref()
                            .map(|t| t.as_str().to_string()),
                        block_device_mappings: instance
                            .block_device_mappings()
                            .iter()
                            .map(|bdm| BlockDeviceMapping {
                                device_name: bdm.device_name.clone().unwrap_or_default(),
                                volume_id: bdm.ebs.as_ref().and_then(|ebs| ebs.volume_id.clone()),
                                volume_size: None, // EBS instance block device doesn't have size info
                                volume_type: None, // EBS instance block device doesn't have type info
                                iops: None, // EBS instance block device doesn't have IOPS info
                                throughput: None, // EBS instance block device doesn't have throughput info
                                encrypted: None, // EBS instance block device doesn't have encryption info
                            })
                            .collect(),
                        ebs_optimized: instance.ebs_optimized,
                    });

                    // Get network information
                    let network_interfaces: Vec<NetworkInterface> = instance
                        .network_interfaces()
                        .iter()
                        .map(|ni| NetworkInterface {
                            network_interface_id: ni
                                .network_interface_id
                                .clone()
                                .unwrap_or_default(),
                            subnet_id: ni.subnet_id.clone().unwrap_or_default(),
                            vpc_id: ni.vpc_id.clone().unwrap_or_default(),
                            private_ip_address: ni.private_ip_address.clone(),
                            public_ip_address: ni
                                .association
                                .as_ref()
                                .and_then(|a| a.public_ip.clone()),
                            security_groups: ni
                                .groups()
                                .iter()
                                .filter_map(|g| g.group_id.clone())
                                .collect(),
                            status: ni
                                .status
                                .as_ref()
                                .map(|s| s.as_str().to_string())
                                .unwrap_or_default(),
                        })
                        .collect();

                    info.network_info = Some(NetworkInfo {
                        network_interfaces,
                        source_dest_check: instance.source_dest_check,
                        ena_support: instance.ena_support,
                        sriov_net_support: instance
                            .sriov_net_support
                            .as_ref()
                            .map(|s| s.to_string()),
                    });

                    // Get monitoring information
                    if let Some(monitoring) = &instance.monitoring {
                        info.monitoring = Some(MonitoringInfo {
                            state: monitoring
                                .state
                                .as_ref()
                                .map(|s| s.as_str().to_string())
                                .unwrap_or_default(),
                            detailed_monitoring_enabled: monitoring
                                .state
                                .as_ref()
                                .map(|s| s.as_str() == "enabled")
                                .unwrap_or(false),
                        });
                    }

                    // Get tags
                    info.tags = instance
                        .tags()
                        .iter()
                        .map(|tag| InstanceTag {
                            key: tag.key.clone().unwrap_or_default(),
                            value: tag.value.clone().unwrap_or_default(),
                        })
                        .collect();

                    debug!("Enhanced instance info retrieved: {:?}", info);
                    return Ok(Some(info));
                }
            }
        }

        debug!("Instance not found: {}", instance_id);
        Ok(None)
    }

    async fn check_detailed_instance_state(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking detailed instance state: {}", instance_id);

        match self.get_enhanced_instance_info(instance_id).await {
            Ok(Some(info)) => {
                let state = info.state.as_deref().unwrap_or("unknown");
                let state_code = info.state_code.unwrap_or(-1);
                let transition_reason = info
                    .state_transition_reason
                    .as_deref()
                    .unwrap_or("No reason provided");

                let details = serde_json::json!({
                    "instance_id": instance_id,
                    "state": state,
                    "state_code": state_code,
                    "transition_reason": transition_reason,
                    "launch_time": info.launch_time,
                    "instance_type": info.instance_type,
                    "availability_zone": info.availability_zone,
                    "platform": info.platform
                });

                match state {
                    "running" => {
                        debug!("Instance is running: {}", instance_id);
                        Ok(DiagnosticResult::success(
                            "detailed_instance_state".to_string(),
                            format!(
                                "Instance {} is running (state code: {})",
                                instance_id, state_code
                            ),
                            start_time.elapsed(),
                        )
                        .with_details(details))
                    }
                    "stopped" => {
                        warn!("Instance is stopped: {}", instance_id);
                        Ok(DiagnosticResult::error(
                            "detailed_instance_state".to_string(),
                            format!(
                                "Instance {} is stopped (state code: {}). Reason: {}",
                                instance_id, state_code, transition_reason
                            ),
                            start_time.elapsed(),
                            Severity::High,
                        )
                        .with_details(details)
                        .with_auto_fixable(true))
                    }
                    "stopping" => {
                        warn!("Instance is stopping: {}", instance_id);
                        Ok(DiagnosticResult::warning(
                            "detailed_instance_state".to_string(),
                            format!(
                                "Instance {} is stopping (state code: {}). Reason: {}",
                                instance_id, state_code, transition_reason
                            ),
                            start_time.elapsed(),
                            Severity::Medium,
                        )
                        .with_details(details))
                    }
                    "pending" => {
                        info!("Instance is starting: {}", instance_id);
                        Ok(DiagnosticResult::warning(
                            "detailed_instance_state".to_string(),
                            format!(
                                "Instance {} is starting (state code: {}). Reason: {}",
                                instance_id, state_code, transition_reason
                            ),
                            start_time.elapsed(),
                            Severity::Low,
                        )
                        .with_details(details))
                    }
                    "shutting-down" | "terminated" => {
                        error!("Instance is terminated or terminating: {}", instance_id);
                        Ok(DiagnosticResult::error(
                            "detailed_instance_state".to_string(),
                            format!("Instance {} is {} (state code: {}) and cannot be connected to. Reason: {}", 
                                instance_id, state, state_code, transition_reason),
                            start_time.elapsed(),
                            Severity::Critical,
                        ).with_details(details))
                    }
                    _ => {
                        warn!("Instance has unknown state: {} ({})", instance_id, state);
                        Ok(DiagnosticResult::warning(
                            "detailed_instance_state".to_string(),
                            format!(
                                "Instance {} has unknown state: {} (state code: {}). Reason: {}",
                                instance_id, state, state_code, transition_reason
                            ),
                            start_time.elapsed(),
                            Severity::Medium,
                        )
                        .with_details(details))
                    }
                }
            }
            Ok(None) => {
                error!("Instance not found: {}", instance_id);
                Ok(DiagnosticResult::error(
                    "detailed_instance_state".to_string(),
                    format!("Instance {} not found", instance_id),
                    start_time.elapsed(),
                    Severity::Critical,
                ))
            }
            Err(e) => {
                error!("Failed to check detailed instance state: {}", e);
                Ok(DiagnosticResult::error(
                    "detailed_instance_state".to_string(),
                    format!("Failed to check detailed instance state: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
