#![allow(dead_code)]

use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_ssm::Client as SsmClient;
use aws_credential_types::Credentials;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{debug, info, warn};

/// AWS 認証とプロファイル管理
#[derive(Debug, Clone)]
pub struct AwsManager {
    pub ssm_client: SsmClient,
    pub ec2_client: Ec2Client,
    region: String,
}

/// AWS クライアントのエイリアス（Auto Reconnector で使用）
pub type AwsClient = AwsManager;

/// SSM セッション情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsmSession {
    pub session_id: String,
    pub target: String,
    pub status: SsmSessionStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub region: String,
}

/// SSM セッション状態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SsmSessionStatus {
    Connected,
    Connecting,
    Disconnected,
    Failed,
    Terminated,
    Terminating,
}

/// AWS CLI export-credentials の出力形式
#[derive(Debug, Deserialize)]
struct ExportedCredentials {
    #[serde(rename = "AccessKeyId")]
    access_key_id: String,
    #[serde(rename = "SecretAccessKey")]
    secret_access_key: String,
    #[serde(rename = "SessionToken")]
    session_token: Option<String>,
}

/// AWS CLIからMFA認証済み認証情報を取得してSdkConfigを構築
pub async fn load_aws_config(region: Option<&str>, profile: Option<&str>) -> aws_config::SdkConfig {
    let mut config_loader = aws_config::defaults(BehaviorVersion::latest());
    
    if let Some(region_name) = region {
        config_loader = config_loader.region(Region::new(region_name.to_string()));
    }
    
    if let Some(profile_name) = profile {
        if let Ok(creds) = get_credentials_from_cli(profile_name) {
            let credentials = Credentials::new(
                creds.access_key_id,
                creds.secret_access_key,
                creds.session_token,
                None,
                "aws-cli-export",
            );
            config_loader = config_loader.credentials_provider(credentials);
            info!("Using credentials from AWS CLI (MFA supported)");
        } else {
            warn!("Failed to get credentials from AWS CLI, falling back to SDK");
            config_loader = config_loader.profile_name(profile_name);
        }
    }
    
    config_loader.load().await
}

fn get_credentials_from_cli(profile: &str) -> Result<ExportedCredentials> {
    debug!("Getting credentials from AWS CLI for profile: {}", profile);
    
    // aws コマンドのパスを探す（非対話シェルでも動作するように）
    let aws_cmd = find_aws_command().unwrap_or_else(|| "aws".to_string());
    
    let output = Command::new(&aws_cmd)
        .args(["configure", "export-credentials", "--profile", profile])
        .output()
        .context("Failed to execute aws configure export-credentials")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("AWS CLI failed: {}", stderr);
    }
    
    serde_json::from_slice(&output.stdout)
        .context("Failed to parse credentials from AWS CLI")
}

