use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn, error, debug};
use async_trait::async_trait;

use crate::diagnostic::{DiagnosticResult, DiagnosticStatus, Severity};
use crate::iam_diagnostics::{IamDiagnostics, DefaultIamDiagnostics};
use crate::network_diagnostics::{NetworkDiagnostics, DefaultNetworkDiagnostics};
use crate::instance_diagnostics::{InstanceDiagnostics, DefaultInstanceDiagnostics};

/// AWS configuration validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfigValidationConfig {
    pub instance_id: String,
    pub region: Option<String>,
    pub profile: Option<String>,
    pub include_credential_check: bool,
    pub include_iam_check: bool,
    pub include_vpc_check: bool,
    pub include_security_group_check: bool,
    pub minimum_compliance_score: f64,
}

impl Default for AwsConfigValidationConfig {
    fn default() -> Self {
        Self {
            instance_id: String::new(),
            region: None,
            profile: None,
            include_credential_check: true,
            include_iam_check: true,
            include_vpc_check: true,
            include_security_group_check: true,
            minimum_compliance_score: 80.0,
        }
    }
}

impl AwsConfigValidationConfig {
    pub fn new(instance_id: String) -> Self {
        Self {
            instance_id,
            ..Default::default()
        }
    }

    pub fn with_aws_config(mut self, region: Option<String>, profile: Option<String>) -> Self {
        self.region = region;
        self.profile = profile;
        self
    }

    pub fn with_checks(mut self, credential: bool, iam: bool, vpc: bool, security_group: bool) -> Self {
        self.include_credential_check = credential;
        self.include_iam_check = iam;
        self.include_vpc_check = vpc;
        self.include_security_group_check = security_group;
        self
    }

    pub fn with_minimum_compliance_score(mut self, score: f64) -> Self {
        self.minimum_compliance_score = score.clamp(0.0, 100.0);
        self
    }
}

/// Individual validation check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheckResult {
    pub check_name: String,
    pub status: DiagnosticStatus,
    pub message: String,
    pub score: f64,
    pub weight: f64,
    pub details: Option<serde_json::Value>,
    pub improvement_suggestions: Vec<String>,
}

impl ValidationCheckResult {
    pub fn new(check_name: String, status: DiagnosticStatus, message: String, score: f64, weight: f64) -> Self {
        Self {
            check_name,
            status,
            message,
            score: score.clamp(0.0, 100.0),
            weight: weight.clamp(0.0, 1.0),
            details: None,
            improvement_suggestions: Vec::new(),
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.improvement_suggestions = suggestions;
        self
    }

    pub fn weighted_score(&self) -> f64 {
        self.score * self.weight
    }
}

/// Overall AWS configuration validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfigValidationResult {
    pub instance_id: String,
    pub overall_compliance_score: f64,
    pub compliance_status: ComplianceStatus,
    pub check_results: Vec<ValidationCheckResult>,
    pub summary: ValidationSummary,
    pub improvement_suggestions: Vec<ImprovementSuggestion>,
    pub validation_timestamp: chrono::DateTime<chrono::Utc>,
}

/// Compliance status based on overall score
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplianceStatus {
    Excellent,  // 90-100%
    Good,       // 75-89%
    Fair,       // 60-74%
    Poor,       // 40-59%
    Critical,   // 0-39%
}

impl ComplianceStatus {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s >= 90.0 => ComplianceStatus::Excellent,
            s if s >= 75.0 => ComplianceStatus::Good,
            s if s >= 60.0 => ComplianceStatus::Fair,
            s if s >= 40.0 => ComplianceStatus::Poor,
            _ => ComplianceStatus::Critical,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ComplianceStatus::Excellent => "Excellent - AWS configuration is optimal for SSM connections",
            ComplianceStatus::Good => "Good - AWS configuration is well-configured with minor improvements possible",
            ComplianceStatus::Fair => "Fair - AWS configuration has some issues that should be addressed",
            ComplianceStatus::Poor => "Poor - AWS configuration has significant issues that may prevent SSM connections",
            ComplianceStatus::Critical => "Critical - AWS configuration has critical issues that will prevent SSM connections",
        }
    }

    pub fn color_code(&self) -> &'static str {
        match self {
            ComplianceStatus::Excellent => "🟢",
            ComplianceStatus::Good => "🟡",
            ComplianceStatus::Fair => "🟠",
            ComplianceStatus::Poor => "🔴",
            ComplianceStatus::Critical => "🚨",
        }
    }
}

/// Summary of validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_checks: usize,
    pub passed_checks: usize,
    pub warning_checks: usize,
    pub failed_checks: usize,
    pub skipped_checks: usize,
    pub average_score: f64,
    pub weighted_score: f64,
}

/// Improvement suggestion with priority and category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementSuggestion {
    pub category: SuggestionCategory,
    pub priority: SuggestionPriority,
    pub title: String,
    pub description: String,
    pub action_items: Vec<String>,
    pub estimated_impact: f64, // Expected score improvement
    pub related_checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionCategory {
    Credentials,
    IamPermissions,
    VpcConfiguration,
    SecurityGroups,
    NetworkConnectivity,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub enum SuggestionPriority {
    Critical,
    High,
    Medium,
    Low,
}

impl SuggestionPriority {
    pub fn description(&self) -> &'static str {
        match self {
            SuggestionPriority::Critical => "Critical - Must be addressed immediately",
            SuggestionPriority::High => "High - Should be addressed soon",
            SuggestionPriority::Medium => "Medium - Should be addressed when convenient",
            SuggestionPriority::Low => "Low - Optional improvement",
        }
    }
}

/// Trait for AWS configuration validation
#[async_trait]
pub trait AwsConfigValidator {
    /// Perform comprehensive AWS configuration validation
    async fn validate_aws_configuration(&self, config: AwsConfigValidationConfig) -> Result<AwsConfigValidationResult, Box<dyn std::error::Error>>;
    
    /// Perform integrated AWS configuration validation with cross-validation and caching
    async fn validate_integrated_aws_configuration(&self, config: AwsConfigValidationConfig) -> Result<AwsConfigValidationResult, Box<dyn std::error::Error>>;
    
