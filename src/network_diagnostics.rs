use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{debug, error, info};

use crate::aws::AwsManager;
use crate::diagnostic::{DiagnosticResult, Severity};

/// Network diagnostics trait for SSM connection network requirements
#[async_trait]
pub trait NetworkDiagnostics {
    /// Check VPC endpoint configuration for SSM services
    async fn check_vpc_endpoints(&self, instance_id: &str) -> Result<DiagnosticResult>;

    /// Verify security group rules for SSM connectivity
    async fn check_security_group_rules(&self, instance_id: &str) -> Result<DiagnosticResult>;
    /// Test network connectivity to SSM endpoints
    async fn test_network_connectivity(&self, instance_id: &str) -> Result<DiagnosticResult>;
    // Enhanced methods for Task 25.3 - Comprehensive prerequisite checking

    /// VPC エンドポイント設定の詳細確認
    async fn detailed_vpc_endpoint_analysis(
        &self,
        instance_id: &str,
    ) -> Result<Vec<DiagnosticResult>>;

    /// セキュリティグループルールの詳細分析
    async fn detailed_security_group_analysis(
        &self,
        instance_id: &str,
    ) -> Result<Vec<DiagnosticResult>>;
}

/// VPC endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpcEndpointInfo {
    pub endpoint_id: String,
    pub service_name: String,
    pub state: String,
    pub vpc_id: String,
    pub route_table_ids: Vec<String>,
    pub subnet_ids: Vec<String>,
    pub policy_document: Option<String>,
}

/// Security group rule information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityGroupRule {
    pub group_id: String,
    pub group_name: String,
    pub rule_type: String, // "ingress" or "egress"
    pub protocol: String,
    pub from_port: Option<i32>,
    pub to_port: Option<i32>,
    pub cidr_blocks: Vec<String>,
    pub description: Option<String>,
}
/// Network connectivity test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityTestResult {
    pub endpoint: String,
    pub port: u16,
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error_message: Option<String>,
}

/// Default implementation of network diagnostics
pub struct DefaultNetworkDiagnostics {
    aws_manager: AwsManager,
}

impl DefaultNetworkDiagnostics {
    /// Create a new network diagnostics instance with default AWS configuration
    pub async fn with_default_aws() -> Result<Self> {
        let aws_manager = AwsManager::default()
            .await
            .context("Failed to create AWS manager for network diagnostics")?;

        Ok(Self { aws_manager })
    }

    /// Create a new network diagnostics instance with specific AWS configuration
    pub async fn with_aws_config(region: Option<&str>, profile: Option<&str>) -> Result<Self> {
        let aws_manager = AwsManager::new(
            region.map(|s| s.to_string()),
            profile.map(|s| s.to_string()),
        )
        .await
        .context("Failed to create AWS manager for network diagnostics")?;

        Ok(Self { aws_manager })
    }

    /// Create a new network diagnostics instance with custom AWS manager
    pub fn new(aws_manager: AwsManager) -> Self {
        Self { aws_manager }
    }

    // Helper methods for enhanced network diagnostics
    /// Get instance VPC and subnet information
    async fn get_instance_network_info(&self, instance_id: &str) -> Result<(String, String)> {
        debug!("Getting network info for instance: {}", instance_id);

        let response = self
            .aws_manager
            .ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .context("Failed to describe instance")?;

        for reservation in response.reservations() {
            for instance in reservation.instances() {
                if instance.instance_id.as_deref() == Some(instance_id) {
                    let vpc_id = instance
                        .vpc_id
                        .as_ref()
                        .context("Instance is not in a VPC")?
                        .clone();

                    let subnet_id = instance
                        .subnet_id
                        .as_ref()
                        .context("Instance subnet not found")?
                        .clone();

                    debug!(
                        "Instance network info - VPC: {}, Subnet: {}",
                        vpc_id, subnet_id
                    );
                    return Ok((vpc_id, subnet_id));
                }
            }
        }

        Err(anyhow::anyhow!("Instance {} not found", instance_id))
    }

