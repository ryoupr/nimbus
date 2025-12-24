use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn, error, debug};
use async_trait::async_trait;

use crate::diagnostic::{DiagnosticConfig, DiagnosticResult, DiagnosticStatus, Severity};
use crate::instance_diagnostics::{InstanceDiagnostics, DefaultInstanceDiagnostics};
use crate::port_diagnostics::{PortDiagnostics, DefaultPortDiagnostics};
use crate::ssm_agent_diagnostics::{SsmAgentDiagnostics, DefaultSsmAgentDiagnostics};
use crate::iam_diagnostics::{IamDiagnostics, DefaultIamDiagnostics};
use crate::network_diagnostics::{NetworkDiagnostics, DefaultNetworkDiagnostics};

/// Configuration for preventive checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreventiveCheckConfig {
    pub instance_id: String,
    pub local_port: Option<u16>,
    pub remote_port: Option<u16>,
    pub region: Option<String>,
    pub profile: Option<String>,
    pub abort_on_critical: bool,
    pub timeout: Duration,
}

impl PreventiveCheckConfig {
    pub fn new(instance_id: String) -> Self {
        Self {
            instance_id,
            local_port: None,
            remote_port: None,
            region: None,
            profile: None,
            abort_on_critical: true,
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

    pub fn with_abort_on_critical(mut self, abort: bool) -> Self {
        self.abort_on_critical = abort;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl From<DiagnosticConfig> for PreventiveCheckConfig {
    fn from(config: DiagnosticConfig) -> Self {
        Self {
            instance_id: config.instance_id,
            local_port: config.local_port,
            remote_port: config.remote_port,
            region: config.region,
            profile: config.profile,
            abort_on_critical: true,
            timeout: config.timeout,
        }
    }
}

/// Result of preventive check execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreventiveCheckResult {
    pub overall_status: PreventiveCheckStatus,
    pub connection_likelihood: ConnectionLikelihood,
    pub critical_issues: Vec<DiagnosticResult>,
    pub warnings: Vec<DiagnosticResult>,
    pub recommendations: Vec<String>,
    pub should_abort_connection: bool,
    pub total_duration: Duration,
}

/// Overall status of preventive checks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PreventiveCheckStatus {
    Ready,
    Warning,
    Critical,
    Aborted,
}

/// Likelihood of successful connection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionLikelihood {
    High,
    Medium,
    Low,
    VeryLow,
}

impl ConnectionLikelihood {
    pub fn as_percentage(&self) -> u8 {
        match self {
            ConnectionLikelihood::High => 90,
            ConnectionLikelihood::Medium => 70,
            ConnectionLikelihood::Low => 40,
            ConnectionLikelihood::VeryLow => 10,
        }
    }

    pub fn as_description(&self) -> &'static str {
        match self {
            ConnectionLikelihood::High => "接続成功の可能性が高いです",
            ConnectionLikelihood::Medium => "接続成功の可能性は中程度です",
            ConnectionLikelihood::Low => "接続成功の可能性は低いです",
            ConnectionLikelihood::VeryLow => "接続成功の可能性は非常に低いです",
        }
    }
}

/// Trait for preventive check implementations
#[async_trait]
pub trait PreventiveCheck {
    /// Run comprehensive preventive checks before connection attempt
    async fn run_preventive_checks(&self, config: PreventiveCheckConfig) -> Result<PreventiveCheckResult, Box<dyn std::error::Error>>;
    
    /// Check basic instance state
    async fn check_basic_state(&self, config: &PreventiveCheckConfig) -> Result<DiagnosticResult, Box<dyn std::error::Error>>;
    
    /// Verify SSM prerequisites
    async fn verify_ssm_prerequisites(&self, config: &PreventiveCheckConfig) -> Result<Vec<DiagnosticResult>, Box<dyn std::error::Error>>;
    
    /// Display warnings for detected issues
    async fn display_warnings(&self, issues: &[DiagnosticResult]) -> Result<(), Box<dyn std::error::Error>>;
    
    /// Calculate connection success likelihood
    fn calculate_connection_likelihood(&self, results: &[DiagnosticResult]) -> ConnectionLikelihood;
    