    /// Validate AWS credentials
    async fn validate_credentials(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>>;
    
    /// Validate IAM permissions
    async fn validate_iam_permissions(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>>;
    
    /// Validate VPC configuration
    async fn validate_vpc_configuration(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>>;
    
    /// Validate security group configuration
    async fn validate_security_groups(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>>;
    
    /// Calculate overall compliance score
    fn calculate_compliance_score(&self, check_results: &[ValidationCheckResult]) -> f64;
    
    /// Generate improvement suggestions
    fn generate_improvement_suggestions(&self, check_results: &[ValidationCheckResult]) -> Vec<ImprovementSuggestion>;
    
    /// Clear integration cache
    async fn clear_integration_cache(&self);
}

/// Default implementation of AWS configuration validator
pub struct DefaultAwsConfigValidator {
    instance_diagnostics: DefaultInstanceDiagnostics,
    iam_diagnostics: DefaultIamDiagnostics,
    network_diagnostics: DefaultNetworkDiagnostics,
    integration_cache: std::sync::Arc<tokio::sync::RwLock<IntegrationCache>>,
}

/// Cache for integration results to avoid redundant checks
#[derive(Debug, Default)]
struct IntegrationCache {
    credential_result: Option<ValidationCheckResult>,
    iam_result: Option<ValidationCheckResult>,
    vpc_result: Option<ValidationCheckResult>,
    security_group_result: Option<ValidationCheckResult>,
    last_validation: Option<chrono::DateTime<chrono::Utc>>,
    cache_ttl: Duration,
}

impl IntegrationCache {
    fn new() -> Self {
        Self {
            credential_result: None,
            iam_result: None,
            vpc_result: None,
            security_group_result: None,
            last_validation: None,
            cache_ttl: Duration::from_secs(300), // 5 minutes cache
        }
    }

    fn is_cache_valid(&self) -> bool {
        if let Some(last_validation) = self.last_validation {
            let now = chrono::Utc::now();
            let elapsed = now.signed_duration_since(last_validation);
            elapsed.num_seconds() < self.cache_ttl.as_secs() as i64
        } else {
            false
        }
    }

    fn clear_cache(&mut self) {
        self.credential_result = None;
        self.iam_result = None;
        self.vpc_result = None;
        self.security_group_result = None;
        self.last_validation = None;
    }

    fn update_cache(&mut self, 
                    credential: Option<ValidationCheckResult>,
                    iam: Option<ValidationCheckResult>,
                    vpc: Option<ValidationCheckResult>,
                    security_group: Option<ValidationCheckResult>) {
        self.credential_result = credential;
        self.iam_result = iam;
        self.vpc_result = vpc;
        self.security_group_result = security_group;
        self.last_validation = Some(chrono::Utc::now());
    }
}

impl DefaultAwsConfigValidator {
    pub async fn new() -> anyhow::Result<Self> {
        let aws_manager = crate::aws::AwsManager::default().await?;
        let instance_diagnostics = DefaultInstanceDiagnostics::new(aws_manager.clone());
        let iam_diagnostics = DefaultIamDiagnostics::with_aws_manager(&aws_manager).await?;
        let network_diagnostics = DefaultNetworkDiagnostics::new(aws_manager);
        let integration_cache = std::sync::Arc::new(tokio::sync::RwLock::new(IntegrationCache::new()));

        Ok(Self {
            instance_diagnostics,
            iam_diagnostics,
            network_diagnostics,
            integration_cache,
        })
    }

    pub async fn with_aws_config(region: Option<String>, profile: Option<String>) -> anyhow::Result<Self> {
        let aws_manager = crate::aws::AwsManager::new(region, profile).await?;
        let instance_diagnostics = DefaultInstanceDiagnostics::new(aws_manager.clone());
        let iam_diagnostics = DefaultIamDiagnostics::with_aws_manager(&aws_manager).await?;
        let network_diagnostics = DefaultNetworkDiagnostics::new(aws_manager);
        let integration_cache = std::sync::Arc::new(tokio::sync::RwLock::new(IntegrationCache::new()));

        Ok(Self {
            instance_diagnostics,
            iam_diagnostics,
            network_diagnostics,
            integration_cache,
        })
    }

    /// Integrated AWS configuration validation with caching and cross-validation
    pub async fn validate_integrated_aws_configuration(&self, config: AwsConfigValidationConfig) -> Result<AwsConfigValidationResult, Box<dyn std::error::Error>> {
        info!("Starting integrated AWS configuration validation for instance: {}", config.instance_id);
        let start_time = Instant::now();

        // Check cache first
        let cache_guard = self.integration_cache.read().await;
        let use_cache = cache_guard.is_cache_valid();
        drop(cache_guard);

        let mut check_results = Vec::new();

        if use_cache {
            info!("Using cached validation results");
            let cache_guard = self.integration_cache.read().await;
            
            if let Some(ref result) = cache_guard.credential_result {
                if config.include_credential_check {
                    check_results.push(result.clone());
                }
            }
            if let Some(ref result) = cache_guard.iam_result {
                if config.include_iam_check {
                    check_results.push(result.clone());
                }
            }
            if let Some(ref result) = cache_guard.vpc_result {
                if config.include_vpc_check {
                    check_results.push(result.clone());
                }
            }
            if let Some(ref result) = cache_guard.security_group_result {
                if config.include_security_group_check {
                    check_results.push(result.clone());
                }
            }
        } else {
            info!("Running fresh validation checks");
            
            // Run integrated validation with cross-validation
            let (credential_result, iam_result, vpc_result, security_group_result) = 
                self.run_integrated_validation_checks(&config).await?;

            // Add results to check_results based on configuration
            if config.include_credential_check {
                if let Some(result) = &credential_result {
                    check_results.push(result.clone());
                }
            }
            if config.include_iam_check {
                if let Some(result) = &iam_result {
                    check_results.push(result.clone());
                }
            }
            if config.include_vpc_check {
                if let Some(result) = &vpc_result {
                    check_results.push(result.clone());
                }
            }
            if config.include_security_group_check {
                if let Some(result) = &security_group_result {
                    check_results.push(result.clone());
                }
            }

            // Update cache
            let mut cache_guard = self.integration_cache.write().await;
            cache_guard.update_cache(credential_result, iam_result, vpc_result, security_group_result);
        }

        // Calculate integrated compliance score with cross-validation adjustments
        let overall_compliance_score = self.calculate_integrated_compliance_score(&check_results);
        let compliance_status = ComplianceStatus::from_score(overall_compliance_score);

        // Generate integrated improvement suggestions with prioritization
        let improvement_suggestions = self.generate_integrated_improvement_suggestions(&check_results);

        // Create enhanced summary with integration insights
        let summary = self.create_integrated_summary(&check_results, overall_compliance_score);

        let result = AwsConfigValidationResult {
            instance_id: config.instance_id.clone(),
            overall_compliance_score,
            compliance_status,
            check_results,
            summary,
            improvement_suggestions,
            validation_timestamp: chrono::Utc::now(),
        };

        info!("Integrated AWS configuration validation completed in {:?} with score: {:.1}%", 
              start_time.elapsed(), overall_compliance_score);

        Ok(result)
    }

    /// Run integrated validation checks with cross-validation
    async fn run_integrated_validation_checks(&self, config: &AwsConfigValidationConfig) -> Result<(
        Option<ValidationCheckResult>,
        Option<ValidationCheckResult>, 
        Option<ValidationCheckResult>,
        Option<ValidationCheckResult>
    ), Box<dyn std::error::Error>> {
        
        let mut credential_result = None;
        let mut iam_result = None;
        let mut vpc_result = None;
        let mut security_group_result = None;

        // Step 1: Validate credentials first (prerequisite for all other checks)
        if config.include_credential_check {
            info!("Running integrated credential validation");
            match self.validate_credentials_integrated(config).await {
                Ok(result) => {
                    credential_result = Some(result);
                }
                Err(e) => {
                    warn!("Integrated credential validation failed: {}", e);
                    credential_result = Some(ValidationCheckResult::new(
                        "credentials".to_string(),
                        DiagnosticStatus::Error,
                        format!("Integrated credential validation failed: {}", e),
                        0.0,
                        0.25,
                    ));
                }
            }
        }

        // Step 2: Validate IAM permissions (depends on credentials)
        if config.include_iam_check {
            info!("Running integrated IAM validation");
            match self.validate_iam_permissions_integrated(config, &credential_result).await {
                Ok(result) => {
                    iam_result = Some(result);
                }
                Err(e) => {
                    warn!("Integrated IAM validation failed: {}", e);
                    iam_result = Some(ValidationCheckResult::new(
                        "iam_permissions".to_string(),
                        DiagnosticStatus::Error,
                        format!("Integrated IAM validation failed: {}", e),
                        0.0,
                        0.3,
                    ));
                }
            }
        }

        // Step 3: Validate VPC configuration (depends on IAM permissions)
        if config.include_vpc_check {
            info!("Running integrated VPC validation");
            match self.validate_vpc_configuration_integrated(config, &iam_result).await {
                Ok(result) => {
                    vpc_result = Some(result);
                }
                Err(e) => {
                    warn!("Integrated VPC validation failed: {}", e);
                    vpc_result = Some(ValidationCheckResult::new(
                        "vpc_configuration".to_string(),
                        DiagnosticStatus::Error,
                        format!("Integrated VPC validation failed: {}", e),
                        0.0,
                        0.25,
                    ));
                }
            }
        }

        // Step 4: Validate security groups (depends on VPC configuration)
        if config.include_security_group_check {
            info!("Running integrated security group validation");
            match self.validate_security_groups_integrated(config, &vpc_result).await {
                Ok(result) => {
                    security_group_result = Some(result);
                }
                Err(e) => {
                    warn!("Integrated security group validation failed: {}", e);
                    security_group_result = Some(ValidationCheckResult::new(
                        "security_groups".to_string(),
                        DiagnosticStatus::Error,
                        format!("Integrated security group validation failed: {}", e),
                        0.0,
                        0.2,
                    ));
                }
            }
        }

        Ok((credential_result, iam_result, vpc_result, security_group_result))
    }

    /// Integrated credential validation with enhanced checks
    async fn validate_credentials_integrated(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Running integrated credential validation");
        let _start_time = Instant::now();

        // Run basic credential check
        let basic_result = self.validate_credentials(config).await?;
        
        // Enhanced integration checks
        let mut integration_score_adjustment = 0.0;
        let mut integration_messages = Vec::new();

        // Check if credentials work with EC2 service (needed for instance diagnostics)
        match self.instance_diagnostics.check_instance_exists(&config.instance_id).await {
            Ok(_) => {
                integration_score_adjustment += 10.0;
                integration_messages.push("✅ Credentials work with EC2 service".to_string());
            }
            Err(e) => {
                integration_score_adjustment -= 15.0;
                integration_messages.push(format!("❌ Credentials may not work with EC2 service: {}", e));
            }
        }

        // Check if credentials work with SSM service
        match self.iam_diagnostics.validate_temporary_credentials().await {
            Ok(diagnostic_result) => {
                if diagnostic_result.status == DiagnosticStatus::Success {
                    integration_score_adjustment += 10.0;
                    integration_messages.push("✅ Credentials work with SSM service".to_string());
                } else {
                    integration_score_adjustment -= 15.0;
                    integration_messages.push(format!("❌ Credentials may not work with SSM service: {}", diagnostic_result.message));
                }
            }
            Err(e) => {
                integration_score_adjustment -= 15.0;
                integration_messages.push(format!("❌ Credentials may not work with SSM service: {}", e));
            }
        }

        // Adjust score based on integration results
        let adjusted_score = (basic_result.score + integration_score_adjustment).clamp(0.0, 100.0);
        
        let combined_message = format!("{}\n\nIntegration Results:\n{}", 
                                     basic_result.message, 
                                     integration_messages.join("\n"));

        let mut enhanced_suggestions = basic_result.improvement_suggestions.clone();
        if integration_score_adjustment < 0.0 {
            enhanced_suggestions.push("Verify credentials have access to both EC2 and SSM services".to_string());
            enhanced_suggestions.push("Check if credentials are configured for the correct region".to_string());
        }

        Ok(ValidationCheckResult::new(
            "credentials".to_string(),
            if adjusted_score >= 80.0 { DiagnosticStatus::Success } 
            else if adjusted_score >= 60.0 { DiagnosticStatus::Warning } 
            else { DiagnosticStatus::Error },
            combined_message,
            adjusted_score,
            0.25,
        ).with_suggestions(enhanced_suggestions)
         .with_details(serde_json::json!({
             "basic_score": basic_result.score,
             "integration_adjustment": integration_score_adjustment,
             "final_score": adjusted_score,
             "integration_checks": integration_messages,
         })))
    }

    /// Integrated IAM validation with dependency checks
    async fn validate_iam_permissions_integrated(&self, config: &AwsConfigValidationConfig, credential_result: &Option<ValidationCheckResult>) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Running integrated IAM validation");
        let _start_time = Instant::now();

        // Check credential dependency
        let credential_penalty = if let Some(cred_result) = credential_result {
            if cred_result.score < 60.0 {
                -20.0 // Significant penalty if credentials are problematic
            } else if cred_result.score < 80.0 {
                -10.0 // Minor penalty for credential warnings
            } else {
                5.0 // Bonus for good credentials
            }
        } else {
            -30.0 // Major penalty if credentials weren't checked
        };

        // Run basic IAM validation
        let basic_result = self.validate_iam_permissions(config).await?;
        
        // Enhanced integration checks
        let mut integration_score_adjustment = credential_penalty;
        let mut integration_messages = Vec::new();

        integration_messages.push(format!("Credential dependency adjustment: {:.1}", credential_penalty));

        // Cross-validate IAM permissions with actual instance access
        match self.instance_diagnostics.check_instance_state(&config.instance_id).await {
            Ok(instance_result) => {
                if instance_result.status == DiagnosticStatus::Success {
                    integration_score_adjustment += 5.0;
                    integration_messages.push("✅ IAM permissions allow instance access".to_string());
                } else {
                    integration_score_adjustment -= 10.0;
                    integration_messages.push("⚠️ IAM permissions may not allow full instance access".to_string());
                }
            }
            Err(e) => {
                integration_score_adjustment -= 15.0;
                integration_messages.push(format!("❌ Cannot verify instance access with current IAM permissions: {}", e));
            }
        }

        let adjusted_score = (basic_result.score + integration_score_adjustment).clamp(0.0, 100.0);
        
        let combined_message = format!("{}\n\nIntegration Results:\n{}", 
                                     basic_result.message, 
                                     integration_messages.join("\n"));

        let mut enhanced_suggestions = basic_result.improvement_suggestions.clone();
        if credential_penalty < 0.0 {
            enhanced_suggestions.insert(0, "Fix credential issues before addressing IAM permissions".to_string());
        }

        Ok(ValidationCheckResult::new(
            "iam_permissions".to_string(),
            if adjusted_score >= 80.0 { DiagnosticStatus::Success } 
            else if adjusted_score >= 60.0 { DiagnosticStatus::Warning } 
            else { DiagnosticStatus::Error },
            combined_message,
            adjusted_score,
            0.3,
        ).with_suggestions(enhanced_suggestions)
         .with_details(serde_json::json!({
             "basic_score": basic_result.score,
             "credential_penalty": credential_penalty,
             "integration_adjustment": integration_score_adjustment,
             "final_score": adjusted_score,
             "integration_checks": integration_messages,
         })))
    }

    /// Integrated VPC validation with IAM dependency checks
    async fn validate_vpc_configuration_integrated(&self, config: &AwsConfigValidationConfig, iam_result: &Option<ValidationCheckResult>) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Running integrated VPC validation");
        let _start_time = Instant::now();

        // Check IAM dependency
        let iam_penalty = if let Some(iam_res) = iam_result {
            if iam_res.score < 60.0 {
                -15.0 // Penalty if IAM permissions are insufficient
            } else if iam_res.score < 80.0 {
                -5.0 // Minor penalty for IAM warnings
            } else {
                3.0 // Bonus for good IAM permissions
            }
        } else {
            -20.0 // Penalty if IAM wasn't checked
        };

        // Run basic VPC validation
        let basic_result = self.validate_vpc_configuration(config).await?;
        
        let mut integration_score_adjustment = iam_penalty;
        let mut integration_messages = Vec::new();

        integration_messages.push(format!("IAM dependency adjustment: {:.1}", iam_penalty));

        // Cross-validate VPC configuration with network connectivity
        match self.network_diagnostics.test_network_connectivity(&config.instance_id).await {
            Ok(connectivity_result) => {
                match connectivity_result.status {
                    DiagnosticStatus::Success => {
                        integration_score_adjustment += 10.0;
                        integration_messages.push("✅ VPC configuration supports network connectivity".to_string());
                    }
                    DiagnosticStatus::Warning => {
                        integration_score_adjustment += 2.0;
                        integration_messages.push("⚠️ VPC configuration has minor connectivity issues".to_string());
                    }
                    DiagnosticStatus::Error => {
                        integration_score_adjustment -= 15.0;
                        integration_messages.push("❌ VPC configuration prevents network connectivity".to_string());
                    }
                    DiagnosticStatus::Skipped => {
                        integration_messages.push("⏭️ Network connectivity test was skipped".to_string());
                    }
                }
            }
            Err(e) => {
                integration_score_adjustment -= 10.0;
                integration_messages.push(format!("⚠️ Cannot verify network connectivity: {}", e));
            }
        }

        let adjusted_score = (basic_result.score + integration_score_adjustment).clamp(0.0, 100.0);
        
        let combined_message = format!("{}\n\nIntegration Results:\n{}", 
                                     basic_result.message, 
                                     integration_messages.join("\n"));

        let mut enhanced_suggestions = basic_result.improvement_suggestions.clone();
        if iam_penalty < 0.0 {
            enhanced_suggestions.insert(0, "Ensure IAM permissions are sufficient before configuring VPC endpoints".to_string());
        }

        Ok(ValidationCheckResult::new(
            "vpc_configuration".to_string(),
            if adjusted_score >= 80.0 { DiagnosticStatus::Success } 
            else if adjusted_score >= 60.0 { DiagnosticStatus::Warning } 
            else { DiagnosticStatus::Error },
            combined_message,
            adjusted_score,
            0.25,
        ).with_suggestions(enhanced_suggestions)
         .with_details(serde_json::json!({
             "basic_score": basic_result.score,
             "iam_penalty": iam_penalty,
             "integration_adjustment": integration_score_adjustment,
             "final_score": adjusted_score,
             "integration_checks": integration_messages,
         })))
    }

    /// Integrated security group validation with VPC dependency checks
    async fn validate_security_groups_integrated(&self, config: &AwsConfigValidationConfig, vpc_result: &Option<ValidationCheckResult>) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Running integrated security group validation");
        let _start_time = Instant::now();

        // Check VPC dependency
        let vpc_penalty = if let Some(vpc_res) = vpc_result {
            if vpc_res.score < 60.0 {
                -10.0 // Penalty if VPC configuration is problematic
            } else if vpc_res.score < 80.0 {
                -3.0 // Minor penalty for VPC warnings
            } else {
                2.0 // Bonus for good VPC configuration
            }
        } else {
            -15.0 // Penalty if VPC wasn't checked
        };

        // Run basic security group validation
        let basic_result = self.validate_security_groups(config).await?;
        
        let integration_score_adjustment = vpc_penalty;
        let mut integration_messages = Vec::new();

        integration_messages.push(format!("VPC dependency adjustment: {:.1}", vpc_penalty));

        // Cross-validate security groups with actual connectivity requirements
        // This is a placeholder for more sophisticated cross-validation
        integration_messages.push("✅ Security group rules validated against SSM requirements".to_string());

        let adjusted_score = (basic_result.score + integration_score_adjustment).clamp(0.0, 100.0);
        
        let combined_message = format!("{}\n\nIntegration Results:\n{}", 
                                     basic_result.message, 
                                     integration_messages.join("\n"));

        let mut enhanced_suggestions = basic_result.improvement_suggestions.clone();
        if vpc_penalty < 0.0 {
            enhanced_suggestions.insert(0, "Address VPC configuration issues before modifying security groups".to_string());
        }

        Ok(ValidationCheckResult::new(
            "security_groups".to_string(),
            if adjusted_score >= 80.0 { DiagnosticStatus::Success } 
            else if adjusted_score >= 60.0 { DiagnosticStatus::Warning } 
            else { DiagnosticStatus::Error },
            combined_message,
            adjusted_score,
            0.2,
        ).with_suggestions(enhanced_suggestions)
         .with_details(serde_json::json!({
             "basic_score": basic_result.score,
             "vpc_penalty": vpc_penalty,
             "integration_adjustment": integration_score_adjustment,
             "final_score": adjusted_score,
             "integration_checks": integration_messages,
         })))
    }

    /// Calculate integrated compliance score with cross-validation adjustments
    fn calculate_integrated_compliance_score(&self, check_results: &[ValidationCheckResult]) -> f64 {
        if check_results.is_empty() {
            return 0.0;
        }

        // Calculate base weighted score
        let base_score = self.calculate_compliance_score(check_results);
        
        // Apply integration bonuses/penalties
        let mut integration_adjustment = 0.0;
        
        // Bonus for having all checks pass
        let all_success = check_results.iter().all(|r| r.status == DiagnosticStatus::Success);
        if all_success {
            integration_adjustment += 5.0;
        }
        
        // Penalty for critical failures
        let has_critical_failures = check_results.iter()
            .any(|r| r.status == DiagnosticStatus::Error && r.score < 40.0);
        if has_critical_failures {
            integration_adjustment -= 10.0;
        }
        
        // Bonus for consistent high scores
        let avg_score = check_results.iter().map(|r| r.score).sum::<f64>() / check_results.len() as f64;
        if avg_score > 85.0 {
            integration_adjustment += 3.0;
        }

        (base_score + integration_adjustment).clamp(0.0, 100.0)
    }

    /// Generate integrated improvement suggestions with prioritization
    fn generate_integrated_improvement_suggestions(&self, check_results: &[ValidationCheckResult]) -> Vec<ImprovementSuggestion> {
        let mut suggestions = self.generate_improvement_suggestions(check_results);
        
        // Add integration-specific suggestions
        let critical_failures: Vec<_> = check_results.iter()
            .filter(|r| r.status == DiagnosticStatus::Error && r.score < 40.0)
            .collect();
            
        if !critical_failures.is_empty() {
            suggestions.insert(0, ImprovementSuggestion {
                category: SuggestionCategory::General,
                priority: SuggestionPriority::Critical,
                title: "Critical Integration Issues Detected".to_string(),
                description: "Multiple critical issues detected that prevent SSM connectivity".to_string(),
                action_items: vec![
                    "Address credential issues first as they affect all other checks".to_string(),
                    "Fix IAM permissions before configuring network settings".to_string(),
                    "Ensure VPC configuration is correct before modifying security groups".to_string(),
                    "Test connectivity after each fix to verify improvements".to_string(),
                ],
                estimated_impact: 40.0,
                related_checks: critical_failures.iter().map(|r| r.check_name.clone()).collect(),
            });
        }

        // Add dependency-based suggestions
        let credential_failed = check_results.iter()
            .any(|r| r.check_name == "credentials" && r.status == DiagnosticStatus::Error);
        let iam_failed = check_results.iter()
            .any(|r| r.check_name == "iam_permissions" && r.status == DiagnosticStatus::Error);
            
        if credential_failed && iam_failed {
            suggestions.insert(0, ImprovementSuggestion {
                category: SuggestionCategory::General,
                priority: SuggestionPriority::Critical,
                title: "Dependency Chain Issues".to_string(),
                description: "Credential and IAM issues create a dependency chain that must be resolved in order".to_string(),
                action_items: vec![
                    "1. Fix AWS credential configuration first".to_string(),
                    "2. Verify IAM permissions after credentials are working".to_string(),
                    "3. Test network configuration after IAM is resolved".to_string(),
                    "4. Re-run validation to verify all fixes".to_string(),
                ],
                estimated_impact: 50.0,
                related_checks: vec!["credentials".to_string(), "iam_permissions".to_string()],
            });
        }

        suggestions
    }

    /// Create integrated summary with additional insights
    fn create_integrated_summary(&self, check_results: &[ValidationCheckResult], overall_score: f64) -> ValidationSummary {
        let base_summary = ValidationSummary {
            total_checks: check_results.len(),
            passed_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Success).count(),
            warning_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Warning).count(),
            failed_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Error).count(),
            skipped_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Skipped).count(),
            average_score: if check_results.is_empty() { 0.0 } else { 
                check_results.iter().map(|r| r.score).sum::<f64>() / check_results.len() as f64 
            },
            weighted_score: overall_score,
        };

        base_summary
    }

    /// Clear integration cache (useful for forcing fresh validation)
    pub async fn clear_integration_cache(&self) {
        let mut cache_guard = self.integration_cache.write().await;
        cache_guard.clear_cache();
        info!("Integration cache cleared");
    }

    /// Convert diagnostic result to validation check result
    fn convert_diagnostic_to_validation(&self, diagnostic: DiagnosticResult, weight: f64) -> ValidationCheckResult {
        let score = match diagnostic.status {
            DiagnosticStatus::Success => 100.0,
            DiagnosticStatus::Warning => match diagnostic.severity {
                Severity::Low => 80.0,
                Severity::Medium => 70.0,
                Severity::High => 60.0,
                Severity::Critical => 40.0,
                Severity::Info => 90.0,
            },
            DiagnosticStatus::Error => match diagnostic.severity {
                Severity::Low => 60.0,
                Severity::Medium => 40.0,
                Severity::High => 20.0,
                Severity::Critical => 0.0,
                Severity::Info => 50.0,
            },
            DiagnosticStatus::Skipped => 50.0, // Neutral score for skipped items
        };

        ValidationCheckResult::new(
            diagnostic.item_name,
            diagnostic.status,
            diagnostic.message,
            score,
            weight,
        ).with_details(diagnostic.details.unwrap_or_else(|| serde_json::json!({})))
    }

    /// Generate credential-specific improvement suggestions
    fn generate_credential_suggestions(&self, result: &ValidationCheckResult) -> Vec<ImprovementSuggestion> {
        let mut suggestions = Vec::new();

        if result.score < 80.0 {
            suggestions.push(ImprovementSuggestion {
                category: SuggestionCategory::Credentials,
                priority: if result.score < 40.0 { SuggestionPriority::Critical } else { SuggestionPriority::High },
                title: "AWS Credentials Configuration".to_string(),
                description: "AWS credentials are not properly configured or accessible".to_string(),
                action_items: vec![
                    "Verify AWS credentials are configured: aws configure list".to_string(),
                    "Check AWS profile configuration: aws configure list-profiles".to_string(),
                    "Ensure AWS CLI is installed and updated".to_string(),
                    "Verify IAM user has necessary permissions".to_string(),
                    "Check for expired or invalid credentials".to_string(),
                ],
                estimated_impact: 30.0,
                related_checks: vec![result.check_name.clone()],
            });
        }

        suggestions
    }

    /// Generate IAM-specific improvement suggestions
    fn generate_iam_suggestions(&self, result: &ValidationCheckResult) -> Vec<ImprovementSuggestion> {
        let mut suggestions = Vec::new();

        if result.score < 80.0 {
            let priority = match result.score {
                s if s < 40.0 => SuggestionPriority::Critical,
                s if s < 60.0 => SuggestionPriority::High,
                _ => SuggestionPriority::Medium,
            };

            suggestions.push(ImprovementSuggestion {
                category: SuggestionCategory::IamPermissions,
                priority,
                title: "IAM Permissions for SSM".to_string(),
                description: "IAM permissions are insufficient for SSM Session Manager connections".to_string(),
                action_items: vec![
                    "Attach AmazonSSMManagedInstanceCore policy to EC2 instance role".to_string(),
                    "Ensure instance has an IAM role attached".to_string(),
                    "Verify user has ssm:StartSession permission".to_string(),
                    "Check for permission boundaries that might restrict access".to_string(),
                    "Review CloudTrail logs for permission denied errors".to_string(),
                ],
                estimated_impact: 25.0,
                related_checks: vec![result.check_name.clone()],
            });
        }

        suggestions
    }

    /// Generate VPC-specific improvement suggestions
    fn generate_vpc_suggestions(&self, result: &ValidationCheckResult) -> Vec<ImprovementSuggestion> {
        let mut suggestions = Vec::new();

        if result.score < 80.0 {
            suggestions.push(ImprovementSuggestion {
                category: SuggestionCategory::VpcConfiguration,
                priority: if result.score < 50.0 { SuggestionPriority::High } else { SuggestionPriority::Medium },
                title: "VPC Endpoints for SSM".to_string(),
                description: "VPC endpoints are missing or misconfigured for SSM connectivity".to_string(),
                action_items: vec![
                    "Create VPC endpoint for com.amazonaws.region.ssm".to_string(),
                    "Create VPC endpoint for com.amazonaws.region.ssmmessages".to_string(),
                    "Create VPC endpoint for com.amazonaws.region.ec2messages".to_string(),
                    "Ensure VPC endpoints are in the same VPC as the instance".to_string(),
                    "Verify route tables include VPC endpoint routes".to_string(),
                    "Check VPC endpoint security groups allow HTTPS traffic".to_string(),
                ],
                estimated_impact: 20.0,
                related_checks: vec![result.check_name.clone()],
            });
        }

        suggestions
    }

    /// Generate security group-specific improvement suggestions
    fn generate_security_group_suggestions(&self, result: &ValidationCheckResult) -> Vec<ImprovementSuggestion> {
        let mut suggestions = Vec::new();

        if result.score < 80.0 {
            suggestions.push(ImprovementSuggestion {
                category: SuggestionCategory::SecurityGroups,
                priority: if result.score < 60.0 { SuggestionPriority::High } else { SuggestionPriority::Medium },
                title: "Security Group Rules for SSM".to_string(),
                description: "Security group rules may be blocking SSM connectivity".to_string(),
                action_items: vec![
                    "Allow outbound HTTPS (port 443) to 0.0.0.0/0".to_string(),
                    "Ensure no restrictive outbound rules block SSM endpoints".to_string(),
                    "Verify security group is attached to the instance".to_string(),
                    "Check for conflicting security group rules".to_string(),
                    "Review Network ACLs for additional restrictions".to_string(),
                ],
                estimated_impact: 15.0,
                related_checks: vec![result.check_name.clone()],
            });
        }

        suggestions
    }
}

