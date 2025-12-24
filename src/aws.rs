use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_ssm::Client as SsmClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};
use crate::session::{Session, SessionConfig, SessionStatus};
use std::time::SystemTime;

/// AWS 認証とプロファイル管理
#[derive(Debug, Clone)]
pub struct AwsManager {
    pub ssm_client: SsmClient,
    pub ec2_client: Ec2Client,
    region: String,
}

/// AWS クライアントのエイリアス（Auto Reconnector で使用）
pub type AwsClient = AwsManager;

/// AWS プロファイル設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsProfile {
    pub name: String,
    pub region: String,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub session_token: Option<String>,
}

/// SSM セッション設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsmSessionConfig {
    pub target: String,
    pub document_name: Option<String>,
    pub parameters: HashMap<String, Vec<String>>,
    pub reason: Option<String>,
}

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

impl AwsManager {
    /// 新しい AWS マネージャーを作成
    pub async fn new(region: Option<String>, profile: Option<String>) -> Result<Self> {
        info!("Initializing AWS manager with region: {:?}, profile: {:?}", region, profile);
        
        // AWS 設定を構築
        let mut config_loader = aws_config::defaults(BehaviorVersion::latest());
        
        if let Some(ref region_name) = region {
            config_loader = config_loader.region(Region::new(region_name.clone()));
        }
        
        if let Some(ref profile_name) = profile {
            config_loader = config_loader.profile_name(profile_name);
        }
        
        let config = config_loader.load().await;
        
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
    
    /// 同期版のデフォルト AWS マネージャーを作成（テスト用）
    /// 注意: このメソッドは実際のAWS認証情報を持たないため、テスト専用です
    pub fn default_sync() -> Self {
        // ダミーの設定でクライアントを作成
        let config = aws_config::SdkConfig::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .build();
        
        let ssm_client = SsmClient::new(&config);
        let ec2_client = Ec2Client::new(&config);
        
        Self {
            ssm_client,
            ec2_client,
            region: "us-east-1".to_string(),
        }
    }
    
    /// 指定されたプロファイルで AWS マネージャーを作成
    pub async fn with_profile(profile: &str) -> Result<Self> {
        Self::new(None, Some(profile.to_string())).await
    }
    
    /// 指定されたリージョンで AWS マネージャーを作成
    pub async fn with_region(region: &str) -> Result<Self> {
        Self::new(Some(region.to_string()), None).await
    }
    
    /// SSM セッションを開始
    pub async fn start_ssm_session(&self, config: SsmSessionConfig) -> Result<SsmSession> {
        info!("Starting SSM session for target: {}", config.target);
        
        // EC2 インスタンスの存在確認
        self.verify_instance_exists(&config.target).await
            .context("Failed to verify EC2 instance")?;
        
        // SSM セッションを開始
        let document_name = config.document_name
            .unwrap_or_else(|| "AWS-StartSSHSession".to_string());
        
        let mut request = self.ssm_client
            .start_session()
            .target(&config.target)
            .document_name(&document_name);
        
        // パラメータを設定
        if !config.parameters.is_empty() {
            request = request.set_parameters(Some(config.parameters.clone()));
        }
        
        // 理由を設定
        if let Some(ref reason) = config.reason {
            request = request.reason(reason);
        }
        
        let response = request.send().await
            .context("Failed to start SSM session")?;
        
        let session_id = response.session_id()
            .context("SSM session ID not returned")?
            .to_string();
        
        info!("SSM session started successfully: {}", session_id);
        
        Ok(SsmSession {
            session_id,
            target: config.target,
            status: SsmSessionStatus::Connected,
            created_at: chrono::Utc::now(),
            region: self.region.clone(),
        })
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
    
    /// EC2 インスタンスの存在確認
    async fn verify_instance_exists(&self, instance_id: &str) -> Result<bool> {
        debug!("Verifying EC2 instance exists: {}", instance_id);
        
        let response = self.ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .context("Failed to describe EC2 instance")?;
        
        let exists = response.reservations()
            .iter()
            .any(|reservation| {
                reservation.instances()
                    .iter()
                    .any(|instance| {
                        instance.instance_id.as_deref() == Some(instance_id)
                    })
            });
        
        if exists {
            debug!("EC2 instance verified: {}", instance_id);
        } else {
            warn!("EC2 instance not found: {}", instance_id);
        }
        
        Ok(exists)
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
    
    /// セッション設定からセッションを作成（Auto Reconnector 用）
    pub async fn create_session(&self, config: SessionConfig) -> Result<Session> {
        info!("Creating session for instance: {}", config.instance_id);
        
        // SSM セッション設定を構築
        let mut parameters = HashMap::new();
        parameters.insert("portNumber".to_string(), vec![config.remote_port.to_string()]);
        parameters.insert("localPortNumber".to_string(), vec![config.local_port.to_string()]);
        
        let ssm_config = SsmSessionConfig {
            target: config.instance_id.clone(),
            document_name: Some("AWS-StartPortForwardingSession".to_string()),
            parameters,
            reason: Some("EC2 Connect Auto Reconnection".to_string()),
        };
        
        // SSM セッションを開始
        let ssm_session = self.start_ssm_session(ssm_config).await?;
        
        // Session オブジェクトを作成
        let mut session = Session::new(
            config.instance_id,
            config.local_port,
            config.remote_port,
            config.aws_profile,
            config.region,
        );
        
        // SSM セッション ID を設定
        session.id = ssm_session.session_id;
        session.status = SessionStatus::Active;
        session.created_at = SystemTime::now();
        session.last_activity = SystemTime::now();
        
        info!("Session created successfully: {}", session.id);
        Ok(session)
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