    /// Get security groups for an instance
    async fn get_instance_security_groups(&self, instance_id: &str) -> Result<Vec<String>> {
        debug!("Getting security groups for instance: {}", instance_id);

        let response = self
            .aws_manager
            .ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .context("Failed to describe instance")?;

        for reservation in response.reservations() {
            for instance in reservation.instances() {
                if instance.instance_id.as_deref() == Some(instance_id) {
                    let security_groups: Vec<String> = instance
                        .security_groups()
                        .iter()
                        .filter_map(|sg| sg.group_id.as_ref())
                        .cloned()
                        .collect();

                    debug!("Instance security groups: {:?}", security_groups);
                    return Ok(security_groups);
                }
            }
        }

        Err(anyhow::anyhow!("Instance {} not found", instance_id))
    }

    /// Check if required SSM VPC endpoints exist
    async fn check_ssm_vpc_endpoints(&self, vpc_id: &str) -> Result<Vec<VpcEndpointInfo>> {
        debug!("Checking SSM VPC endpoints for VPC: {}", vpc_id);

        let required_services = [
            format!("com.amazonaws.{}.ssm", self.aws_manager.region()),
            format!("com.amazonaws.{}.ssmmessages", self.aws_manager.region()),
            format!("com.amazonaws.{}.ec2messages", self.aws_manager.region()),
        ];

        let response = self
            .aws_manager
            .ec2_client
            .describe_vpc_endpoints()
            .set_filters(Some(vec![aws_sdk_ec2::types::Filter::builder()
                .name("vpc-id")
                .values(vpc_id)
                .build()]))
            .send()
            .await
            .context("Failed to describe VPC endpoints")?;

        let mut found_endpoints = Vec::new();

        for endpoint in response.vpc_endpoints() {
            if let (Some(service_name), Some(endpoint_id), Some(state)) = (
                &endpoint.service_name,
                &endpoint.vpc_endpoint_id,
                &endpoint.state,
            ) {
                if required_services
                    .iter()
                    .any(|service| service == service_name)
                {
                    let route_table_ids = endpoint.route_table_ids().to_vec();

                    let subnet_ids = endpoint.subnet_ids().to_vec();

                    found_endpoints.push(VpcEndpointInfo {
                        endpoint_id: endpoint_id.clone(),
                        service_name: service_name.clone(),
                        state: state.as_str().to_string(),
                        vpc_id: vpc_id.to_string(),
                        route_table_ids,
                        subnet_ids,
                        policy_document: endpoint.policy_document.clone(),
                    });
                }
            }
        }

        debug!("Found {} SSM VPC endpoints", found_endpoints.len());
        Ok(found_endpoints)
    }