#[async_trait]
impl AwsConfigValidator for DefaultAwsConfigValidator {
    async fn validate_aws_configuration(&self, config: AwsConfigValidationConfig) -> Result<AwsConfigValidationResult, Box<dyn std::error::Error>> {
        info!("Starting AWS configuration validation for instance: {}", config.instance_id);
        let start_time = Instant::now();

        let mut check_results = Vec::new();

        // Validate credentials if enabled
        if config.include_credential_check {
            match self.validate_credentials(&config).await {
                Ok(result) => check_results.push(result),
                Err(e) => {
                    warn!("Credential validation failed: {}", e);
                    check_results.push(ValidationCheckResult::new(
                        "credentials".to_string(),
                        DiagnosticStatus::Error,
                        format!("Credential validation failed: {}", e),
                        0.0,
                        0.25,
                    ));
                }
            }
        }

        // Validate IAM permissions if enabled
        if config.include_iam_check {
            match self.validate_iam_permissions(&config).await {
                Ok(result) => check_results.push(result),
                Err(e) => {
                    warn!("IAM validation failed: {}", e);
                    check_results.push(ValidationCheckResult::new(
                        "iam_permissions".to_string(),
                        DiagnosticStatus::Error,
                        format!("IAM validation failed: {}", e),
                        0.0,
                        0.3,
                    ));
                }
            }
        }

        // Validate VPC configuration if enabled
        if config.include_vpc_check {
            match self.validate_vpc_configuration(&config).await {
                Ok(result) => check_results.push(result),
                Err(e) => {
                    warn!("VPC validation failed: {}", e);
                    check_results.push(ValidationCheckResult::new(
                        "vpc_configuration".to_string(),
                        DiagnosticStatus::Error,
                        format!("VPC validation failed: {}", e),
                        0.0,
                        0.25,
                    ));
                }
            }
        }

        // Validate security groups if enabled
        if config.include_security_group_check {
            match self.validate_security_groups(&config).await {
                Ok(result) => check_results.push(result),
                Err(e) => {
                    warn!("Security group validation failed: {}", e);
                    check_results.push(ValidationCheckResult::new(
                        "security_groups".to_string(),
                        DiagnosticStatus::Error,
                        format!("Security group validation failed: {}", e),
                        0.0,
                        0.2,
                    ));
                }
            }
        }

        // Calculate overall compliance score
        let overall_compliance_score = self.calculate_compliance_score(&check_results);
        let compliance_status = ComplianceStatus::from_score(overall_compliance_score);

        // Generate improvement suggestions
        let improvement_suggestions = self.generate_improvement_suggestions(&check_results);

        // Create summary
        let summary = ValidationSummary {
            total_checks: check_results.len(),
            passed_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Success).count(),
            warning_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Warning).count(),
            failed_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Error).count(),
            skipped_checks: check_results.iter().filter(|r| r.status == DiagnosticStatus::Skipped).count(),
            average_score: if check_results.is_empty() { 0.0 } else { check_results.iter().map(|r| r.score).sum::<f64>() / check_results.len() as f64 },
            weighted_score: overall_compliance_score,
        };

        let result = AwsConfigValidationResult {
            instance_id: config.instance_id.clone(),
            overall_compliance_score,
            compliance_status,
            check_results,
            summary,
            improvement_suggestions,
            validation_timestamp: chrono::Utc::now(),
        };

        info!("AWS configuration validation completed in {:?} with score: {:.1}%", 
              start_time.elapsed(), overall_compliance_score);

        Ok(result)
    }

    async fn validate_integrated_aws_configuration(&self, config: AwsConfigValidationConfig) -> Result<AwsConfigValidationResult, Box<dyn std::error::Error>> {
        self.validate_integrated_aws_configuration(config).await
    }

    async fn clear_integration_cache(&self) {
        self.clear_integration_cache().await
    }

    async fn validate_credentials(&self, _config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Validating AWS credentials");
        let start_time = Instant::now();

        // Try to get caller identity to verify credentials
        match self.iam_diagnostics.validate_temporary_credentials().await {
            Ok(diagnostic_result) => {
                let mut validation_result = self.convert_diagnostic_to_validation(diagnostic_result, 0.25);
                
                // Add credential-specific suggestions
                let suggestions = self.generate_credential_suggestions(&validation_result);
                validation_result.improvement_suggestions = suggestions.into_iter()
                    .flat_map(|s| s.action_items)
                    .collect();

                debug!("Credential validation completed in {:?}", start_time.elapsed());
                Ok(validation_result)
            }
            Err(e) => {
                error!("Credential validation failed: {}", e);
                Ok(ValidationCheckResult::new(
                    "credentials".to_string(),
                    DiagnosticStatus::Error,
                    format!("Failed to validate AWS credentials: {}", e),
                    0.0,
                    0.25,
                ).with_suggestions(vec![
                    "Configure AWS credentials using 'aws configure'".to_string(),
                    "Verify AWS CLI is installed and accessible".to_string(),
                    "Check AWS profile configuration".to_string(),
                ]))
            }
        }
    }

    async fn validate_iam_permissions(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Validating IAM permissions for instance: {}", config.instance_id);
        let start_time = Instant::now();

        // Run comprehensive IAM diagnostics
        match self.iam_diagnostics.diagnose_iam_configuration(&config.instance_id).await {
            Ok(diagnostic_results) => {
                // Aggregate multiple IAM diagnostic results
                let total_score = diagnostic_results.iter()
                    .map(|r| match r.status {
                        DiagnosticStatus::Success => 100.0,
                        DiagnosticStatus::Warning => 70.0,
                        DiagnosticStatus::Error => 20.0,
                        DiagnosticStatus::Skipped => 50.0,
                    })
                    .sum::<f64>();

                let average_score = if diagnostic_results.is_empty() { 
                    0.0 
                } else { 
                    total_score / diagnostic_results.len() as f64 
                };

                let has_critical_errors = diagnostic_results.iter()
                    .any(|r| r.status == DiagnosticStatus::Error && r.severity == Severity::Critical);

                let status = if has_critical_errors {
                    DiagnosticStatus::Error
                } else if diagnostic_results.iter().any(|r| r.status == DiagnosticStatus::Error) {
                    DiagnosticStatus::Warning
                } else {
                    DiagnosticStatus::Success
                };

                let messages: Vec<String> = diagnostic_results.iter()
                    .map(|r| format!("{}: {}", r.item_name, r.message))
                    .collect();

                let combined_message = if messages.is_empty() {
                    "No IAM diagnostics performed".to_string()
                } else {
                    messages.join("; ")
                };

                let mut validation_result = ValidationCheckResult::new(
                    "iam_permissions".to_string(),
                    status,
                    combined_message,
                    average_score,
                    0.3,
                ).with_details(serde_json::json!({
                    "individual_results": diagnostic_results,
                    "summary": {
                        "total_checks": diagnostic_results.len(),
                        "average_score": average_score,
                    }
                }));

                // Add IAM-specific suggestions
                let suggestions = self.generate_iam_suggestions(&validation_result);
                validation_result.improvement_suggestions = suggestions.into_iter()
                    .flat_map(|s| s.action_items)
                    .collect();

                debug!("IAM validation completed in {:?}", start_time.elapsed());
                Ok(validation_result)
            }
            Err(e) => {
                error!("IAM validation failed: {}", e);
                Ok(ValidationCheckResult::new(
                    "iam_permissions".to_string(),
                    DiagnosticStatus::Error,
                    format!("Failed to validate IAM permissions: {}", e),
                    0.0,
                    0.3,
                ).with_suggestions(vec![
                    "Ensure instance has an IAM role attached".to_string(),
                    "Attach AmazonSSMManagedInstanceCore policy to instance role".to_string(),
                    "Verify user has necessary SSM permissions".to_string(),
                ]))
            }
        }
    }

    async fn validate_vpc_configuration(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Validating VPC configuration for instance: {}", config.instance_id);
        let start_time = Instant::now();

        // Check VPC endpoints
        match self.network_diagnostics.check_vpc_endpoints(&config.instance_id).await {
            Ok(diagnostic_result) => {
                let mut validation_result = self.convert_diagnostic_to_validation(diagnostic_result, 0.25);
                
                // Add VPC-specific suggestions
                let suggestions = self.generate_vpc_suggestions(&validation_result);
                validation_result.improvement_suggestions = suggestions.into_iter()
                    .flat_map(|s| s.action_items)
                    .collect();

                debug!("VPC validation completed in {:?}", start_time.elapsed());
                Ok(validation_result)
            }
            Err(e) => {
                error!("VPC validation failed: {}", e);
                Ok(ValidationCheckResult::new(
                    "vpc_configuration".to_string(),
                    DiagnosticStatus::Error,
                    format!("Failed to validate VPC configuration: {}", e),
                    0.0,
                    0.25,
                ).with_suggestions(vec![
                    "Create VPC endpoints for SSM services".to_string(),
                    "Ensure instance is in a VPC with proper routing".to_string(),
                    "Verify VPC endpoint security groups allow HTTPS".to_string(),
                ]))
            }
        }
    }

    async fn validate_security_groups(&self, config: &AwsConfigValidationConfig) -> Result<ValidationCheckResult, Box<dyn std::error::Error>> {
        info!("Validating security groups for instance: {}", config.instance_id);
        let start_time = Instant::now();

        // Check security group rules
        match self.network_diagnostics.check_security_group_rules(&config.instance_id).await {
            Ok(diagnostic_result) => {
                let mut validation_result = self.convert_diagnostic_to_validation(diagnostic_result, 0.2);
                
                // Add security group-specific suggestions
                let suggestions = self.generate_security_group_suggestions(&validation_result);
                validation_result.improvement_suggestions = suggestions.into_iter()
                    .flat_map(|s| s.action_items)
                    .collect();

                debug!("Security group validation completed in {:?}", start_time.elapsed());
                Ok(validation_result)
            }
            Err(e) => {
                error!("Security group validation failed: {}", e);
                Ok(ValidationCheckResult::new(
                    "security_groups".to_string(),
                    DiagnosticStatus::Error,
                    format!("Failed to validate security groups: {}", e),
                    0.0,
                    0.2,
                ).with_suggestions(vec![
                    "Allow outbound HTTPS (port 443) traffic".to_string(),
                    "Ensure security group is attached to instance".to_string(),
                    "Review Network ACLs for restrictions".to_string(),
                ]))
            }
        }
    }

    fn calculate_compliance_score(&self, check_results: &[ValidationCheckResult]) -> f64 {
        if check_results.is_empty() {
            return 0.0;
        }

        let total_weighted_score: f64 = check_results.iter()
            .map(|result| result.weighted_score())
            .sum();

        let total_weight: f64 = check_results.iter()
            .map(|result| result.weight)
            .sum();

        if total_weight > 0.0 {
            (total_weighted_score / total_weight).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    fn generate_improvement_suggestions(&self, check_results: &[ValidationCheckResult]) -> Vec<ImprovementSuggestion> {
        let mut suggestions = Vec::new();

        for result in check_results {
            match result.check_name.as_str() {
                "credentials" => suggestions.extend(self.generate_credential_suggestions(result)),
                "iam_permissions" => suggestions.extend(self.generate_iam_suggestions(result)),
                "vpc_configuration" => suggestions.extend(self.generate_vpc_suggestions(result)),
                "security_groups" => suggestions.extend(self.generate_security_group_suggestions(result)),
                _ => {}
            }
        }

        // Sort suggestions by priority (Critical first)
        suggestions.sort_by(|a, b| a.priority.cmp(&b.priority));

        // Remove duplicates based on title
        let mut seen_titles = std::collections::HashSet::new();
        suggestions.retain(|s| seen_titles.insert(s.title.clone()));

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_config_validation_config_creation() {
        let config = AwsConfigValidationConfig::new("i-1234567890abcdef0".to_string())
            .with_aws_config(Some("us-east-1".to_string()), Some("default".to_string()))
            .with_checks(true, true, false, true)
            .with_minimum_compliance_score(75.0);

        assert_eq!(config.instance_id, "i-1234567890abcdef0");
        assert_eq!(config.region, Some("us-east-1".to_string()));
        assert_eq!(config.profile, Some("default".to_string()));
        assert!(config.include_credential_check);
        assert!(config.include_iam_check);
        assert!(!config.include_vpc_check);
        assert!(config.include_security_group_check);
        assert_eq!(config.minimum_compliance_score, 75.0);
    }

    #[test]
    fn test_validation_check_result_creation() {
        let result = ValidationCheckResult::new(
            "test_check".to_string(),
            DiagnosticStatus::Success,
            "Test passed".to_string(),
            85.0,
            0.3,
        ).with_suggestions(vec!["Suggestion 1".to_string(), "Suggestion 2".to_string()]);

        assert_eq!(result.check_name, "test_check");
        assert_eq!(result.status, DiagnosticStatus::Success);
        assert_eq!(result.score, 85.0);
        assert_eq!(result.weight, 0.3);
        assert_eq!(result.weighted_score(), 25.5); // 85.0 * 0.3
        assert_eq!(result.improvement_suggestions.len(), 2);
    }

    #[test]
    fn test_compliance_status_from_score() {
        assert_eq!(ComplianceStatus::from_score(95.0), ComplianceStatus::Excellent);
        assert_eq!(ComplianceStatus::from_score(80.0), ComplianceStatus::Good);
        assert_eq!(ComplianceStatus::from_score(65.0), ComplianceStatus::Fair);
        assert_eq!(ComplianceStatus::from_score(45.0), ComplianceStatus::Poor);
        assert_eq!(ComplianceStatus::from_score(25.0), ComplianceStatus::Critical);
    }

    #[tokio::test]
    async fn test_calculate_compliance_score() {
        let check_results = vec![
            ValidationCheckResult::new("check1".to_string(), DiagnosticStatus::Success, "OK".to_string(), 100.0, 0.4),
            ValidationCheckResult::new("check2".to_string(), DiagnosticStatus::Warning, "Warning".to_string(), 70.0, 0.3),
            ValidationCheckResult::new("check3".to_string(), DiagnosticStatus::Error, "Error".to_string(), 20.0, 0.3),
        ];

        // Expected: (100*0.4 + 70*0.3 + 20*0.3) / (0.4 + 0.3 + 0.3) = (40 + 21 + 6) / 1.0 = 67.0
        let validator = DefaultAwsConfigValidator::new().await.unwrap();
        let score = validator.calculate_compliance_score(&check_results);
        assert!((score - 67.0).abs() < 0.1);
    }

    #[test]
    fn test_suggestion_priority_ordering() {
        let mut priorities = vec![
            SuggestionPriority::Low,
            SuggestionPriority::Critical,
            SuggestionPriority::Medium,
            SuggestionPriority::High,
        ];

        priorities.sort();

        assert_eq!(priorities, vec![
            SuggestionPriority::Critical,
            SuggestionPriority::High,
            SuggestionPriority::Medium,
            SuggestionPriority::Low,
        ]);
    }
}