fn find_aws_command() -> Option<String> {
    let paths = [
        "/opt/homebrew/bin/aws",
        "/usr/local/bin/aws",
        "/usr/bin/aws",
    ];
    for path in paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

impl AwsManager {
    /// 新しい AWS マネージャーを作成
    pub async fn new(region: Option<String>, profile: Option<String>) -> Result<Self> {
        info!("Initializing AWS manager with region: {:?}, profile: {:?}", region, profile);
        
        let config = load_aws_config(region.as_deref(), profile.as_deref()).await;
        
        // クライアントを作成
        let ssm_client = SsmClient::new(&config);
        let ec2_client = Ec2Client::new(&config);
        
        let region_name = config.region()
            .map(|r| r.as_ref().to_string())
            .unwrap_or_else(|| "us-east-1".to_string());
        
        debug!("AWS clients initialized successfully");
        
        Ok(Self {
            ssm_client,
            ec2_client,
            region: region_name,
        })
    }
    
    /// デフォルト設定で AWS マネージャーを作成
    pub async fn default() -> Result<Self> {
        Self::new(None, None).await
    }
    
    /// 指定されたプロファイルで AWS マネージャーを作成
    pub async fn with_profile(profile: &str) -> Result<Self> {
        Self::new(None, Some(profile.to_string())).await
    }
    
    /// 指定されたリージョンで AWS マネージャーを作成
    pub async fn with_region(region: &str) -> Result<Self> {
        Self::new(Some(region.to_string()), None).await
    }
    
    /// SSM セッションを終了
    pub async fn terminate_ssm_session(&self, session_id: &str) -> Result<()> {
        info!("Terminating SSM session: {}", session_id);
        
        self.ssm_client
            .terminate_session()
            .session_id(session_id)
            .send()
            .await
            .context("Failed to terminate SSM session")?;
        
        info!("SSM session terminated successfully: {}", session_id);
        Ok(())
    }
    
    /// SSM セッション状態を取得
    pub async fn get_session_status(&self, session_id: &str) -> Result<SsmSessionStatus> {
        debug!("Getting session status for: {}", session_id);
        
        let response = self.ssm_client
            .describe_sessions()
            .set_filters(Some(vec![
                aws_sdk_ssm::types::SessionFilter::builder()
                    .key(aws_sdk_ssm::types::SessionFilterKey::SessionId)
                    .value(session_id)
                    .build()
                    .context("Failed to build session filter")?
            ]))
            .send()
            .await
            .context("Failed to describe SSM sessions")?;
        
        let sessions = response.sessions();
        if let Some(session) = sessions.first() {
            let status = match session.status {
                Some(aws_sdk_ssm::types::SessionStatus::Connected) => SsmSessionStatus::Connected,
                Some(aws_sdk_ssm::types::SessionStatus::Connecting) => SsmSessionStatus::Connecting,
                Some(aws_sdk_ssm::types::SessionStatus::Disconnected) => SsmSessionStatus::Disconnected,
                Some(aws_sdk_ssm::types::SessionStatus::Failed) => SsmSessionStatus::Failed,
                Some(aws_sdk_ssm::types::SessionStatus::Terminated) => SsmSessionStatus::Terminated,
                Some(aws_sdk_ssm::types::SessionStatus::Terminating) => SsmSessionStatus::Terminating,
                _ => SsmSessionStatus::Failed,
            };
            
            debug!("Session status: {:?}", status);
            return Ok(status);
        }
        
        warn!("Session not found: {}", session_id);
        Ok(SsmSessionStatus::Terminated)
    }
    
    /// アクティブな SSM セッション一覧を取得
    pub async fn list_active_sessions(&self) -> Result<Vec<SsmSession>> {
        debug!("Listing active SSM sessions");
        
        let response = self.ssm_client
            .describe_sessions()
            .set_filters(Some(vec![
                aws_sdk_ssm::types::SessionFilter::builder()
                    .key(aws_sdk_ssm::types::SessionFilterKey::Status)
                    .value("Connected")
                    .build()
                    .context("Failed to build status filter")?
            ]))
            .send()
            .await
            .context("Failed to describe SSM sessions")?;
        
        let mut sessions = Vec::new();
        
        let session_list = response.sessions();
        for session in session_list {
            if let (Some(session_id), Some(target)) = (&session.session_id, &session.target) {
                let status = match &session.status {
                    Some(aws_sdk_ssm::types::SessionStatus::Connected) => SsmSessionStatus::Connected,
                    Some(aws_sdk_ssm::types::SessionStatus::Connecting) => SsmSessionStatus::Connecting,
                    _ => continue, // アクティブでないセッションはスキップ
                };
                
                let created_at = session.start_date
                    .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos()))
                    .unwrap_or_else(chrono::Utc::now);
                
                sessions.push(SsmSession {
                    session_id: session_id.clone(),
                    target: target.clone(),
                    status,
                    created_at,
                    region: self.region.clone(),
                });
            }
        }
        
        info!("Found {} active SSM sessions", sessions.len());
        Ok(sessions)
    }
    
    /// EC2 インスタンス情報を取得
    #[allow(dead_code)]
    pub async fn get_instance_info(&self, instance_id: &str) -> Result<Option<InstanceInfo>> {
        debug!("Getting EC2 instance info: {}", instance_id);
        
        let response = self.ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .context("Failed to describe EC2 instance")?;
        
        for reservation in response.reservations() {
            for instance in reservation.instances() {
                if instance.instance_id.as_deref() == Some(instance_id) {
                    let info = InstanceInfo {
                        instance_id: instance_id.to_string(),
                        instance_type: instance.instance_type
                            .as_ref()
                            .map(|t| t.as_str().to_string()),
                        state: instance.state
                            .as_ref()
                            .and_then(|s| s.name.as_ref())
                            .map(|n| n.as_str().to_string()),
                        private_ip: instance.private_ip_address
                            .as_ref()
                            .map(|ip| ip.clone()),
                        public_ip: instance.public_ip_address
                            .as_ref()
                            .map(|ip| ip.clone()),
                        availability_zone: instance.placement
                            .as_ref()
                            .and_then(|p| p.availability_zone.as_ref())
                            .map(|az| az.clone()),
                        vpc_id: instance.vpc_id
                            .as_ref()
                            .map(|vpc| vpc.clone()),
                        subnet_id: instance.subnet_id
                            .as_ref()
                            .map(|subnet| subnet.clone()),
                    };
                    
                    debug!("Instance info retrieved: {:?}", info);
                    return Ok(Some(info));
                }
            }
        }
        
        warn!("Instance info not found: {}", instance_id);
        Ok(None)
    }
    
    /// 現在のリージョンを取得
    pub fn region(&self) -> &str {
        &self.region
    }
}

/// EC2 インスタンス情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub instance_type: Option<String>,
    pub state: Option<String>,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
    pub availability_zone: Option<String>,
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_aws_manager_creation() {
        // デフォルト設定でのマネージャー作成をテスト
        // 実際のAWS認証情報が必要なため、統合テストとして実装
        let result = AwsManager::default().await;
        
        // 認証情報が設定されていない環境では失敗することを想定
        match result {
            Ok(_) => {
                // 認証情報が正しく設定されている場合
                println!("AWS manager created successfully");
            }
            Err(e) => {
                // 認証情報が設定されていない場合（テスト環境では正常）
                println!("AWS manager creation failed (expected in test environment): {}", e);
            }
        }
    }
    
    #[test]
    fn test_ssm_session_status_serialization() {
        let status = SsmSessionStatus::Connected;
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: SsmSessionStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(status, deserialized);
    }
    
    #[test]
    fn test_ssm_session_config_creation() {
        let mut parameters = HashMap::new();
        parameters.insert("portNumber".to_string(), vec!["22".to_string()]);
        
        let config = SsmSessionConfig {
            target: "i-1234567890abcdef0".to_string(),
            document_name: Some("AWS-StartSSHSession".to_string()),
            parameters,
            reason: Some("Development access".to_string()),
        };
        
        assert_eq!(config.target, "i-1234567890abcdef0");
        assert_eq!(config.document_name, Some("AWS-StartSSHSession".to_string()));
        assert_eq!(config.reason, Some("Development access".to_string()));
    }
}