    /// Determine if connection should be aborted
    fn should_abort_connection(&self, results: &[DiagnosticResult], config: &PreventiveCheckConfig) -> bool;
}

/// Default implementation of preventive checks
pub struct DefaultPreventiveCheck {
    instance_diagnostics: DefaultInstanceDiagnostics,
    port_diagnostics: DefaultPortDiagnostics,
    ssm_diagnostics: DefaultSsmAgentDiagnostics,
    iam_diagnostics: DefaultIamDiagnostics,
    network_diagnostics: DefaultNetworkDiagnostics,
}

impl DefaultPreventiveCheck {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            instance_diagnostics: DefaultInstanceDiagnostics::with_default_aws().await?,
            port_diagnostics: DefaultPortDiagnostics::new(),
            ssm_diagnostics: DefaultSsmAgentDiagnostics::with_default_aws().await?,
            iam_diagnostics: DefaultIamDiagnostics::with_default_aws().await?,
            network_diagnostics: DefaultNetworkDiagnostics::with_default_aws().await?,
        })
    }

    pub async fn with_aws_config(region: Option<String>, profile: Option<String>) -> anyhow::Result<Self> {
        let aws_manager = crate::aws::AwsManager::new(region, profile).await
            .map_err(|e| anyhow::anyhow!("Failed to create AWS manager: {}", e))?;
        
        Ok(Self {
            instance_diagnostics: DefaultInstanceDiagnostics::new(aws_manager.clone()),
            port_diagnostics: DefaultPortDiagnostics::new(),
            ssm_diagnostics: DefaultSsmAgentDiagnostics::new(aws_manager.clone()),
            iam_diagnostics: DefaultIamDiagnostics::with_aws_manager(&aws_manager).await?,
            network_diagnostics: DefaultNetworkDiagnostics::new(aws_manager),
        })
    }

    /// Generate recommendations based on diagnostic results
    fn generate_recommendations(&self, results: &[DiagnosticResult]) -> Vec<String> {
        let mut recommendations = Vec::new();

        for result in results {
            match (&result.status, &result.severity) {
                (DiagnosticStatus::Error, Severity::Critical) => {
                    match result.item_name.as_str() {
                        "instance_state" => {
                            recommendations.push("インスタンスを起動してから接続を再試行してください".to_string());
                        }
                        "ssm_agent" => {
                            recommendations.push("SSMエージェントの設定を確認し、必要に応じて再インストールしてください".to_string());
                        }
                        "iam_permissions" => {
                            recommendations.push("IAMロールとポリシーの設定を確認してください".to_string());
                        }
                        "vpc_endpoints" => {
                            recommendations.push("VPCエンドポイントの設定を確認するか、インターネットゲートウェイを設定してください".to_string());
                        }
                        "security_groups" => {
                            recommendations.push("セキュリティグループのアウトバウンドルールを確認してください".to_string());
                        }
                        "local_port" => {
                            recommendations.push("ローカルポートの使用状況を確認し、必要に応じて代替ポートを使用してください".to_string());
                        }
                        _ => {
                            recommendations.push(format!("{}の問題を解決してから接続を再試行してください", result.item_name));
                        }
                    }
                }
                (DiagnosticStatus::Warning, _) => {
                    recommendations.push(format!("{}の警告を確認することをお勧めします", result.item_name));
                }
                _ => {}
            }
        }

        if recommendations.is_empty() {
            recommendations.push("すべての前提条件が満たされています。接続を開始できます".to_string());
        }

        recommendations
    }
}