    /// Check security group rules for SSM connectivity
    async fn check_ssm_security_group_rules(
        &self,
        security_group_ids: &[String],
    ) -> Result<Vec<SecurityGroupRule>> {
        debug!(
            "Checking security group rules for: {:?}",
            security_group_ids
        );

        let response = self
            .aws_manager
            .ec2_client
            .describe_security_groups()
            .set_group_ids(Some(security_group_ids.to_vec()))
            .send()
            .await
            .context("Failed to describe security groups")?;

        let mut rules = Vec::new();

        for sg in response.security_groups() {
            if let (Some(group_id), Some(group_name)) = (&sg.group_id, &sg.group_name) {
                // Check egress rules (outbound)
                for rule in sg.ip_permissions_egress() {
                    if let Some(protocol) = &rule.ip_protocol {
                        let cidr_blocks: Vec<String> = rule
                            .ip_ranges()
                            .iter()
                            .filter_map(|range| range.cidr_ip.as_ref())
                            .cloned()
                            .collect();

                        rules.push(SecurityGroupRule {
                            group_id: group_id.clone(),
                            group_name: group_name.clone(),
                            rule_type: "egress".to_string(),
                            protocol: protocol.clone(),
                            from_port: rule.from_port,
                            to_port: rule.to_port,
                            cidr_blocks,
                            description: rule
                                .ip_ranges()
                                .first()
                                .and_then(|range| range.description.as_ref())
                                .cloned(),
                        });
                    }
                }

                // Check ingress rules (inbound) - less critical for SSM but good to know
                for rule in sg.ip_permissions() {
                    if let Some(protocol) = &rule.ip_protocol {
                        let cidr_blocks: Vec<String> = rule
                            .ip_ranges()
                            .iter()
                            .filter_map(|range| range.cidr_ip.as_ref())
                            .cloned()
                            .collect();

                        rules.push(SecurityGroupRule {
                            group_id: group_id.clone(),
                            group_name: group_name.clone(),
                            rule_type: "ingress".to_string(),
                            protocol: protocol.clone(),
                            from_port: rule.from_port,
                            to_port: rule.to_port,
                            cidr_blocks,
                            description: rule
                                .ip_ranges()
                                .first()
                                .and_then(|range| range.description.as_ref())
                                .cloned(),
                        });
                    }
                }
            }
        }

        debug!("Found {} security group rules", rules.len());
        Ok(rules)
    }
    /// Test connectivity to SSM endpoints
    async fn test_ssm_connectivity(&self) -> Result<Vec<ConnectivityTestResult>> {
        debug!("Testing SSM endpoint connectivity");

        let endpoints = vec![
            (
                format!("ssm.{}.amazonaws.com", self.aws_manager.region()),
                443,
            ),
            (
                format!("ssmmessages.{}.amazonaws.com", self.aws_manager.region()),
                443,
            ),
            (
                format!("ec2messages.{}.amazonaws.com", self.aws_manager.region()),
                443,
            ),
        ];

        let mut results = Vec::new();

        for (endpoint, port) in endpoints {
            let start_time = Instant::now();

            match tokio::time::timeout(
                Duration::from_secs(5),
                tokio::net::TcpStream::connect(format!("{}:{}", endpoint, port)),
            )
            .await
            {
                Ok(Ok(_)) => {
                    let latency = start_time.elapsed().as_millis() as u64;
                    results.push(ConnectivityTestResult {
                        endpoint: endpoint.clone(),
                        port,
                        success: true,
                        latency_ms: Some(latency),
                        error_message: None,
                    });
                    debug!("Connectivity test successful: {} ({}ms)", endpoint, latency);
                }
                Ok(Err(e)) => {
                    results.push(ConnectivityTestResult {
                        endpoint: endpoint.clone(),
                        port,
                        success: false,
                        latency_ms: None,
                        error_message: Some(e.to_string()),
                    });
                    debug!("Connectivity test failed: {} - {}", endpoint, e);
                }
                Err(_) => {
                    results.push(ConnectivityTestResult {
                        endpoint: endpoint.clone(),
                        port,
                        success: false,
                        latency_ms: None,
                        error_message: Some("Connection timeout".to_string()),
                    });
                    debug!("Connectivity test timeout: {}", endpoint);
                }
            }
        }

        Ok(results)
    }
}

