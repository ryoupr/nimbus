use crate::aws::{AwsManager, SsmSessionConfig, SsmSessionStatus};
use crate::session::{Session, SessionConfig, SessionStatus};
use crate::error::{Result, SessionError};
use std::collections::HashMap;
use tracing::{info, warn, error, debug};
use anyhow::Context;
use serde::{Serialize, Deserialize};

/// Resource usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub active_sessions: u32,
}

/// Session statistics for monitoring and reporting
#[derive(Debug, Clone)]
pub struct SessionStatistics {
    pub total_sessions: u32,
    pub active_sessions: u32,
    pub inactive_sessions: u32,
    pub terminated_sessions: u32,
    pub sessions_by_instance: std::collections::HashMap<String, u32>,
    pub average_session_age_seconds: f64,
    pub average_idle_time_seconds: f64,
    pub resource_usage: ResourceUsage,
}

/// Session manager trait for managing multiple sessions
pub trait SessionManager {
    fn create_session(&mut self, config: SessionConfig) -> impl std::future::Future<Output = Result<Session>> + Send;
    fn find_existing_sessions(&self, instance_id: &str, port: u16) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send;
    fn suggest_reuse(&self, sessions: &[Session]) -> impl std::future::Future<Output = Option<Session>> + Send;
    fn monitor_resource_usage(&self) -> impl std::future::Future<Output = Result<ResourceUsage>> + Send;
    fn enforce_limits(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_session(&self, session_id: &str) -> impl std::future::Future<Output = Result<Session>> + Send;
    fn update_session(&mut self, session: Session) -> impl std::future::Future<Output = Result<()>> + Send;
    fn terminate_session(&mut self, session_id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    fn list_sessions(&self) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send;
    
    // 新しいメソッド - セッション管理最適化のため
    fn list_active_sessions(&self) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send;
    fn list_sessions_by_instance(&self, instance_id: &str) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send;
    fn cleanup_inactive_sessions(&mut self) -> impl std::future::Future<Output = Result<u32>> + Send;
    fn get_session_statistics(&self) -> impl std::future::Future<Output = Result<SessionStatistics>> + Send;
}

/// Default implementation of session manager with AWS integration
pub struct DefaultSessionManager {
    sessions: HashMap<String, Session>,
    aws_sessions: HashMap<String, String>, // session_id -> ssm_session_id mapping
    max_sessions_per_instance: u32,
    aws_manager: AwsManager,
}

impl DefaultSessionManager {
    pub async fn new(max_sessions_per_instance: u32) -> Result<Self> {
        let aws_manager = AwsManager::default().await
            .context("Failed to initialize AWS manager")?;
        
        Ok(Self {
            sessions: HashMap::new(),
            aws_sessions: HashMap::new(),
            max_sessions_per_instance,
            aws_manager,
        })
    }
    
    pub async fn with_profile(max_sessions_per_instance: u32, profile: &str) -> Result<Self> {
        let aws_manager = AwsManager::with_profile(profile).await
            .context("Failed to initialize AWS manager with profile")?;
        
        Ok(Self {
            sessions: HashMap::new(),
            aws_sessions: HashMap::new(),
            max_sessions_per_instance,
            aws_manager,
        })
    }
    
    pub async fn with_region(max_sessions_per_instance: u32, region: &str) -> Result<Self> {
        let aws_manager = AwsManager::with_region(region).await
            .context("Failed to initialize AWS manager with region")?;
        
        Ok(Self {
            sessions: HashMap::new(),
            aws_sessions: HashMap::new(),
            max_sessions_per_instance,
            aws_manager,
        })
    }
    
    /// Count active sessions for an instance
    fn count_instance_sessions(&self, instance_id: &str) -> u32 {
        self.sessions
            .values()
            .filter(|s| s.instance_id == instance_id && s.is_active())
            .count() as u32
    }
    
    /// Check if session can be reused
    fn can_reuse_session(&self, session: &Session, config: &SessionConfig) -> bool {
        session.instance_id == config.instance_id
            && session.local_port == config.local_port
            && session.remote_port == config.remote_port
            && session.is_healthy()
            && session.aws_profile == config.aws_profile
            && session.region == config.region
    }
    
    /// Check if session is inactive based on activity criteria
    fn is_session_inactive(&self, session: &Session) -> bool {
        let idle_time = session.idle_seconds();
        
        // セッションが非アクティブと判定される条件:
        // 1. ローカルポートへの新規接続が30秒以上ない
        // 2. ポートフォワーディング経由のデータ転送が30秒以上ない
        // 3. SSMセッションプロセスが応答しない状態が5秒以上継続
        
        // 基本的なアイドル時間チェック（30秒）
        let is_idle = idle_time > 30;
        
        // プロセスが応答しない場合（5秒以上）
        let process_unresponsive = session.process_id.is_none() && idle_time > 5;
        
        // セッションが終了状態の場合
        let is_terminated = matches!(session.status, SessionStatus::Terminated | SessionStatus::Inactive);
        
        is_idle || process_unresponsive || is_terminated
    }
    
    /// Find sessions that can be cleaned up
    pub fn find_inactive_sessions(&self) -> Vec<String> {
        self.sessions
            .iter()
            .filter(|(_, session)| self.is_session_inactive(session))
            .map(|(id, _)| id.clone())
            .collect()
    }
    
    /// Clean up inactive sessions
    pub async fn cleanup_inactive_sessions(&mut self) -> Result<u32> {
        let inactive_session_ids = self.find_inactive_sessions();
        let count = inactive_session_ids.len() as u32;
        
        for session_id in inactive_session_ids {
            if let Err(e) = self.terminate_session(&session_id).await {
                warn!("Failed to cleanup inactive session {}: {}", session_id, e);
            } else {
                info!("Cleaned up inactive session: {}", session_id);
            }
        }
        
        Ok(count)
    }
    
    /// Sync session status with AWS SSM
    pub async fn sync_session_status(&mut self, session_id: &str) -> Result<()> {
        if let Some(ssm_session_id) = self.aws_sessions.get(session_id) {
            match self.aws_manager.get_session_status(ssm_session_id).await {
                Ok(ssm_status) => {
                    if let Some(session) = self.sessions.get_mut(session_id) {
                        let new_status = match ssm_status {
                            SsmSessionStatus::Connected => SessionStatus::Active,
                            SsmSessionStatus::Connecting => SessionStatus::Connecting,
                            SsmSessionStatus::Disconnected => SessionStatus::Inactive,
                            SsmSessionStatus::Failed => SessionStatus::Terminated,
                            SsmSessionStatus::Terminated => SessionStatus::Terminated,
                            SsmSessionStatus::Terminating => SessionStatus::Terminated,
                        };
                        
                        if session.status != new_status {
                            debug!("Session {} status changed from {:?} to {:?}", 
                                   session_id, session.status, new_status);
                            session.status = new_status;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to get SSM session status for {}: {}", session_id, e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Sync all session statuses with AWS SSM
    pub async fn sync_all_sessions(&mut self) -> Result<()> {
        let session_ids: Vec<String> = self.sessions.keys().cloned().collect();
        
        for session_id in session_ids {
            if let Err(e) = self.sync_session_status(&session_id).await {
                warn!("Failed to sync session {}: {}", session_id, e);
            }
        }
        
        Ok(())
    }
    
    /// Get AWS manager reference
    pub fn aws_manager(&self) -> &AwsManager {
        &self.aws_manager
    }
    
    /// Get SSM session ID for a local session
    pub fn get_ssm_session_id(&self, session_id: &str) -> Option<&String> {
        self.aws_sessions.get(session_id)
    }
}

impl SessionManager for DefaultSessionManager {
    async fn create_session(&mut self, config: SessionConfig) -> Result<Session> {
        info!("Creating new session for instance: {}", config.instance_id);
        
        // 既存セッションの検索と再利用提案（要件 3.1, 3.2）
        let existing_sessions = self.find_existing_sessions(&config.instance_id, config.local_port).await?;
        
        // 健全な既存セッションがある場合は再利用を提案
        if let Some(reusable_session) = self.suggest_reuse(&existing_sessions).await {
            if self.can_reuse_session(&reusable_session, &config) {
                info!("Suggesting reuse of existing session: {}", reusable_session.id);
                // 実際の再利用は呼び出し側で判断されるため、ここでは既存セッションを返す
                return Ok(reusable_session);
            }
        }
        
        // セッション制限の確認（要件 3.5）
        let current_count = self.count_instance_sessions(&config.instance_id);
        if current_count >= self.max_sessions_per_instance {
            // 非アクティブセッションのクリーンアップを試行
            let cleaned_count = self.cleanup_inactive_sessions().await?;
            info!("Cleaned up {} inactive sessions", cleaned_count);
            
            // クリーンアップ後も制限を超える場合はエラー
            let updated_count = self.count_instance_sessions(&config.instance_id);
            if updated_count >= self.max_sessions_per_instance {
                return Err(SessionError::LimitExceeded {
                    max_sessions: self.max_sessions_per_instance,
                }.into());
            }
        }
        
        // Create local session object
        let mut session = Session::with_priority(
            config.instance_id.clone(),
            config.local_port,
            config.remote_port,
            config.aws_profile.clone(),
            config.region.clone(),
            config.priority,
        );
        
        // Add tags from config
        for (key, value) in config.tags {
            session.add_tag(key, value);
        }
        
        // Create AWS SSM session configuration
        let mut parameters = HashMap::new();
        parameters.insert("portNumber".to_string(), vec![config.remote_port.to_string()]);
        parameters.insert("localPortNumber".to_string(), vec![config.local_port.to_string()]);
        
        let ssm_config = SsmSessionConfig {
            target: config.instance_id.clone(),
            document_name: Some("AWS-StartPortForwardingSession".to_string()),
            parameters,
            reason: Some("EC2 Connect automated session".to_string()),
        };
        
        // Start AWS SSM session
        match self.aws_manager.start_ssm_session(ssm_config).await {
            Ok(ssm_session) => {
                session.status = SessionStatus::Active;
                session.update_activity();
                
                let session_id = session.id.clone();
                info!("Created session: {} with SSM session: {}", session_id, &ssm_session.session_id);
                
                self.sessions.insert(session_id.clone(), session.clone());
                self.aws_sessions.insert(session_id.clone(), ssm_session.session_id);
                Ok(session)
            }
            Err(e) => {
                error!("Failed to create SSM session: {}", e);
                session.status = SessionStatus::Terminated;
                Err(e.into())
            }
        }
    }
    
    async fn find_existing_sessions(&self, instance_id: &str, port: u16) -> Result<Vec<Session>> {
        let sessions: Vec<Session> = self.sessions
            .values()
            .filter(|s| {
                s.instance_id == instance_id && 
                s.local_port == port &&
                !matches!(s.status, SessionStatus::Terminated)
            })
            .cloned()
            .collect();
        
        info!("Found {} existing sessions for instance {} on port {}", 
              sessions.len(), instance_id, port);
        
        Ok(sessions)
    }
    
    async fn suggest_reuse(&self, sessions: &[Session]) -> Option<Session> {
        // 健全なセッションのみを対象とする
        let healthy_sessions: Vec<&Session> = sessions
            .iter()
            .filter(|s| s.is_healthy() && !self.is_session_inactive(s))
            .collect();
        
        if healthy_sessions.is_empty() {
            info!("No healthy sessions available for reuse");
            return None;
        }
        
        // 最も最近活動があったセッションを選択
        let best_session = healthy_sessions
            .iter()
            .min_by_key(|s| s.idle_seconds())
            .copied();
        
        if let Some(session) = best_session {
            info!("Suggesting reuse of session: {} (idle for {} seconds)", 
                  session.id, session.idle_seconds());
            Some(session.clone())
        } else {
            None
        }
    }
    
    async fn monitor_resource_usage(&self) -> Result<ResourceUsage> {
        // 実際のリソース監視の実装
        let active_sessions = self.sessions
            .values()
            .filter(|s| s.is_active())
            .count() as u32;
        
        // TODO: 実際のメモリとCPU使用量を測定
        // 現在はプレースホルダー実装
        let memory_mb = (active_sessions as f64) * 2.0 + 3.0; // セッションあたり約2MB + ベース3MB
        let cpu_percent = (active_sessions as f64) * 0.1 + 0.1; // セッションあたり約0.1% + ベース0.1%
        
        let usage = ResourceUsage {
            memory_mb,
            cpu_percent,
            active_sessions,
        };
        
        debug!("Resource usage: {:.1}MB memory, {:.1}% CPU, {} active sessions", 
               usage.memory_mb, usage.cpu_percent, usage.active_sessions);
        
        Ok(usage)
    }
    
    async fn enforce_limits(&mut self) -> Result<()> {
        let usage = self.monitor_resource_usage().await?;
        
        // リソース制限の確認と警告
        if usage.memory_mb > 10.0 {
            warn!("Memory usage exceeds limit: {:.1}MB > 10.0MB", usage.memory_mb);
            
            // 非アクティブセッションのクリーンアップを試行
            let cleaned_count = self.cleanup_inactive_sessions().await?;
            if cleaned_count > 0 {
                info!("Cleaned up {} inactive sessions to reduce memory usage", cleaned_count);
            }
        }
        
        if usage.cpu_percent > 0.5 {
            warn!("CPU usage exceeds limit: {:.1}% > 0.5%", usage.cpu_percent);
        }
        
        // インスタンスごとのセッション制限確認
        let mut instances_over_limit = Vec::new();
        let mut instance_counts = std::collections::HashMap::new();
        
        for session in self.sessions.values() {
            if session.is_active() {
                *instance_counts.entry(&session.instance_id).or_insert(0) += 1;
            }
        }
        
        for (instance_id, count) in instance_counts {
            if count > self.max_sessions_per_instance {
                instances_over_limit.push((instance_id.clone(), count));
            }
        }
        
        if !instances_over_limit.is_empty() {
            warn!("Instances over session limit: {:?}", instances_over_limit);
        }
        
        Ok(())
    }
    
    async fn get_session(&self, session_id: &str) -> Result<Session> {
        self.sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| SessionError::NotFound {
                session_id: session_id.to_string(),
            }.into())
    }
    
    async fn update_session(&mut self, session: Session) -> Result<()> {
        let session_id = session.id.clone();
        self.sessions.insert(session_id, session);
        Ok(())
    }
    
    async fn terminate_session(&mut self, session_id: &str) -> Result<()> {
        if let Some(mut session) = self.sessions.remove(session_id) {
            session.status = SessionStatus::Terminated;
            
            // Terminate AWS SSM session if it exists
            if let Some(ssm_session_id) = self.aws_sessions.remove(session_id) {
                match self.aws_manager.terminate_ssm_session(&ssm_session_id).await {
                    Ok(_) => {
                        info!("Terminated SSM session: {}", ssm_session_id);
                    }
                    Err(e) => {
                        warn!("Failed to terminate SSM session {}: {}", ssm_session_id, e);
                    }
                }
            }
            
            info!("Terminated session: {}", session_id);
            Ok(())
        } else {
            Err(SessionError::NotFound {
                session_id: session_id.to_string(),
            }.into())
        }
    }
    
    async fn list_sessions(&self) -> Result<Vec<Session>> {
        Ok(self.sessions.values().cloned().collect())
    }
    
    async fn list_active_sessions(&self) -> Result<Vec<Session>> {
        Ok(self.sessions
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect())
    }
    
    async fn list_sessions_by_instance(&self, instance_id: &str) -> Result<Vec<Session>> {
        Ok(self.sessions
            .values()
            .filter(|s| s.instance_id == instance_id)
            .cloned()
            .collect())
    }
    
    async fn cleanup_inactive_sessions(&mut self) -> Result<u32> {
        let inactive_session_ids = self.find_inactive_sessions();
        let count = inactive_session_ids.len() as u32;
        
        for session_id in inactive_session_ids {
            if let Err(e) = self.terminate_session(&session_id).await {
                warn!("Failed to cleanup inactive session {}: {}", session_id, e);
            } else {
                info!("Cleaned up inactive session: {}", session_id);
            }
        }
        
        Ok(count)
    }
    
    async fn get_session_statistics(&self) -> Result<SessionStatistics> {
        let all_sessions: Vec<&Session> = self.sessions.values().collect();
        let total_sessions = all_sessions.len() as u32;
        
        let active_sessions = all_sessions.iter()
            .filter(|s| s.is_active())
            .count() as u32;
            
        let inactive_sessions = all_sessions.iter()
            .filter(|s| self.is_session_inactive(s))
            .count() as u32;
            
        let terminated_sessions = all_sessions.iter()
            .filter(|s| matches!(s.status, SessionStatus::Terminated))
            .count() as u32;
        
        // インスタンスごとのセッション数
        let mut sessions_by_instance = std::collections::HashMap::new();
        for session in &all_sessions {
            *sessions_by_instance.entry(session.instance_id.clone()).or_insert(0) += 1;
        }
        
        // 平均セッション年齢
        let average_session_age_seconds = if total_sessions > 0 {
            all_sessions.iter()
                .map(|s| s.age_seconds() as f64)
                .sum::<f64>() / total_sessions as f64
        } else {
            0.0
        };
        
        // 平均アイドル時間
        let average_idle_time_seconds = if total_sessions > 0 {
            all_sessions.iter()
                .map(|s| s.idle_seconds() as f64)
                .sum::<f64>() / total_sessions as f64
        } else {
            0.0
        };
        
        let resource_usage = self.monitor_resource_usage().await?;
        
        Ok(SessionStatistics {
            total_sessions,
            active_sessions,
            inactive_sessions,
            terminated_sessions,
            sessions_by_instance,
            average_session_age_seconds,
            average_idle_time_seconds,
            resource_usage,
        })
    }
}