#[async_trait]
impl PreventiveCheck for DefaultPreventiveCheck {
    async fn run_preventive_checks(&self, config: PreventiveCheckConfig) -> Result<PreventiveCheckResult, Box<dyn std::error::Error>> {
        info!("Starting preventive checks for instance: {}", config.instance_id);
        let start_time = Instant::now();

        // Step 1: Check basic instance state
        info!("Step 1: Checking basic instance state");
        let basic_state_result = self.check_basic_state(&config).await?;
        
        // If instance is not in a good state, abort early
        if basic_state_result.status == DiagnosticStatus::Error && basic_state_result.severity == Severity::Critical {
            warn!("Critical instance state issue detected, aborting preventive checks");
            let recommendations = self.generate_recommendations(&[basic_state_result.clone()]);
            return Ok(PreventiveCheckResult {
                overall_status: PreventiveCheckStatus::Critical,
                connection_likelihood: ConnectionLikelihood::VeryLow,
                critical_issues: vec![basic_state_result],
                warnings: vec![],
                recommendations,
                should_abort_connection: config.abort_on_critical,
                total_duration: start_time.elapsed(),
            });
        }

        // Step 2: Verify SSM prerequisites
        info!("Step 2: Verifying SSM prerequisites");
        let mut prerequisite_results = self.verify_ssm_prerequisites(&config).await?;
        prerequisite_results.insert(0, basic_state_result);

        // Step 3: Analyze results and determine overall status
        info!("Step 3: Analyzing results");
        let critical_issues: Vec<DiagnosticResult> = prerequisite_results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Error && r.severity == Severity::Critical)
            .cloned()
            .collect();