#[async_trait]
impl NetworkDiagnostics for DefaultNetworkDiagnostics {
    async fn check_vpc_endpoints(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking VPC endpoints for instance: {}", instance_id);

        match self.get_instance_network_info(instance_id).await {
            Ok((vpc_id, _)) => match self.check_ssm_vpc_endpoints(&vpc_id).await {
                Ok(endpoints) => {
                    let required_services = vec![
                        format!("com.amazonaws.{}.ssm", self.aws_manager.region()),
                        format!("com.amazonaws.{}.ssmmessages", self.aws_manager.region()),
                        format!("com.amazonaws.{}.ec2messages", self.aws_manager.region()),
                    ];

                    let mut missing_services = Vec::new();
                    let mut available_services = Vec::new();

                    for service in &required_services {
                        if endpoints
                            .iter()
                            .any(|ep| &ep.service_name == service && ep.state == "Available")
                        {
                            available_services.push(service.clone());
                        } else {
                            missing_services.push(service.clone());
                        }
                    }

                    let details = serde_json::json!({
                        "vpc_id": vpc_id,
                        "required_services": required_services,
                        "available_endpoints": endpoints,
                        "missing_services": missing_services,
                        "available_services": available_services
                    });

                    if missing_services.is_empty() {
                        Ok(DiagnosticResult::success(
                            "vpc_endpoints".to_string(),
                            format!(
                                "All required SSM VPC endpoints are available in VPC {}",
                                vpc_id
                            ),
                            start_time.elapsed(),
                        )
                        .with_details(details))
                    } else if available_services.is_empty() {
                        Ok(DiagnosticResult::error(
                                "vpc_endpoints".to_string(),
                                format!("No SSM VPC endpoints found in VPC {}. SSM will use internet gateway if available.", vpc_id),
                                start_time.elapsed(),
                                Severity::Medium,
                            ).with_details(details))
                    } else {
                        Ok(DiagnosticResult::warning(
                            "vpc_endpoints".to_string(),
                            format!(
                                "Some SSM VPC endpoints missing in VPC {}: {:?}",
                                vpc_id, missing_services
                            ),
                            start_time.elapsed(),
                            Severity::Medium,
                        )
                        .with_details(details))
                    }
                }
                Err(e) => {
                    error!("Failed to check VPC endpoints: {}", e);
                    Ok(DiagnosticResult::error(
                        "vpc_endpoints".to_string(),
                        format!("Failed to check VPC endpoints: {}", e),
                        start_time.elapsed(),
                        Severity::High,
                    ))
                }
            },
            Err(e) => {
                error!("Failed to get instance network info: {}", e);
                Ok(DiagnosticResult::error(
                    "vpc_endpoints".to_string(),
                    format!("Failed to get instance network info: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }

    async fn check_security_group_rules(&self, instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!(
            "Checking security group rules for instance: {}",
            instance_id
        );

        match self.get_instance_security_groups(instance_id).await {
            Ok(security_group_ids) => {
                match self
                    .check_ssm_security_group_rules(&security_group_ids)
                    .await
                {
                    Ok(rules) => {
                        // Check for required HTTPS outbound rule (port 443)
                        let has_https_outbound = rules.iter().any(|rule| {
                            rule.rule_type == "egress"
                                && (rule.protocol == "tcp" || rule.protocol == "-1")
                                && (rule.to_port.is_some_and(|port| port >= 443)
                                    || rule.protocol == "-1")
                                && (rule.from_port.is_some_and(|port| port <= 443)
                                    || rule.protocol == "-1")
                                && (rule.cidr_blocks.contains(&"0.0.0.0/0".to_string())
                                    || !rule.cidr_blocks.is_empty())
                        });

                        let details = serde_json::json!({
                            "security_group_ids": security_group_ids,
                            "rules": rules,
                            "has_https_outbound": has_https_outbound,
                            "analysis": {
                                "total_rules": rules.len(),
                                "egress_rules": rules.iter().filter(|r| r.rule_type == "egress").count(),
                                "ingress_rules": rules.iter().filter(|r| r.rule_type == "ingress").count(),
                            }
                        });

                        if has_https_outbound {
                            Ok(DiagnosticResult::success(
                                "security_groups".to_string(),
                                "Security groups allow HTTPS outbound traffic (required for SSM)"
                                    .to_string(),
                                start_time.elapsed(),
                            )
                            .with_details(details))
                        } else {
                            Ok(DiagnosticResult::error(
                                "security_groups".to_string(),
                                "Security groups do not allow HTTPS outbound traffic on port 443 (required for SSM)".to_string(),
                                start_time.elapsed(),
                                Severity::Critical,
                            ).with_details(details).with_auto_fixable(true))
                        }
                    }
                    Err(e) => {
                        error!("Failed to check security group rules: {}", e);
                        Ok(DiagnosticResult::error(
                            "security_groups".to_string(),
                            format!("Failed to check security group rules: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get instance security groups: {}", e);
                Ok(DiagnosticResult::error(
                    "security_groups".to_string(),
                    format!("Failed to get instance security groups: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    async fn test_network_connectivity(&self, _instance_id: &str) -> Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Testing network connectivity to SSM endpoints");

        match self.test_ssm_connectivity().await {
            Ok(test_results) => {
                let successful_tests = test_results.iter().filter(|r| r.success).count();
                let total_tests = test_results.len();

                let details = serde_json::json!({
                    "connectivity_tests": test_results,
                    "summary": {
                        "total_tests": total_tests,
                        "successful_tests": successful_tests,
                        "failed_tests": total_tests - successful_tests,
                    }
                });

                if successful_tests == total_tests {
                    Ok(DiagnosticResult::success(
                        "network_connectivity".to_string(),
                        format!(
                            "All SSM endpoints are reachable ({}/{})",
                            successful_tests, total_tests
                        ),
                        start_time.elapsed(),
                    )
                    .with_details(details))
                } else if successful_tests > 0 {
                    Ok(DiagnosticResult::warning(
                        "network_connectivity".to_string(),
                        format!(
                            "Some SSM endpoints are not reachable ({}/{})",
                            successful_tests, total_tests
                        ),
                        start_time.elapsed(),
                        Severity::Medium,
                    )
                    .with_details(details))
                } else {
                    Ok(DiagnosticResult::error(
                        "network_connectivity".to_string(),
                        format!("No SSM endpoints are reachable (0/{})", total_tests),
                        start_time.elapsed(),
                        Severity::Critical,
                    )
                    .with_details(details))
                }
            }
            Err(e) => {
                error!("Failed to test network connectivity: {}", e);
                Ok(DiagnosticResult::error(
                    "network_connectivity".to_string(),
                    format!("Failed to test network connectivity: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    // Enhanced methods for Task 25.3 - Comprehensive prerequisite checking

    async fn detailed_vpc_endpoint_analysis(
        &self,
        instance_id: &str,
    ) -> Result<Vec<DiagnosticResult>> {
        info!(
            "Running detailed VPC endpoint analysis for: {}",
            instance_id
        );
        let mut results = Vec::new();

        match self.get_instance_network_info(instance_id).await {
            Ok((vpc_id, _)) => {
                match self.check_ssm_vpc_endpoints(&vpc_id).await {
                    Ok(endpoints) => {
                        // Analyze each endpoint individually
                        for endpoint in endpoints {
                            let start_time = Instant::now();

                            let mut endpoint_issues = Vec::new();
                            let mut endpoint_details = Vec::new();

                            // Check endpoint state
                            if endpoint.state != "Available" {
                                endpoint_issues.push(format!(
                                    "Endpoint state is '{}' (should be 'Available')",
                                    endpoint.state
                                ));
                            } else {
                                endpoint_details.push("Endpoint is available".to_string());
                            }

                            // Check route table associations
                            if endpoint.route_table_ids.is_empty() && endpoint.subnet_ids.is_empty()
                            {
                                endpoint_issues.push(
                                    "No route table or subnet associations found".to_string(),
                                );
                            } else {
                                endpoint_details.push(format!(
                                    "Associated with {} route tables and {} subnets",
                                    endpoint.route_table_ids.len(),
                                    endpoint.subnet_ids.len()
                                ));
                            }

                            // Check endpoint policy
                            if let Some(policy) = &endpoint.policy_document {
                                if policy.contains("Deny") {
                                    endpoint_issues.push(
                                        "Endpoint policy contains Deny statements".to_string(),
                                    );
                                } else {
                                    endpoint_details
                                        .push("Endpoint policy allows access".to_string());
                                }
                            } else {
                                endpoint_details
                                    .push("No custom endpoint policy (full access)".to_string());
                            }

                            let details = serde_json::json!({
                                "endpoint": endpoint,
                                "issues": endpoint_issues,
                                "details": endpoint_details
                            });

                            let result = if endpoint_issues.is_empty() {
                                DiagnosticResult::success(
                                    format!(
                                        "vpc_endpoint_{}",
                                        endpoint
                                            .service_name
                                            .split('.')
                                            .next_back()
                                            .unwrap_or("unknown")
                                    ),
                                    format!(
                                        "VPC endpoint {} is properly configured",
                                        endpoint.service_name
                                    ),
                                    start_time.elapsed(),
                                )
                                .with_details(details)
                            } else {
                                DiagnosticResult::warning(
                                    format!(
                                        "vpc_endpoint_{}",
                                        endpoint
                                            .service_name
                                            .split('.')
                                            .next_back()
                                            .unwrap_or("unknown")
                                    ),
                                    format!(
                                        "VPC endpoint {} has issues: {}",
                                        endpoint.service_name,
                                        endpoint_issues.join(", ")
                                    ),
                                    start_time.elapsed(),
                                    Severity::Medium,
                                )
                                .with_details(details)
                            };

                            results.push(result);
                        }
                    }
                    Err(e) => {
                        let start_time = Instant::now();
                        results.push(DiagnosticResult::error(
                            "detailed_vpc_endpoint_analysis".to_string(),
                            format!("Failed to analyze VPC endpoints: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        ));
                    }
                }
            }
            Err(e) => {
                let start_time = Instant::now();
                results.push(DiagnosticResult::error(
                    "detailed_vpc_endpoint_analysis".to_string(),
                    format!("Failed to get instance network info: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ));
            }
        }

        Ok(results)
    }

    async fn detailed_security_group_analysis(
        &self,
        instance_id: &str,
    ) -> Result<Vec<DiagnosticResult>> {
        info!(
            "Running detailed security group analysis for: {}",
            instance_id
        );
        let mut results = Vec::new();

        match self.get_instance_security_groups(instance_id).await {
            Ok(security_group_ids) => {
                match self
                    .check_ssm_security_group_rules(&security_group_ids)
                    .await
                {
                    Ok(rules) => {
                        // Group rules by security group
                        let mut sg_rules_map: HashMap<String, Vec<&SecurityGroupRule>> =
                            HashMap::new();

                        for rule in &rules {
                            sg_rules_map
                                .entry(rule.group_id.clone())
                                .or_default()
                                .push(rule);
                        }

                        // Analyze each security group individually
                        for (sg_id, sg_rules) in sg_rules_map {
                            let start_time = Instant::now();

                            let mut sg_issues = Vec::new();
                            let mut sg_analysis = Vec::new();

                            // Check for required HTTPS outbound (port 443)
                            let has_https_outbound = sg_rules.iter().any(|rule| {
                                rule.rule_type == "egress"
                                    && (rule.protocol == "tcp" || rule.protocol == "-1")
                                    && (rule.to_port.is_some_and(|port| port >= 443)
                                        || rule.protocol == "-1")
                                    && (rule.from_port.is_some_and(|port| port <= 443)
                                        || rule.protocol == "-1")
                                    && (rule.cidr_blocks.contains(&"0.0.0.0/0".to_string())
                                        || !rule.cidr_blocks.is_empty())
                            });

                            if !has_https_outbound {
                                sg_issues
                                    .push("Missing HTTPS outbound rule (port 443)".to_string());
                            } else {
                                sg_analysis.push("HTTPS outbound access available".to_string());
                            }

                            // Check for overly permissive rules
                            let has_all_outbound = sg_rules.iter().any(|rule| {
                                rule.rule_type == "egress"
                                    && rule.protocol == "-1"
                                    && rule.cidr_blocks.contains(&"0.0.0.0/0".to_string())
                            });

                            if has_all_outbound {
                                sg_analysis
                                    .push("All outbound traffic allowed (permissive)".to_string());
                            }

                            // Check for specific SSM ports
                            let ssm_ports = vec![443, 80]; // HTTPS and HTTP for SSM
                            for port in ssm_ports {
                                let has_port = sg_rules.iter().any(|rule| {
                                    rule.rule_type == "egress"
                                        && rule.protocol == "tcp"
                                        && rule.from_port.is_some_and(|from| from <= port)
                                        && rule.to_port.is_some_and(|to| to >= port)
                                });

                                if has_port {
                                    sg_analysis
                                        .push(format!("Port {} outbound access available", port));
                                } else {
                                    sg_issues.push(format!(
                                        "Port {} outbound access not explicitly allowed",
                                        port
                                    ));
                                }
                            }

                            // Analyze CIDR blocks
                            let mut cidr_analysis = Vec::new();
                            for rule in sg_rules.iter().filter(|r| r.rule_type == "egress") {
                                for cidr in &rule.cidr_blocks {
                                    if cidr == "0.0.0.0/0" {
                                        cidr_analysis
                                            .push("Internet access (0.0.0.0/0)".to_string());
                                    } else if cidr.starts_with("10.")
                                        || cidr.starts_with("172.")
                                        || cidr.starts_with("192.168.")
                                    {
                                        cidr_analysis
                                            .push(format!("Private network access ({})", cidr));
                                    } else {
                                        cidr_analysis
                                            .push(format!("Specific network access ({})", cidr));
                                    }
                                }
                            }

                            let details = serde_json::json!({
                                "security_group_id": sg_id,
                                "rules_count": sg_rules.len(),
                                "egress_rules": sg_rules.iter().filter(|r| r.rule_type == "egress").count(),
                                "ingress_rules": sg_rules.iter().filter(|r| r.rule_type == "ingress").count(),
                                "issues": sg_issues,
                                "analysis": sg_analysis,
                                "cidr_analysis": cidr_analysis,
                                "rules": sg_rules
                            });

                            let result = if sg_issues.is_empty() {
                                DiagnosticResult::success(
                                    format!("security_group_{}", sg_id),
                                    format!(
                                        "Security group {} is properly configured for SSM",
                                        sg_id
                                    ),
                                    start_time.elapsed(),
                                )
                                .with_details(details)
                            } else {
                                DiagnosticResult::warning(
                                    format!("security_group_{}", sg_id),
                                    format!(
                                        "Security group {} has issues: {}",
                                        sg_id,
                                        sg_issues.join(", ")
                                    ),
                                    start_time.elapsed(),
                                    Severity::Medium,
                                )
                                .with_details(details)
                            };

                            results.push(result);
                        }
                    }
                    Err(e) => {
                        let start_time = Instant::now();
                        results.push(DiagnosticResult::error(
                            "detailed_security_group_analysis".to_string(),
                            format!("Failed to analyze security group rules: {}", e),
                            start_time.elapsed(),
                            Severity::High,
                        ));
                    }
                }
            }
            Err(e) => {
                let start_time = Instant::now();
                results.push(DiagnosticResult::error(
                    "detailed_security_group_analysis".to_string(),
                    format!("Failed to get instance security groups: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ));
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::AwsManager;

    #[test]
    fn test_vpc_endpoint_info_serialization() {
        let endpoint_info = VpcEndpointInfo {
            endpoint_id: "vpce-12345".to_string(),
            service_name: "com.amazonaws.us-east-1.ssm".to_string(),
            state: "Available".to_string(),
            vpc_id: "vpc-12345".to_string(),
            route_table_ids: vec!["rtb-12345".to_string()],
            subnet_ids: vec!["subnet-12345".to_string()],
            policy_document: None,
        };

        let serialized = serde_json::to_string(&endpoint_info).unwrap();
        let deserialized: VpcEndpointInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(endpoint_info.endpoint_id, deserialized.endpoint_id);
        assert_eq!(endpoint_info.service_name, deserialized.service_name);
    }

    #[test]
    fn test_security_group_rule_creation() {
        let rule = SecurityGroupRule {
            group_id: "sg-12345".to_string(),
            group_name: "test-sg".to_string(),
            rule_type: "egress".to_string(),
            protocol: "tcp".to_string(),
            from_port: Some(443),
            to_port: Some(443),
            cidr_blocks: vec!["0.0.0.0/0".to_string()],
            description: Some("HTTPS outbound".to_string()),
        };

        assert_eq!(rule.group_id, "sg-12345");
        assert_eq!(rule.rule_type, "egress");
        assert_eq!(rule.from_port, Some(443));
    }

    #[test]
    fn test_connectivity_test_result() {
        let result = ConnectivityTestResult {
            endpoint: "ssm.us-east-1.amazonaws.com".to_string(),
            port: 443,
            success: true,
            latency_ms: Some(50),
            error_message: None,
        };

        assert!(result.success);
        assert_eq!(result.port, 443);
        assert_eq!(result.latency_ms, Some(50));
    }

    #[tokio::test]
    async fn test_network_diagnostics_creation() {
        // Test with properly configured AWS manager
        if let Ok(aws_manager) = AwsManager::default().await {
            let network_diagnostics = DefaultNetworkDiagnostics::new(aws_manager);

            // Verify the diagnostics instance was created
            // Region may vary based on AWS configuration
            assert!(!network_diagnostics.aws_manager.region().is_empty());
        }
        // Skip test if AWS credentials are not available
    }
}
