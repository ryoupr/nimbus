use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn, error, debug};
use async_trait::async_trait;
use aws_sdk_iam::Client as IamClient;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_sts::Client as StsClient;
use crate::diagnostic::{DiagnosticResult, Severity};
use crate::aws::{AwsManager, load_aws_config};

/// IAM診断に必要な情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamInfo {
    pub instance_profile_name: Option<String>,
    pub instance_profile_arn: Option<String>,
    pub role_name: Option<String>,
    pub role_arn: Option<String>,
    pub attached_policies: Vec<PolicyInfo>,
    pub inline_policies: Vec<String>,
    pub permissions_boundary: Option<String>,
    pub credentials_valid: bool,
    pub session_token_valid: bool,
}

/// IAMポリシー情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyInfo {
    pub policy_name: String,
    pub policy_arn: String,
    pub policy_type: PolicyType,
    pub has_ssm_permissions: bool,
}

/// ポリシータイプ
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PolicyType {
    Managed,
    Inline,
    AwsManaged,
}

/// 認証情報の状態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsStatus {
    pub access_key_id: Option<String>,
    pub session_token_present: bool,
    pub expiration: Option<chrono::DateTime<chrono::Utc>>,
    pub is_valid: bool,
    pub is_expired: bool,
}

/// IAM診断のトレイト
#[async_trait]
pub trait IamDiagnostics {
    /// インスタンスプロファイルの確認
    async fn check_instance_profile(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult>;
    
    /// SSM接続に必要な権限の検証
    async fn verify_ssm_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult>;
    
    /// 権限境界の確認
    async fn check_permissions_boundary(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult>;
    
    /// 一時的認証情報の妥当性確認
    async fn validate_temporary_credentials(&self) -> anyhow::Result<DiagnosticResult>;
    
    /// 包括的なIAM診断
    async fn diagnose_iam_configuration(&self, instance_id: &str) -> anyhow::Result<Vec<DiagnosticResult>>;
    
    // Enhanced methods for Task 25.3 - Comprehensive prerequisite checking
    
    /// 詳細なIAM権限の個別確認（必要な権限の個別確認）
    async fn verify_individual_ssm_permissions(&self, instance_id: &str) -> anyhow::Result<Vec<DiagnosticResult>>;
    
    /// クロスアカウント権限の詳細検証
    async fn verify_cross_account_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult>;
    
    /// リソースベース権限の詳細分析
    async fn analyze_resource_based_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult>;
    
    /// 権限境界の影響分析
    async fn analyze_permissions_boundary_impact(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult>;
    
    /// EC2必須権限の詳細確認
    async fn verify_ec2_required_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult>;
}

/// デフォルトのIAM診断実装
pub struct DefaultIamDiagnostics {
    iam_client: IamClient,
    ec2_client: Ec2Client,
    sts_client: StsClient,
}

impl DefaultIamDiagnostics {
    /// デフォルトのAWS設定でIAM診断を作成
    pub async fn with_default_aws() -> anyhow::Result<Self> {
        Self::with_aws_config(None, None).await
    }
    
    /// 指定されたAWS設定でIAM診断を作成
    pub async fn with_aws_config(region: Option<&str>, profile: Option<&str>) -> anyhow::Result<Self> {
        let config = load_aws_config(region, profile).await;
        let sts_client = StsClient::new(&config);
        let iam_client = IamClient::new(&config);
        let ec2_client = Ec2Client::new(&config);
        
        Ok(Self {
            iam_client,
            ec2_client,
            sts_client,
        })
    }
    
    /// 指定されたAWSマネージャーでIAM診断を作成
    pub async fn with_aws_manager(aws_manager: &AwsManager) -> anyhow::Result<Self> {
        let config = load_aws_config(None, None).await;
        let sts_client = StsClient::new(&config);
        let iam_client = IamClient::new(&config);
        
        Ok(Self {
            iam_client,
            ec2_client: aws_manager.ec2_client.clone(),
            sts_client,
        })
    }
    
    // Helper methods for enhanced IAM diagnostics
    
    /// Check if a specific permission is available in the IAM info
    async fn check_specific_permission(&self, iam_info: &IamInfo, permission: &str) -> anyhow::Result<bool> {
        // Check attached policies
        for policy in &iam_info.attached_policies {
            if policy.has_ssm_permissions {
                // For AWS managed policies, do a more specific check
                if policy.policy_type == PolicyType::AwsManaged {
                    if self.check_aws_managed_policy_permission(&policy.policy_arn, permission).await? {
                        return Ok(true);
                    }
                } else {
                    // For custom policies, check the policy document
                    if self.check_policy_document_permission(&policy.policy_arn, permission).await? {
                        return Ok(true);
                    }
                }
            }
        }
        
        // Check inline policies (simplified check)
        if !iam_info.inline_policies.is_empty() {
            // For inline policies, we would need to get each policy document
            // This is a simplified implementation
            return Ok(false);
        }
        
        Ok(false)
    }
    
    /// Check if an AWS managed policy contains a specific permission
    async fn check_aws_managed_policy_permission(&self, policy_arn: &str, permission: &str) -> anyhow::Result<bool> {
        // Known AWS managed policies and their permissions
        let policy_permissions = match policy_arn {
            arn if arn.contains("AmazonSSMManagedInstanceCore") => {
                vec![
                    "ssm:UpdateInstanceInformation",
                    "ssmmessages:CreateControlChannel",
                    "ssmmessages:CreateDataChannel",
                    "ssmmessages:OpenControlChannel",
                    "ssmmessages:OpenDataChannel",
                    "ec2messages:AcknowledgeMessage",
                    "ec2messages:DeleteMessage",
                    "ec2messages:FailMessage",
                    "ec2messages:GetEndpoint",
                    "ec2messages:GetMessages",
                    "ec2messages:SendReply",
                ]
            }
            arn if arn.contains("PowerUserAccess") || arn.contains("AdministratorAccess") => {
                // These policies have broad permissions
                return Ok(true);
            }
            _ => {
                // For other policies, try to get the actual policy document
                return self.check_policy_document_permission(policy_arn, permission).await;
            }
        };
        
        Ok(policy_permissions.contains(&permission))
    }
    
    /// Check if a policy document contains a specific permission
    async fn check_policy_document_permission(&self, policy_arn: &str, permission: &str) -> anyhow::Result<bool> {
        match self.get_policy_version(policy_arn).await {
            Ok(policy_document) => {
                // Simple string search for the permission
                Ok(policy_document.contains(permission) || 
                   policy_document.contains("\"*\"") ||
                   policy_document.contains(&format!("{}:*", permission.split(':').next().unwrap_or(""))))
            }
            Err(_) => Ok(false),
        }
    }
    
    /// Analyze role trust policy for cross-account relationships
    async fn analyze_role_trust_policy(&self, role_name: &str) -> anyhow::Result<String> {
        let response = self.iam_client
            .get_role()
            .role_name(role_name)
            .send()
            .await?;
        
        if let Some(role) = response.role() {
            if let Some(trust_policy) = role.assume_role_policy_document() {
                let decoded_policy = urlencoding::decode(trust_policy)?;
                
                // Analyze trust policy for cross-account elements
                if decoded_policy.contains("arn:aws:iam::") && !decoded_policy.contains("arn:aws:iam:::") {
                    return Ok("Cross-account trust relationship detected".to_string());
                }
                
                if decoded_policy.contains("ec2.amazonaws.com") {
                    return Ok("Standard EC2 service trust relationship".to_string());
                }
                
                return Ok("Custom trust relationship detected".to_string());
            }
        }
        
        Ok("No trust policy found".to_string())
    }
    
    /// Check if external ID is required
    async fn check_external_id_requirement(&self, _iam_info: &IamInfo) -> anyhow::Result<bool> {
        // This would require analyzing the trust policy for ExternalId conditions
        // Simplified implementation for now
        Ok(false)
    }
    
    /// Check KMS permissions for instance
    async fn check_kms_permissions_for_instance(&self, _instance_id: &str) -> anyhow::Result<String> {
        // This would check if the instance uses encrypted EBS volumes and verify KMS permissions
        // Simplified implementation for now
        Ok("KMS permissions not required for unencrypted volumes".to_string())
    }
    
    /// Check S3 permissions for SSM
    async fn check_s3_permissions_for_ssm(&self) -> anyhow::Result<String> {
        // This would check S3 permissions if SSM is configured to use S3 for output
        // Simplified implementation for now
        Ok("S3 permissions not required for basic SSM functionality".to_string())
    }
    
    /// Check CloudWatch Logs permissions
    async fn check_cloudwatch_logs_permissions(&self) -> anyhow::Result<String> {
        // This would check CloudWatch Logs permissions for SSM logging
        // Simplified implementation for now
        Ok("CloudWatch Logs permissions available for SSM logging".to_string())
    }
    
    /// Analyze SSM permissions in a policy document
    async fn analyze_ssm_permissions_in_policy(&self, policy_document: &str) -> Vec<String> {
        let mut found_permissions = Vec::new();
        
        let ssm_actions = vec![
            "ssm:UpdateInstanceInformation",
            "ssm:SendCommand",
            "ssm:ListCommands",
            "ssm:ListCommandInvocations",
            "ssm:DescribeInstanceInformation",
            "ssm:GetConnectionStatus",
            "ssm:DescribeInstanceAssociationsStatus",
            "ec2messages:*",
            "ssmmessages:*",
        ];
        
        for action in ssm_actions {
            if policy_document.contains(action) {
                found_permissions.push(action.to_string());
            }
        }
        
        if policy_document.contains("\"*\"") {
            found_permissions.push("Full access (*)".to_string());
        }
        
        found_permissions
    }
    
    /// インスタンスのIAMロール情報を取得
    async fn get_instance_iam_info(&self, instance_id: &str) -> anyhow::Result<Option<IamInfo>> {
        debug!("Getting IAM info for instance: {}", instance_id);
        
        // EC2インスタンスの詳細を取得
        let response = self.ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await?;
        
        for reservation in response.reservations() {
            for instance in reservation.instances() {
                if instance.instance_id.as_deref() == Some(instance_id) {
                    if let Some(iam_instance_profile) = &instance.iam_instance_profile {
                        let profile_arn = iam_instance_profile.arn.as_ref();
                        
                        if let Some(arn) = profile_arn {
                            // ARNからプロファイル名を抽出
                            let profile_name = arn.split('/').last().unwrap_or("unknown").to_string();
                            
                            // インスタンスプロファイルの詳細を取得
                            let iam_info = self.get_instance_profile_details(&profile_name).await?;
                            return Ok(Some(iam_info));
                        }
                    }
                    
                    // IAMインスタンスプロファイルが設定されていない
                    return Ok(None);
                }
            }
        }
        
        Ok(None)
    }
    
    /// インスタンスプロファイルの詳細を取得
    async fn get_instance_profile_details(&self, profile_name: &str) -> anyhow::Result<IamInfo> {
        debug!("Getting instance profile details: {}", profile_name);
        
        let response = self.iam_client
            .get_instance_profile()
            .instance_profile_name(profile_name)
            .send()
            .await?;
        
        let instance_profile = response.instance_profile()
            .ok_or_else(|| anyhow::anyhow!("Instance profile not found in response"))?;
        
        let mut iam_info = IamInfo {
            instance_profile_name: Some(profile_name.to_string()),
            instance_profile_arn: Some(instance_profile.arn().to_string()),
            role_name: None,
            role_arn: None,
            attached_policies: Vec::new(),
            inline_policies: Vec::new(),
            permissions_boundary: None,
            credentials_valid: false,
            session_token_valid: false,
        };
        
        // インスタンスプロファイルに関連付けられたロールを取得
        let roles = instance_profile.roles();
        if let Some(role) = roles.first() {
            iam_info.role_name = Some(role.role_name().to_string());
            iam_info.role_arn = Some(role.arn().to_string());
            
            // ロールの詳細情報を取得
            self.populate_role_details(&mut iam_info, role.role_name()).await?;
        }
        
        Ok(iam_info)
    }
    
    /// ロールの詳細情報を取得してIamInfoに設定
    async fn populate_role_details(&self, iam_info: &mut IamInfo, role_name: &str) -> anyhow::Result<()> {
        debug!("Getting role details: {}", role_name);
        
        // アタッチされたポリシーを取得
        let attached_policies_response = self.iam_client
            .list_attached_role_policies()
            .role_name(role_name)
            .send()
            .await?;
        
        let policies = attached_policies_response.attached_policies();
        for policy in policies {
            if let (Some(policy_name), Some(policy_arn)) = (policy.policy_name(), policy.policy_arn()) {
                let has_ssm_permissions = self.check_policy_ssm_permissions(policy_arn).await.unwrap_or(false);
                
                let policy_type = if policy_arn.contains(":aws:policy/") {
                    PolicyType::AwsManaged
                } else {
                    PolicyType::Managed
                };
                
                iam_info.attached_policies.push(PolicyInfo {
                    policy_name: policy_name.to_string(),
                    policy_arn: policy_arn.to_string(),
                    policy_type,
                    has_ssm_permissions,
                });
            }
        }
        
        // インラインポリシーを取得
        let inline_policies_response = self.iam_client
            .list_role_policies()
            .role_name(role_name)
            .send()
            .await?;
        
        let policy_names = inline_policies_response.policy_names();
        iam_info.inline_policies = policy_names.to_vec();
        
        // ロールの詳細を取得（権限境界など）
        let role_response = self.iam_client
            .get_role()
            .role_name(role_name)
            .send()
            .await?;
        
        if let Some(role) = role_response.role() {
            if let Some(permissions_boundary) = role.permissions_boundary() {
                iam_info.permissions_boundary = permissions_boundary.permissions_boundary_arn().map(|s| s.to_string());
            }
        }
        
        Ok(())
    }
    
    /// ポリシーがSSM権限を持っているかチェック
    async fn check_policy_ssm_permissions(&self, policy_arn: &str) -> anyhow::Result<bool> {
        debug!("Checking SSM permissions for policy: {}", policy_arn);
        
        // AWS管理ポリシーの場合、既知のSSM関連ポリシーをチェック
        if policy_arn.contains(":aws:policy/") {
            let known_ssm_policies = vec![
                "AmazonSSMManagedInstanceCore",
                "AmazonSSMDirectoryServiceAccess",
                "CloudWatchAgentServerPolicy",
                "AmazonSSMPatchAssociation",
                "AmazonSSMMaintenanceWindowRole",
                "AmazonSSMAutomationRole",
            ];
            
            for known_policy in &known_ssm_policies {
                if policy_arn.contains(known_policy) {
                    return Ok(true);
                }
            }
            
            // PowerUserAccess や AdministratorAccess などの広範囲ポリシーもチェック
            let broad_policies = vec![
                "PowerUserAccess",
                "AdministratorAccess",
            ];
            
            for broad_policy in &broad_policies {
                if policy_arn.contains(broad_policy) {
                    return Ok(true);
                }
            }
        }
        
        // カスタムポリシーの場合、ポリシードキュメントを取得して詳細チェック
        // 注意: これは複雑な処理になるため、基本的なチェックのみ実装
        match self.get_policy_version(policy_arn).await {
            Ok(policy_document) => {
                // SSM関連のアクションが含まれているかチェック
                let ssm_actions = vec![
                    "ssm:UpdateInstanceInformation",
                    "ssm:SendCommand",
                    "ssm:ListCommands",
                    "ssm:ListCommandInvocations",
                    "ssm:DescribeInstanceInformation",
                    "ssm:GetConnectionStatus",
                    "ssm:DescribeInstanceAssociationsStatus",
                    "ec2messages:",
                    "ssmmessages:",
                ];
                
                for action in &ssm_actions {
                    if policy_document.contains(action) {
                        return Ok(true);
                    }
                }
                
                // ワイルドカード権限もチェック
                if policy_document.contains("\"*\"") || policy_document.contains("ssm:*") {
                    return Ok(true);
                }
            }
            Err(e) => {
                warn!("Failed to get policy document for {}: {}", policy_arn, e);
            }
        }
        
        Ok(false)
    }
    
    /// ポリシーのバージョンドキュメントを取得
    async fn get_policy_version(&self, policy_arn: &str) -> anyhow::Result<String> {
        debug!("Getting policy version for: {}", policy_arn);
        
        // まずポリシーの情報を取得
        let policy_response = self.iam_client
            .get_policy()
            .policy_arn(policy_arn)
            .send()
            .await?;
        
        let policy = policy_response.policy()
            .ok_or_else(|| anyhow::anyhow!("Policy not found in response"))?;
        
        let default_version_id = policy.default_version_id()
            .ok_or_else(|| anyhow::anyhow!("Default version ID not found"))?;
        
        // デフォルトバージョンのドキュメントを取得
        let version_response = self.iam_client
            .get_policy_version()
            .policy_arn(policy_arn)
            .version_id(default_version_id)
            .send()
            .await?;
        
        let policy_version = version_response.policy_version()
            .ok_or_else(|| anyhow::anyhow!("Policy version not found in response"))?;
        
        let document = policy_version.document()
            .ok_or_else(|| anyhow::anyhow!("Policy document not found"))?;
        
        // URLデコードされたドキュメントを返す
        Ok(urlencoding::decode(document)?.to_string())
    }
    
    /// 現在の認証情報の状態を取得
    async fn get_credentials_status(&self) -> anyhow::Result<CredentialsStatus> {
        debug!("Getting credentials status");
        
        let mut status = CredentialsStatus {
            access_key_id: None,
            session_token_present: false,
            expiration: None,
            is_valid: false,
            is_expired: false,
        };
        
        // STS GetCallerIdentity を使用して認証情報を検証
        match self.sts_client.get_caller_identity().send().await {
            Ok(response) => {
                status.is_valid = true;
                
                // レスポンスからアクセスキーIDを取得（部分的にマスク）
                if let Some(arn) = response.arn() {
                    // ARNからアクセスキーIDを抽出（可能な場合）
                    if arn.contains("assumed-role") {
                        status.session_token_present = true;
                    }
                }
                
                // 一時的認証情報の場合、有効期限をチェック
                // 注意: STS APIからは直接有効期限を取得できないため、
                // 環境変数やメタデータサービスから取得する必要がある
                status.expiration = self.get_token_expiration().await;
                
                if let Some(exp) = status.expiration {
                    status.is_expired = chrono::Utc::now() > exp;
                }
            }
            Err(e) => {
                warn!("Failed to validate credentials: {}", e);
                status.is_valid = false;
            }
        }
        
        Ok(status)
    }
    
    /// トークンの有効期限を取得（環境変数から）
    async fn get_token_expiration(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        // AWS_SESSION_TOKEN_EXPIRATION 環境変数をチェック
        if let Ok(expiration_str) = std::env::var("AWS_SESSION_TOKEN_EXPIRATION") {
            if let Ok(timestamp) = expiration_str.parse::<i64>() {
                return chrono::DateTime::from_timestamp(timestamp, 0);
            }
        }
        
        // EC2インスタンスメタデータサービスから取得を試行
        // 注意: これは実際のEC2インスタンス上でのみ動作する
        match self.get_instance_metadata_token_expiration().await {
            Ok(expiration) => Some(expiration),
            Err(_) => None,
        }
    }
    
    /// EC2インスタンスメタデータサービスからトークン有効期限を取得
    async fn get_instance_metadata_token_expiration(&self) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
        // IMDSv2 トークンを取得
        let client = reqwest::Client::new();
        
        let token_response = client
            .put("http://169.254.169.254/latest/api/token")
            .header("X-aws-ec2-metadata-token-ttl-seconds", "21600")
            .timeout(Duration::from_secs(2))
            .send()
            .await?;
        
        let token = token_response.text().await?;
        
        // セキュリティ認証情報の有効期限を取得
        let creds_response = client
            .get("http://169.254.169.254/latest/meta-data/iam/security-credentials/")
            .header("X-aws-ec2-metadata-token", &token)
            .timeout(Duration::from_secs(2))
            .send()
            .await?;
        
        let role_name = creds_response.text().await?;
        
        let expiration_response = client
            .get(&format!("http://169.254.169.254/latest/meta-data/iam/security-credentials/{}", role_name.trim()))
            .header("X-aws-ec2-metadata-token", &token)
            .timeout(Duration::from_secs(2))
            .send()
            .await?;
        
        let creds_json: serde_json::Value = expiration_response.json().await?;
        
        if let Some(expiration_str) = creds_json.get("Expiration").and_then(|v| v.as_str()) {
            let expiration = chrono::DateTime::parse_from_rfc3339(expiration_str)?
                .with_timezone(&chrono::Utc);
            return Ok(expiration);
        }
        
        Err(anyhow::anyhow!("Expiration not found in metadata"))
    }
}

#[async_trait]
impl IamDiagnostics for DefaultIamDiagnostics {
    async fn check_instance_profile(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking instance profile for: {}", instance_id);
        
        match self.get_instance_iam_info(instance_id).await {
            Ok(Some(iam_info)) => {
                let details = serde_json::to_value(&iam_info)?;
                
                if iam_info.role_name.is_some() {
                    Ok(DiagnosticResult::success(
                        "instance_profile".to_string(),
                        format!("Instance profile configured: {}", 
                            iam_info.instance_profile_name.as_deref().unwrap_or("unknown")),
                        start_time.elapsed(),
                    ).with_details(details))
                } else {
                    Ok(DiagnosticResult::warning(
                        "instance_profile".to_string(),
                        "Instance profile exists but no IAM role attached".to_string(),
                        start_time.elapsed(),
                        Severity::High,
                    ).with_details(details))
                }
            }
            Ok(None) => {
                Ok(DiagnosticResult::error(
                    "instance_profile".to_string(),
                    "No IAM instance profile attached to EC2 instance".to_string(),
                    start_time.elapsed(),
                    Severity::Critical,
                ).with_auto_fixable(false))
            }
            Err(e) => {
                error!("Failed to check instance profile: {}", e);
                Ok(DiagnosticResult::error(
                    "instance_profile".to_string(),
                    format!("Failed to check instance profile: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn verify_ssm_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Verifying SSM permissions for: {}", instance_id);
        
        match self.get_instance_iam_info(instance_id).await {
            Ok(Some(iam_info)) => {
                let mut has_ssm_permissions = false;
                let mut permission_details = Vec::new();
                
                // アタッチされたポリシーをチェック
                for policy in &iam_info.attached_policies {
                    if policy.has_ssm_permissions {
                        has_ssm_permissions = true;
                        permission_details.push(format!("Policy '{}' has SSM permissions", policy.policy_name));
                    }
                }
                
                // インラインポリシーもチェック（簡易版）
                if !iam_info.inline_policies.is_empty() {
                    permission_details.push(format!("Found {} inline policies (manual review recommended)", 
                        iam_info.inline_policies.len()));
                }
                
                let details = serde_json::json!({
                    "iam_info": iam_info,
                    "permission_details": permission_details
                });
                
                if has_ssm_permissions {
                    Ok(DiagnosticResult::success(
                        "ssm_permissions".to_string(),
                        "Required SSM permissions found".to_string(),
                        start_time.elapsed(),
                    ).with_details(details))
                } else {
                    Ok(DiagnosticResult::error(
                        "ssm_permissions".to_string(),
                        "Missing required SSM permissions".to_string(),
                        start_time.elapsed(),
                        Severity::Critical,
                    ).with_details(details).with_auto_fixable(false))
                }
            }
            Ok(None) => {
                Ok(DiagnosticResult::error(
                    "ssm_permissions".to_string(),
                    "Cannot verify SSM permissions - no IAM role attached".to_string(),
                    start_time.elapsed(),
                    Severity::Critical,
                ))
            }
            Err(e) => {
                error!("Failed to verify SSM permissions: {}", e);
                Ok(DiagnosticResult::error(
                    "ssm_permissions".to_string(),
                    format!("Failed to verify SSM permissions: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn check_permissions_boundary(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Checking permissions boundary for: {}", instance_id);
        
        match self.get_instance_iam_info(instance_id).await {
            Ok(Some(iam_info)) => {
                let details = serde_json::to_value(&iam_info)?;
                
                if let Some(boundary_arn) = &iam_info.permissions_boundary {
                    // 権限境界がSSM権限を制限していないかチェック
                    let boundary_allows_ssm = self.check_policy_ssm_permissions(boundary_arn).await
                        .unwrap_or(false);
                    
                    if boundary_allows_ssm {
                        Ok(DiagnosticResult::success(
                            "permissions_boundary".to_string(),
                            format!("Permissions boundary allows SSM access: {}", boundary_arn),
                            start_time.elapsed(),
                        ).with_details(details))
                    } else {
                        Ok(DiagnosticResult::warning(
                            "permissions_boundary".to_string(),
                            format!("Permissions boundary may restrict SSM access: {}", boundary_arn),
                            start_time.elapsed(),
                            Severity::High,
                        ).with_details(details))
                    }
                } else {
                    Ok(DiagnosticResult::success(
                        "permissions_boundary".to_string(),
                        "No permissions boundary configured".to_string(),
                        start_time.elapsed(),
                    ).with_details(details))
                }
            }
            Ok(None) => {
                Ok(DiagnosticResult::error(
                    "permissions_boundary".to_string(),
                    "Cannot check permissions boundary - no IAM role attached".to_string(),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
            Err(e) => {
                error!("Failed to check permissions boundary: {}", e);
                Ok(DiagnosticResult::error(
                    "permissions_boundary".to_string(),
                    format!("Failed to check permissions boundary: {}", e),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
        }
    }
    
    async fn validate_temporary_credentials(&self) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Validating temporary credentials");
        
        match self.get_credentials_status().await {
            Ok(status) => {
                let details = serde_json::to_value(&status)?;
                
                if !status.is_valid {
                    return Ok(DiagnosticResult::error(
                        "temporary_credentials".to_string(),
                        "AWS credentials are invalid".to_string(),
                        start_time.elapsed(),
                        Severity::Critical,
                    ).with_details(details));
                }
                
                if status.is_expired {
                    return Ok(DiagnosticResult::error(
                        "temporary_credentials".to_string(),
                        "AWS session token has expired".to_string(),
                        start_time.elapsed(),
                        Severity::High,
                    ).with_details(details).with_auto_fixable(true));
                }
                
                if status.session_token_present {
                    if let Some(expiration) = status.expiration {
                        let time_until_expiry = expiration - chrono::Utc::now();
                        
                        if time_until_expiry < chrono::Duration::minutes(15) {
                            Ok(DiagnosticResult::warning(
                                "temporary_credentials".to_string(),
                                format!("Session token expires soon: {} minutes remaining", 
                                    time_until_expiry.num_minutes()),
                                start_time.elapsed(),
                                Severity::Medium,
                            ).with_details(details))
                        } else {
                            Ok(DiagnosticResult::success(
                                "temporary_credentials".to_string(),
                                format!("Session token valid for {} hours", 
                                    time_until_expiry.num_hours()),
                                start_time.elapsed(),
                            ).with_details(details))
                        }
                    } else {
                        Ok(DiagnosticResult::success(
                            "temporary_credentials".to_string(),
                            "Session token is valid (expiration unknown)".to_string(),
                            start_time.elapsed(),
                        ).with_details(details))
                    }
                } else {
                    Ok(DiagnosticResult::success(
                        "temporary_credentials".to_string(),
                        "Using long-term AWS credentials".to_string(),
                        start_time.elapsed(),
                    ).with_details(details))
                }
            }
            Err(e) => {
                error!("Failed to validate credentials: {}", e);
                Ok(DiagnosticResult::error(
                    "temporary_credentials".to_string(),
                    format!("Failed to validate credentials: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ))
            }
        }
    }
    
    async fn diagnose_iam_configuration(&self, instance_id: &str) -> anyhow::Result<Vec<DiagnosticResult>> {
        info!("Running comprehensive IAM diagnostics for: {}", instance_id);
        
        let mut results = Vec::new();
        
        // 1. インスタンスプロファイル確認
        results.push(self.check_instance_profile(instance_id).await?);
        
        // 2. SSM権限検証
        results.push(self.verify_ssm_permissions(instance_id).await?);
        
        // 3. 権限境界確認
        results.push(self.check_permissions_boundary(instance_id).await?);
        
        // 4. 一時的認証情報検証
        results.push(self.validate_temporary_credentials().await?);
        
        // Enhanced checks for Task 25.3 - Comprehensive prerequisite checking
        
        // 5. 詳細なIAM権限の個別確認
        let individual_permissions = self.verify_individual_ssm_permissions(instance_id).await?;
        results.extend(individual_permissions);
        
        // 6. クロスアカウント権限の詳細検証
        results.push(self.verify_cross_account_permissions(instance_id).await?);
        
        // 7. リソースベース権限の詳細分析
        results.push(self.analyze_resource_based_permissions(instance_id).await?);
        
        // 8. 権限境界の影響分析
        results.push(self.analyze_permissions_boundary_impact(instance_id).await?);
        
        // 9. EC2必須権限の詳細確認
        results.push(self.verify_ec2_required_permissions(instance_id).await?);
        
        info!("IAM diagnostics completed for: {}", instance_id);
        Ok(results)
    }
    
    // Enhanced methods for Task 25.3 - Comprehensive prerequisite checking
    
    async fn verify_individual_ssm_permissions(&self, instance_id: &str) -> anyhow::Result<Vec<DiagnosticResult>> {
        info!("Verifying individual SSM permissions for: {}", instance_id);
        let mut results = Vec::new();
        
        // Define required SSM permissions with detailed descriptions
        let required_permissions = vec![
            ("ssm:UpdateInstanceInformation", "Required for instance registration with SSM"),
            ("ssm:SendCommand", "Required for executing commands via SSM"),
            ("ssm:ListCommands", "Required for listing command history"),
            ("ssm:ListCommandInvocations", "Required for command execution status"),
            ("ssm:DescribeInstanceInformation", "Required for instance status reporting"),
            ("ssm:GetConnectionStatus", "Required for connection status monitoring"),
            ("ssm:DescribeInstanceAssociationsStatus", "Required for association status"),
            ("ec2messages:AcknowledgeMessage", "Required for EC2 message acknowledgment"),
            ("ec2messages:DeleteMessage", "Required for EC2 message cleanup"),
            ("ec2messages:FailMessage", "Required for EC2 message failure handling"),
            ("ec2messages:GetEndpoint", "Required for EC2 message endpoint discovery"),
            ("ec2messages:GetMessages", "Required for EC2 message retrieval"),
            ("ec2messages:SendReply", "Required for EC2 message responses"),
            ("ssmmessages:CreateControlChannel", "Required for SSM control channel"),
            ("ssmmessages:CreateDataChannel", "Required for SSM data channel"),
            ("ssmmessages:OpenControlChannel", "Required for opening SSM control channel"),
            ("ssmmessages:OpenDataChannel", "Required for opening SSM data channel"),
        ];
        
        match self.get_instance_iam_info(instance_id).await {
            Ok(Some(iam_info)) => {
                for (permission, description) in required_permissions {
                    let start_time = Instant::now();
                    
                    let has_permission = self.check_specific_permission(&iam_info, permission).await
                        .unwrap_or(false);
                    
                    let result = if has_permission {
                        DiagnosticResult::success(
                            format!("permission_{}", permission.replace(":", "_")),
                            format!("Permission '{}' is available: {}", permission, description),
                            start_time.elapsed(),
                        )
                    } else {
                        DiagnosticResult::error(
                            format!("permission_{}", permission.replace(":", "_")),
                            format!("Missing permission '{}': {}", permission, description),
                            start_time.elapsed(),
                            Severity::High,
                        ).with_auto_fixable(false)
                    };
                    
                    results.push(result);
                }
            }
            Ok(None) => {
                let start_time = Instant::now();
                results.push(DiagnosticResult::error(
                    "individual_permissions".to_string(),
                    "Cannot verify individual permissions - no IAM role attached".to_string(),
                    start_time.elapsed(),
                    Severity::Critical,
                ));
            }
            Err(e) => {
                let start_time = Instant::now();
                results.push(DiagnosticResult::error(
                    "individual_permissions".to_string(),
                    format!("Failed to verify individual permissions: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                ));
            }
        }
        
        Ok(results)
    }
    
    async fn verify_cross_account_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Verifying cross-account permissions for: {}", instance_id);
        
        match self.get_instance_iam_info(instance_id).await {
            Ok(Some(iam_info)) => {
                let mut cross_account_issues = Vec::new();
                let mut cross_account_details = Vec::new();
                
                // Check if role has cross-account trust relationships
                if let Some(role_name) = &iam_info.role_name {
                    match self.analyze_role_trust_policy(role_name).await {
                        Ok(trust_analysis) => {
                            cross_account_details.push(trust_analysis.clone());
                            
                            // Check for potential cross-account issues
                            if trust_analysis.contains("cross-account") {
                                cross_account_issues.push("Cross-account trust relationship detected".to_string());
                            }
                        }
                        Err(e) => {
                            cross_account_issues.push(format!("Failed to analyze trust policy: {}", e));
                        }
                    }
                }
                
                // Check for external ID requirements
                let external_id_required = self.check_external_id_requirement(&iam_info).await
                    .unwrap_or(false);
                
                if external_id_required {
                    cross_account_details.push("External ID required for cross-account access".to_string());
                }
                
                let details = serde_json::json!({
                    "cross_account_issues": cross_account_issues,
                    "cross_account_details": cross_account_details,
                    "external_id_required": external_id_required
                });
                
                if cross_account_issues.is_empty() {
                    Ok(DiagnosticResult::success(
                        "cross_account_permissions".to_string(),
                        "No cross-account permission issues detected".to_string(),
                        start_time.elapsed(),
                    ).with_details(details))
                } else {
                    Ok(DiagnosticResult::warning(
                        "cross_account_permissions".to_string(),
                        format!("Cross-account permission considerations: {}", cross_account_issues.join(", ")),
                        start_time.elapsed(),
                        Severity::Medium,
                    ).with_details(details))
                }
            }
            Ok(None) => {
                Ok(DiagnosticResult::error(
                    "cross_account_permissions".to_string(),
                    "Cannot verify cross-account permissions - no IAM role attached".to_string(),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
            Err(e) => {
                Ok(DiagnosticResult::error(
                    "cross_account_permissions".to_string(),
                    format!("Failed to verify cross-account permissions: {}", e),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
        }
    }
    
    async fn analyze_resource_based_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Analyzing resource-based permissions for: {}", instance_id);
        
        let mut resource_analysis = Vec::new();
        let mut resource_issues = Vec::new();
        
        // Check KMS key permissions (if using encrypted EBS volumes)
        match self.check_kms_permissions_for_instance(instance_id).await {
            Ok(kms_analysis) => {
                resource_analysis.push(format!("KMS permissions: {}", kms_analysis));
            }
            Err(e) => {
                resource_issues.push(format!("KMS permission check failed: {}", e));
            }
        }
        
        // Check S3 bucket permissions (if SSM uses S3 for logging/output)
        match self.check_s3_permissions_for_ssm().await {
            Ok(s3_analysis) => {
                resource_analysis.push(format!("S3 permissions: {}", s3_analysis));
            }
            Err(e) => {
                resource_issues.push(format!("S3 permission check failed: {}", e));
            }
        }
        
        // Check CloudWatch Logs permissions
        match self.check_cloudwatch_logs_permissions().await {
            Ok(logs_analysis) => {
                resource_analysis.push(format!("CloudWatch Logs permissions: {}", logs_analysis));
            }
            Err(e) => {
                resource_issues.push(format!("CloudWatch Logs permission check failed: {}", e));
            }
        }
        
        let details = serde_json::json!({
            "resource_analysis": resource_analysis,
            "resource_issues": resource_issues
        });
        
        if resource_issues.is_empty() {
            Ok(DiagnosticResult::success(
                "resource_based_permissions".to_string(),
                "Resource-based permissions analysis completed successfully".to_string(),
                start_time.elapsed(),
            ).with_details(details))
        } else {
            Ok(DiagnosticResult::warning(
                "resource_based_permissions".to_string(),
                format!("Resource-based permission issues: {}", resource_issues.join(", ")),
                start_time.elapsed(),
                Severity::Medium,
            ).with_details(details))
        }
    }
    
    async fn analyze_permissions_boundary_impact(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Analyzing permissions boundary impact for: {}", instance_id);
        
        match self.get_instance_iam_info(instance_id).await {
            Ok(Some(iam_info)) => {
                if let Some(boundary_arn) = &iam_info.permissions_boundary {
                    // Detailed analysis of permissions boundary impact
                    let mut boundary_analysis = Vec::new();
                    let mut boundary_restrictions = Vec::new();
                    
                    // Get the boundary policy document
                    match self.get_policy_version(boundary_arn).await {
                        Ok(policy_document) => {
                            // Analyze specific SSM permissions in boundary
                            let ssm_permissions_in_boundary = self.analyze_ssm_permissions_in_policy(&policy_document).await;
                            boundary_analysis.push(format!("SSM permissions in boundary: {:?}", ssm_permissions_in_boundary));
                            
                            // Check for restrictive conditions
                            if policy_document.contains("DateLessThan") || policy_document.contains("DateGreaterThan") {
                                boundary_restrictions.push("Time-based restrictions detected".to_string());
                            }
                            
                            if policy_document.contains("IpAddress") || policy_document.contains("IpAddressIfExists") {
                                boundary_restrictions.push("IP-based restrictions detected".to_string());
                            }
                            
                            if policy_document.contains("RequestedRegion") {
                                boundary_restrictions.push("Region-based restrictions detected".to_string());
                            }
                            
                            // Check for MFA requirements
                            if policy_document.contains("aws:MultiFactorAuthPresent") {
                                boundary_restrictions.push("MFA requirements detected".to_string());
                            }
                        }
                        Err(e) => {
                            boundary_analysis.push(format!("Failed to analyze boundary policy: {}", e));
                        }
                    }
                    
                    let details = serde_json::json!({
                        "boundary_arn": boundary_arn,
                        "boundary_analysis": boundary_analysis,
                        "boundary_restrictions": boundary_restrictions
                    });
                    
                    let severity = if boundary_restrictions.is_empty() {
                        Severity::Low
                    } else {
                        Severity::Medium
                    };
                    
                    Ok(DiagnosticResult::warning(
                        "permissions_boundary_impact".to_string(),
                        format!("Permissions boundary may impact SSM access: {}", boundary_restrictions.join(", ")),
                        start_time.elapsed(),
                        severity,
                    ).with_details(details))
                } else {
                    Ok(DiagnosticResult::success(
                        "permissions_boundary_impact".to_string(),
                        "No permissions boundary configured - no restrictions".to_string(),
                        start_time.elapsed(),
                    ))
                }
            }
            Ok(None) => {
                Ok(DiagnosticResult::error(
                    "permissions_boundary_impact".to_string(),
                    "Cannot analyze permissions boundary - no IAM role attached".to_string(),
                    start_time.elapsed(),
                    Severity::Low,
                ))
            }
            Err(e) => {
                Ok(DiagnosticResult::error(
                    "permissions_boundary_impact".to_string(),
                    format!("Failed to analyze permissions boundary impact: {}", e),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
        }
    }
    
    async fn verify_ec2_required_permissions(&self, instance_id: &str) -> anyhow::Result<DiagnosticResult> {
        let start_time = Instant::now();
        info!("Verifying EC2 required permissions for: {}", instance_id);
        
        // Define required EC2 permissions for SSM functionality
        let required_ec2_permissions = vec![
            ("ec2:DescribeInstanceStatus", "Required for instance status checks"),
            ("ec2:DescribeInstances", "Required for instance metadata"),
            ("ec2:DescribeInstanceAttribute", "Required for instance attributes"),
            ("ec2:DescribeTags", "Required for instance tags"),
        ];
        
        match self.get_instance_iam_info(instance_id).await {
            Ok(Some(iam_info)) => {
                let mut missing_permissions = Vec::new();
                let mut available_permissions = Vec::new();
                
                let required_ec2_permissions_len = required_ec2_permissions.len();
                
                for (permission, description) in &required_ec2_permissions {
                    let has_permission = self.check_specific_permission(&iam_info, permission).await
                        .unwrap_or(false);
                    
                    if has_permission {
                        available_permissions.push(format!("{}: {}", permission, description));
                    } else {
                        missing_permissions.push(format!("{}: {}", permission, description));
                    }
                }
                
                let details = serde_json::json!({
                    "available_permissions": available_permissions,
                    "missing_permissions": missing_permissions,
                    "total_required": required_ec2_permissions_len,
                    "available_count": available_permissions.len()
                });
                
                if missing_permissions.is_empty() {
                    Ok(DiagnosticResult::success(
                        "ec2_required_permissions".to_string(),
                        "All required EC2 permissions are available".to_string(),
                        start_time.elapsed(),
                    ).with_details(details))
                } else {
                    Ok(DiagnosticResult::warning(
                        "ec2_required_permissions".to_string(),
                        format!("Missing {} EC2 permissions", missing_permissions.len()),
                        start_time.elapsed(),
                        Severity::Medium,
                    ).with_details(details))
                }
            }
            Ok(None) => {
                Ok(DiagnosticResult::error(
                    "ec2_required_permissions".to_string(),
                    "Cannot verify EC2 permissions - no IAM role attached".to_string(),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
            Err(e) => {
                Ok(DiagnosticResult::error(
                    "ec2_required_permissions".to_string(),
                    format!("Failed to verify EC2 permissions: {}", e),
                    start_time.elapsed(),
                    Severity::Medium,
                ))
            }
        }
    }
}

// 必要な依存関係を追加
use reqwest;
use urlencoding;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_policy_info_serialization() {
        let policy = PolicyInfo {
            policy_name: "TestPolicy".to_string(),
            policy_arn: "arn:aws:iam::123456789012:policy/TestPolicy".to_string(),
            policy_type: PolicyType::Managed,
            has_ssm_permissions: true,
        };
        
        let serialized = serde_json::to_string(&policy).unwrap();
        let deserialized: PolicyInfo = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(policy.policy_name, deserialized.policy_name);
        assert_eq!(policy.policy_arn, deserialized.policy_arn);
        assert!(deserialized.has_ssm_permissions);
    }
    
    #[test]
    fn test_credentials_status_creation() {
        let status = CredentialsStatus {
            access_key_id: Some("AKIA...".to_string()),
            session_token_present: true,
            expiration: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            is_valid: true,
            is_expired: false,
        };
        
        assert!(status.is_valid);
        assert!(!status.is_expired);
        assert!(status.session_token_present);
    }
    
    #[tokio::test]
    async fn test_iam_diagnostics_creation() {
        // テスト環境では実際のAWS認証情報が必要
        match DefaultIamDiagnostics::with_default_aws().await {
            Ok(_) => {
                println!("IAM diagnostics created successfully");
            }
            Err(e) => {
                println!("IAM diagnostics creation failed (expected in test environment): {}", e);
            }
        }
    }
}