        let warnings: Vec<DiagnosticResult> = prerequisite_results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Warning || 
                     (r.status == DiagnosticStatus::Error && r.severity != Severity::Critical))
            .cloned()
            .collect();

        // Step 4: Display warnings if any issues detected
        if !critical_issues.is_empty() || !warnings.is_empty() {
            info!("Step 4: Displaying warnings for detected issues");
            let all_issues: Vec<DiagnosticResult> = critical_issues.iter().chain(warnings.iter()).cloned().collect();
            self.display_warnings(&all_issues).await?;
        }

        // Step 5: Calculate connection likelihood
        let connection_likelihood = self.calculate_connection_likelihood(&prerequisite_results);
        info!("Connection likelihood: {:?} ({}%)", connection_likelihood, connection_likelihood.as_percentage());

        // Step 6: Determine if connection should be aborted
        let should_abort = self.should_abort_connection(&prerequisite_results, &config);
        
        let overall_status = if should_abort {
            PreventiveCheckStatus::Aborted
        } else if !critical_issues.is_empty() {
            PreventiveCheckStatus::Critical
        } else if !warnings.is_empty() {
            PreventiveCheckStatus::Warning
        } else {
            PreventiveCheckStatus::Ready
        };

        let recommendations = self.generate_recommendations(&prerequisite_results);

        let result = PreventiveCheckResult {
            overall_status,
            connection_likelihood,
            critical_issues,
            warnings,
            recommendations,
            should_abort_connection: should_abort,
            total_duration: start_time.elapsed(),
        };

        info!("Preventive checks completed in {:?}", result.total_duration);
        Ok(result)
    }

    async fn check_basic_state(&self, config: &PreventiveCheckConfig) -> Result<DiagnosticResult, Box<dyn std::error::Error>> {
        info!("Checking basic instance state for: {}", config.instance_id);
        
        // Check instance state
        let instance_result = self.instance_diagnostics.check_instance_state(&config.instance_id).await?;
        
        // Check local port if specified
        if let Some(port) = config.local_port {
            let port_result = self.port_diagnostics.diagnose_port(port).await;
            
            // If port check fails, it's not critical for basic state but should be noted
            if port_result.status == DiagnosticStatus::Error {
                return Ok(DiagnosticResult::warning(
                    "basic_state".to_string(),
                    format!("Instance is ready but local port {} has issues: {}", port, port_result.message),
                    instance_result.duration + port_result.duration,
                    Severity::Medium,
                ));
            }
        }

        // Return the instance state result as the basic state
        Ok(DiagnosticResult {
            item_name: "basic_state".to_string(),
            status: instance_result.status,
            message: format!("Basic state check: {}", instance_result.message),
            details: instance_result.details,
            duration: instance_result.duration,
            severity: instance_result.severity,
            auto_fixable: instance_result.auto_fixable,
        })
    }

    async fn verify_ssm_prerequisites(&self, config: &PreventiveCheckConfig) -> Result<Vec<DiagnosticResult>, Box<dyn std::error::Error>> {
        info!("Verifying SSM prerequisites for: {}", config.instance_id);
        let mut results = Vec::new();

        // Check SSM agent registration
        info!("Checking SSM agent registration");
        match self.ssm_diagnostics.check_managed_instance_registration(&config.instance_id).await {
            Ok(result) => results.push(result),
            Err(e) => {
                error!("SSM agent check failed: {}", e);
                results.push(DiagnosticResult::error(
                    "ssm_agent".to_string(),
                    format!("SSM agent check failed: {}", e),
                    Duration::from_millis(100),
                    Severity::Critical,
                ));
            }
        }

        // Check IAM permissions
        info!("Checking IAM permissions");
        match self.iam_diagnostics.diagnose_iam_configuration(&config.instance_id).await {
            Ok(iam_results) => {
                // Aggregate IAM results
                let critical_iam_issues: Vec<_> = iam_results
                    .iter()
                    .filter(|r| r.status == DiagnosticStatus::Error && r.severity == Severity::Critical)
                    .collect();

                if !critical_iam_issues.is_empty() {
                    results.push(DiagnosticResult::error(
                        "iam_prerequisites".to_string(),
                        format!("Critical IAM issues detected: {} issues", critical_iam_issues.len()),
                        Duration::from_millis(100),
                        Severity::Critical,
                    ));
                } else {
                    let warning_count = iam_results
                        .iter()
                        .filter(|r| r.status == DiagnosticStatus::Warning)
                        .count();
                    
                    if warning_count > 0 {
                        results.push(DiagnosticResult::warning(
                            "iam_prerequisites".to_string(),
                            format!("IAM configuration has {} warnings", warning_count),
                            Duration::from_millis(100),
                            Severity::Medium,
                        ));
                    } else {
                        results.push(DiagnosticResult::success(
                            "iam_prerequisites".to_string(),
                            "IAM configuration verified".to_string(),
                            Duration::from_millis(100),
                        ));
                    }
                }
            }
            Err(e) => {
                error!("IAM check failed: {}", e);
                results.push(DiagnosticResult::error(
                    "iam_prerequisites".to_string(),
                    format!("IAM check failed: {}", e),
                    Duration::from_millis(100),
                    Severity::Critical,
                ));
            }
        }

        // Check VPC endpoints
        info!("Checking VPC endpoints");
        match self.network_diagnostics.check_vpc_endpoints(&config.instance_id).await {
            Ok(result) => results.push(result),
            Err(e) => {
                error!("VPC endpoints check failed: {}", e);
                results.push(DiagnosticResult::error(
                    "vpc_prerequisites".to_string(),
                    format!("VPC endpoints check failed: {}", e),
                    Duration::from_millis(100),
                    Severity::High,
                ));
            }
        }

        // Check security groups
        info!("Checking security groups");
        match self.network_diagnostics.check_security_group_rules(&config.instance_id).await {
            Ok(result) => results.push(result),
            Err(e) => {
                error!("Security groups check failed: {}", e);
                results.push(DiagnosticResult::error(
                    "security_group_prerequisites".to_string(),
                    format!("Security groups check failed: {}", e),
                    Duration::from_millis(100),
                    Severity::High,
                ));
            }
        }

        info!("SSM prerequisites verification completed with {} results", results.len());
        Ok(results)
    }

    async fn display_warnings(&self, issues: &[DiagnosticResult]) -> Result<(), Box<dyn std::error::Error>> {
        info!("Displaying warnings for {} detected issues", issues.len());

        for issue in issues {
            match (&issue.status, &issue.severity) {
                (DiagnosticStatus::Error, Severity::Critical) => {
                    error!("🚨 CRITICAL: {} - {}", issue.item_name, issue.message);
                }
                (DiagnosticStatus::Error, Severity::High) => {
                    error!("❌ ERROR: {} - {}", issue.item_name, issue.message);
                }
                (DiagnosticStatus::Warning, _) => {
                    warn!("⚠️ WARNING: {} - {}", issue.item_name, issue.message);
                }
                _ => {
                    info!("ℹ️ INFO: {} - {}", issue.item_name, issue.message);
                }
            }
        }

        Ok(())
    }

    fn calculate_connection_likelihood(&self, results: &[DiagnosticResult]) -> ConnectionLikelihood {
        let critical_count = results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Error && r.severity == Severity::Critical)
            .count();

        let high_error_count = results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Error && r.severity == Severity::High)
            .count();

        let warning_count = results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Warning)
            .count();

        let success_count = results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Success)
            .count();

        // Calculate likelihood based on issue severity and count
        if critical_count > 0 {
            ConnectionLikelihood::VeryLow
        } else if high_error_count >= 2 {
            ConnectionLikelihood::Low
        } else if high_error_count == 1 || warning_count >= 3 {
            ConnectionLikelihood::Medium
        } else if warning_count <= 1 && success_count >= results.len() / 2 {
            ConnectionLikelihood::High
        } else {
            ConnectionLikelihood::Medium
        }
    }

    fn should_abort_connection(&self, results: &[DiagnosticResult], config: &PreventiveCheckConfig) -> bool {
        if !config.abort_on_critical {
            return false;
        }

        // Abort if there are critical issues
        let critical_issues = results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Error && r.severity == Severity::Critical)
            .count();

        if critical_issues > 0 {
            warn!("Aborting connection due to {} critical issues", critical_issues);
            return true;
        }

        // Abort if there are too many high-severity errors
        let high_error_count = results
            .iter()
            .filter(|r| r.status == DiagnosticStatus::Error && r.severity == Severity::High)
            .count();

        if high_error_count >= 3 {
            warn!("Aborting connection due to {} high-severity errors", high_error_count);
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preventive_check_config_creation() {
        let config = PreventiveCheckConfig::new("i-1234567890abcdef0".to_string())
            .with_ports(8080, 80)
            .with_aws_config(Some("us-east-1".to_string()), Some("default".to_string()))
            .with_abort_on_critical(false)
            .with_timeout(Duration::from_secs(60));

        assert_eq!(config.instance_id, "i-1234567890abcdef0");
        assert_eq!(config.local_port, Some(8080));
        assert_eq!(config.remote_port, Some(80));
        assert_eq!(config.region, Some("us-east-1".to_string()));
        assert_eq!(config.profile, Some("default".to_string()));
        assert!(!config.abort_on_critical);
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_connection_likelihood() {
        assert_eq!(ConnectionLikelihood::High.as_percentage(), 90);
        assert_eq!(ConnectionLikelihood::Medium.as_percentage(), 70);
        assert_eq!(ConnectionLikelihood::Low.as_percentage(), 40);
        assert_eq!(ConnectionLikelihood::VeryLow.as_percentage(), 10);

        assert!(ConnectionLikelihood::High.as_description().contains("高い"));
        assert!(ConnectionLikelihood::VeryLow.as_description().contains("非常に低い"));
    }

    #[test]
    fn test_diagnostic_config_conversion() {
        let diagnostic_config = DiagnosticConfig::new("i-1234567890abcdef0".to_string())
            .with_ports(8080, 80)
            .with_aws_config(Some("us-east-1".to_string()), Some("default".to_string()));

        let preventive_config: PreventiveCheckConfig = diagnostic_config.into();

        assert_eq!(preventive_config.instance_id, "i-1234567890abcdef0");
        assert_eq!(preventive_config.local_port, Some(8080));
        assert_eq!(preventive_config.remote_port, Some(80));
        assert!(preventive_config.abort_on_critical);
    }

    #[test]
    fn test_preventive_check_result_creation() {
        let result = PreventiveCheckResult {
            overall_status: PreventiveCheckStatus::Ready,
            connection_likelihood: ConnectionLikelihood::High,
            critical_issues: vec![],
            warnings: vec![],
            recommendations: vec!["すべて正常です".to_string()],
            should_abort_connection: false,
            total_duration: Duration::from_millis(500),
        };

        assert_eq!(result.overall_status, PreventiveCheckStatus::Ready);
        assert_eq!(result.connection_likelihood, ConnectionLikelihood::High);
        assert!(result.critical_issues.is_empty());
        assert!(!result.should_abort_connection);
    }
}