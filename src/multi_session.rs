use crate::session::{Session, SessionConfig, SessionPriority, SessionStatus};
use crate::manager::{SessionManager, ResourceUsage, SessionStatistics};
use crate::monitor::{SessionMonitor, SessionHealth};
use crate::error::{Result, SessionError};
use std::collections::{HashMap, BTreeMap};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, Instant};

/// Resource warning levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceWarningLevel {
    Normal,
    Warning,
    Critical,
    Emergency,
}

/// Resource warning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceWarning {
    pub level: ResourceWarningLevel,
    pub message: String,
    pub resource_type: String,
    pub current_value: f64,
    pub threshold: f64,
    pub timestamp: SystemTime,
    pub affected_sessions: Vec<String>,
}

/// Multi-session management state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSessionState {
    pub total_sessions: u32,
    pub active_sessions: u32,
    pub sessions_by_priority: HashMap<SessionPriority, u32>,
    pub sessions_by_instance: HashMap<String, u32>,
    pub resource_usage: ResourceUsage,
    pub resource_warnings: Vec<ResourceWarning>,
    pub last_updated: SystemTime,
}

/// Session priority queue for resource management
#[derive(Debug)]
pub struct SessionPriorityQueue {
    sessions_by_priority: BTreeMap<SessionPriority, Vec<String>>,
    session_weights: HashMap<String, f64>,
}

impl SessionPriorityQueue {
    pub fn new() -> Self {
        Self {
            sessions_by_priority: BTreeMap::new(),
            session_weights: HashMap::new(),
        }
    }
    
    /// Add session to priority queue
    pub fn add_session(&mut self, session: &Session) {
        let priority = session.priority;
        let weight = session.resource_weight();
        
        self.sessions_by_priority
            .entry(priority)
            .or_insert_with(Vec::new)
            .push(session.id.clone());
        
        self.session_weights.insert(session.id.clone(), weight);
        
        debug!("Added session {} with priority {:?} and weight {:.2}", 
               session.id, priority, weight);
    }
    
    /// Remove session from priority queue
    pub fn remove_session(&mut self, session_id: &str) {
        for sessions in self.sessions_by_priority.values_mut() {
            sessions.retain(|id| id != session_id);
        }
        self.session_weights.remove(session_id);
        
        debug!("Removed session {} from priority queue", session_id);
    }
    
    /// Get sessions ordered by priority (highest first)
    pub fn get_sessions_by_priority(&self) -> Vec<String> {
        let mut result = Vec::new();
        
        // 優先度の高い順（Critical -> High -> Normal -> Low）
        for priority in [SessionPriority::Critical, SessionPriority::High, 
                        SessionPriority::Normal, SessionPriority::Low] {
            if let Some(sessions) = self.sessions_by_priority.get(&priority) {
                // 同じ優先度内では重みでソート
                let mut weighted_sessions: Vec<_> = sessions.iter()
                    .map(|id| (id.clone(), self.session_weights.get(id).copied().unwrap_or(0.0)))
                    .collect();
                
                weighted_sessions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                
                for (session_id, _weight) in weighted_sessions {
                    result.push(session_id);
                }
            }
        }
        
        result
    }
    
    /// Get sessions that can be terminated to free resources
    pub fn get_terminable_sessions(&self, required_weight: f64) -> Vec<String> {
        let mut candidates = Vec::new();
        let mut accumulated_weight = 0.0;
        
        // 優先度の低い順（Low -> Normal -> High）でチェック
        for priority in [SessionPriority::Low, SessionPriority::Normal, SessionPriority::High] {
            if let Some(sessions) = self.sessions_by_priority.get(&priority) {
                for session_id in sessions {
                    if let Some(&weight) = self.session_weights.get(session_id) {
                        candidates.push(session_id.clone());
                        accumulated_weight += weight;
                        
                        if accumulated_weight >= required_weight {
                            return candidates;
                        }
                    }
                }
            }
        }
        
        candidates
    }
}

/// Multi-session manager for coordinating multiple sessions
pub struct MultiSessionManager<M: SessionManager, Mon: SessionMonitor> {
    session_manager: Arc<Mutex<M>>,
    session_monitor: Arc<Mutex<Mon>>,
    priority_queue: Arc<Mutex<SessionPriorityQueue>>,
    resource_thresholds: ResourceThresholds,
    state: Arc<RwLock<MultiSessionState>>,
    warning_history: Arc<RwLock<Vec<ResourceWarning>>>,
    max_warning_history: usize,
}

/// Resource usage thresholds for warnings
#[derive(Debug, Clone)]
pub struct ResourceThresholds {
    pub memory_warning_mb: f64,
    pub memory_critical_mb: f64,
    pub cpu_warning_percent: f64,
    pub cpu_critical_percent: f64,
    pub max_sessions_per_instance: u32,
    pub max_total_sessions: u32,
}

impl Default for ResourceThresholds {
    fn default() -> Self {
        Self {
            memory_warning_mb: 8.0,    // 8MB で警告
            memory_critical_mb: 10.0,  // 10MB で危険
            cpu_warning_percent: 0.3,  // 0.3% で警告
            cpu_critical_percent: 0.5, // 0.5% で危険
            max_sessions_per_instance: 3,
            max_total_sessions: 10,
        }
    }
}

impl<M: SessionManager + Send + Sync, Mon: SessionMonitor + Send + Sync> MultiSessionManager<M, Mon> {
    pub fn new(
        session_manager: M,
        session_monitor: Mon,
        resource_thresholds: Option<ResourceThresholds>,
    ) -> Self {
        let initial_state = MultiSessionState {
            total_sessions: 0,
            active_sessions: 0,
            sessions_by_priority: HashMap::new(),
            sessions_by_instance: HashMap::new(),
            resource_usage: ResourceUsage {
                memory_mb: 0.0,
                cpu_percent: 0.0,
                active_sessions: 0,
            },
            resource_warnings: Vec::new(),
            last_updated: SystemTime::now(),
        };
        
        Self {
            session_manager: Arc::new(Mutex::new(session_manager)),
            session_monitor: Arc::new(Mutex::new(session_monitor)),
            priority_queue: Arc::new(Mutex::new(SessionPriorityQueue::new())),
            resource_thresholds: resource_thresholds.unwrap_or_default(),
            state: Arc::new(RwLock::new(initial_state)),
            warning_history: Arc::new(RwLock::new(Vec::new())),
            max_warning_history: 100,
        }
    }
    
    /// Create a new session with priority management
    pub async fn create_session_with_priority(
        &self,
        config: SessionConfig,
    ) -> Result<Session> {
        info!("Creating session with priority {:?} for instance: {}", 
              config.priority, config.instance_id);
        
        // リソース使用量をチェック
        let current_usage = self.session_manager.lock().await.monitor_resource_usage().await?;
        
        // リソース制限をチェック
        if let Err(e) = self.check_resource_limits(&current_usage, &config).await {
            // 低優先度セッションの終了を試行
            if config.priority >= SessionPriority::High {
                info!("Attempting to free resources for high-priority session");
                if let Ok(freed) = self.free_resources_for_priority(config.priority).await {
                    if freed > 0 {
                        info!("Freed {} low-priority sessions for new high-priority session", freed);
                    } else {
                        return Err(e);
                    }
                } else {
                    return Err(e);
                }
            } else {
                return Err(e);
            }
        }
        
        // セッションを作成
        let mut session = self.session_manager.lock().await.create_session(config.clone()).await?;
        session.priority = config.priority;
        session.tags = config.tags;
        
        // 優先度キューに追加
        self.priority_queue.lock().await.add_session(&session);
        
        // 監視を開始
        self.session_monitor.lock().await.start_monitoring(&session.id).await?;
        
        // 状態を更新
        self.update_state().await?;
        
        info!("Successfully created session {} with priority {:?}", 
              session.id, session.priority);
        
        Ok(session)
    }
    
    /// Terminate session with priority consideration
    pub async fn terminate_session_with_priority(&self, session_id: &str) -> Result<()> {
        info!("Terminating session: {}", session_id);
        
        // 監視を停止
        self.session_monitor.lock().await.stop_monitoring(session_id).await?;
        
        // セッションを終了
        self.session_manager.lock().await.terminate_session(session_id).await?;
        
        // 優先度キューから削除
        self.priority_queue.lock().await.remove_session(session_id);
        
        // 状態を更新
        self.update_state().await?;
        
        info!("Successfully terminated session: {}", session_id);
        
        Ok(())
    }
    
    /// Free resources by terminating low-priority sessions
    pub async fn free_resources_for_priority(&self, required_priority: SessionPriority) -> Result<u32> {
        let sessions = self.session_manager.lock().await.list_sessions().await?;
        let mut terminated_count = 0;
        
        // 要求された優先度より低いセッションを特定
        let terminable_sessions: Vec<_> = sessions.iter()
            .filter(|s| s.priority < required_priority && s.is_active())
            .collect();
        
        if terminable_sessions.is_empty() {
            info!("No terminable sessions found for priority {:?}", required_priority);
            return Ok(0);
        }
        
        // 優先度の低い順に終了
        let mut sorted_sessions = terminable_sessions;
        sorted_sessions.sort_by_key(|s| s.priority);
        
        for session in sorted_sessions {
            if terminated_count >= 2 { // 最大2つまで終了
                break;
            }
            
            warn!("Terminating low-priority session {} (priority: {:?}) to free resources", 
                  session.id, session.priority);
            
            if let Err(e) = self.terminate_session_with_priority(&session.id).await {
                error!("Failed to terminate session {}: {}", session.id, e);
            } else {
                terminated_count += 1;
            }
        }
        
        Ok(terminated_count)
    }
    
    /// Check resource limits before creating new session
    async fn check_resource_limits(&self, usage: &ResourceUsage, config: &SessionConfig) -> Result<()> {
        // メモリ制限チェック
        if usage.memory_mb >= self.resource_thresholds.memory_critical_mb {
            return Err(SessionError::ResourceLimitExceeded {
                resource: "memory".to_string(),
                current: usage.memory_mb,
                limit: self.resource_thresholds.memory_critical_mb,
            }.into());
        }
        
        // CPU制限チェック
        if usage.cpu_percent >= self.resource_thresholds.cpu_critical_percent {
            return Err(SessionError::ResourceLimitExceeded {
                resource: "cpu".to_string(),
                current: usage.cpu_percent,
                limit: self.resource_thresholds.cpu_critical_percent,
            }.into());
        }
        
        // 総セッション数制限チェック
        if usage.active_sessions >= self.resource_thresholds.max_total_sessions {
            return Err(SessionError::LimitExceeded {
                max_sessions: self.resource_thresholds.max_total_sessions,
            }.into());
        }
        
        // インスタンス別セッション数制限チェック
        let instance_sessions = self.session_manager.lock().await
            .list_sessions_by_instance(&config.instance_id).await?;
        let active_instance_sessions = instance_sessions.iter()
            .filter(|s| s.is_active())
            .count() as u32;
        
        if active_instance_sessions >= self.resource_thresholds.max_sessions_per_instance {
            return Err(SessionError::LimitExceeded {
                max_sessions: self.resource_thresholds.max_sessions_per_instance,
            }.into());
        }
        
        Ok(())
    }
    
    /// Update multi-session state
    pub async fn update_state(&self) -> Result<()> {
        let sessions = self.session_manager.lock().await.list_sessions().await?;
        let resource_usage = self.session_manager.lock().await.monitor_resource_usage().await?;
        
        let mut sessions_by_priority = HashMap::new();
        let mut sessions_by_instance = HashMap::new();
        
        for session in &sessions {
            *sessions_by_priority.entry(session.priority).or_insert(0) += 1;
            *sessions_by_instance.entry(session.instance_id.clone()).or_insert(0) += 1;
        }
        
        // リソース警告をチェック
        let warnings = self.check_resource_warnings(&resource_usage, &sessions).await;
        
        let new_state = MultiSessionState {
            total_sessions: sessions.len() as u32,
            active_sessions: sessions.iter().filter(|s| s.is_active()).count() as u32,
            sessions_by_priority,
            sessions_by_instance,
            resource_usage,
            resource_warnings: warnings.clone(),
            last_updated: SystemTime::now(),
        };
        
        *self.state.write().await = new_state;
        
        // 警告履歴を更新
        if !warnings.is_empty() {
            let mut history = self.warning_history.write().await;
            history.extend(warnings);
            
            // 履歴サイズを制限
            let history_len = history.len();
            if history_len > self.max_warning_history {
                history.drain(0..history_len - self.max_warning_history);
            }
        }
        
        Ok(())
    }
    
    /// Check for resource warnings
    async fn check_resource_warnings(&self, usage: &ResourceUsage, sessions: &[Session]) -> Vec<ResourceWarning> {
        let mut warnings = Vec::new();
        let now = SystemTime::now();
        
        // メモリ警告
        if usage.memory_mb >= self.resource_thresholds.memory_critical_mb {
            warnings.push(ResourceWarning {
                level: ResourceWarningLevel::Critical,
                message: format!("Memory usage is critical: {:.1}MB (limit: {:.1}MB)", 
                               usage.memory_mb, self.resource_thresholds.memory_critical_mb),
                resource_type: "memory".to_string(),
                current_value: usage.memory_mb,
                threshold: self.resource_thresholds.memory_critical_mb,
                timestamp: now,
                affected_sessions: sessions.iter().map(|s| s.id.clone()).collect(),
            });
        } else if usage.memory_mb >= self.resource_thresholds.memory_warning_mb {
            warnings.push(ResourceWarning {
                level: ResourceWarningLevel::Warning,
                message: format!("Memory usage is high: {:.1}MB (warning: {:.1}MB)", 
                               usage.memory_mb, self.resource_thresholds.memory_warning_mb),
                resource_type: "memory".to_string(),
                current_value: usage.memory_mb,
                threshold: self.resource_thresholds.memory_warning_mb,
                timestamp: now,
                affected_sessions: sessions.iter().map(|s| s.id.clone()).collect(),
            });
        }
        
        // CPU警告
        if usage.cpu_percent >= self.resource_thresholds.cpu_critical_percent {
            warnings.push(ResourceWarning {
                level: ResourceWarningLevel::Critical,
                message: format!("CPU usage is critical: {:.1}% (limit: {:.1}%)", 
                               usage.cpu_percent, self.resource_thresholds.cpu_critical_percent),
                resource_type: "cpu".to_string(),
                current_value: usage.cpu_percent,
                threshold: self.resource_thresholds.cpu_critical_percent,
                timestamp: now,
                affected_sessions: sessions.iter().map(|s| s.id.clone()).collect(),
            });
        } else if usage.cpu_percent >= self.resource_thresholds.cpu_warning_percent {
            warnings.push(ResourceWarning {
                level: ResourceWarningLevel::Warning,
                message: format!("CPU usage is high: {:.1}% (warning: {:.1}%)", 
                               usage.cpu_percent, self.resource_thresholds.cpu_warning_percent),
                resource_type: "cpu".to_string(),
                current_value: usage.cpu_percent,
                threshold: self.resource_thresholds.cpu_warning_percent,
                timestamp: now,
                affected_sessions: sessions.iter().map(|s| s.id.clone()).collect(),
            });
        }
        
        // インスタンス別セッション数警告
        let mut instance_counts = HashMap::new();
        for session in sessions {
            if session.is_active() {
                *instance_counts.entry(&session.instance_id).or_insert(0) += 1;
            }
        }
        
        for (instance_id, count) in instance_counts {
            if count >= self.resource_thresholds.max_sessions_per_instance {
                let affected_sessions: Vec<String> = sessions.iter()
                    .filter(|s| s.instance_id == *instance_id && s.is_active())
                    .map(|s| s.id.clone())
                    .collect();
                
                warnings.push(ResourceWarning {
                    level: ResourceWarningLevel::Warning,
                    message: format!("Instance {} has {} active sessions (limit: {})", 
                                   instance_id, count, self.resource_thresholds.max_sessions_per_instance),
                    resource_type: "sessions_per_instance".to_string(),
                    current_value: count as f64,
                    threshold: self.resource_thresholds.max_sessions_per_instance as f64,
                    timestamp: now,
                    affected_sessions,
                });
            }
        }
        
        warnings
    }
    
    /// Get current multi-session state
    pub async fn get_state(&self) -> MultiSessionState {
        self.state.read().await.clone()
    }
    
    /// Get resource warning history
    pub async fn get_warning_history(&self) -> Vec<ResourceWarning> {
        self.warning_history.read().await.clone()
    }
    
    /// Get sessions ordered by priority
    pub async fn get_sessions_by_priority(&self) -> Result<Vec<Session>> {
        let session_ids = self.priority_queue.lock().await.get_sessions_by_priority();
        let mut sessions = Vec::new();
        
        let manager = self.session_manager.lock().await;
        for session_id in session_ids {
            if let Ok(session) = manager.get_session(&session_id).await {
                sessions.push(session);
            }
        }
        
        Ok(sessions)
    }
    
    /// Perform resource optimization
    pub async fn optimize_resources(&self) -> Result<u32> {
        info!("Starting resource optimization");
        
        let current_usage = self.session_manager.lock().await.monitor_resource_usage().await?;
        let mut optimized_count = 0;
        
        // メモリ使用量が警告レベルを超えている場合
        if current_usage.memory_mb > self.resource_thresholds.memory_warning_mb {
            // 非アクティブセッションをクリーンアップ
            let cleaned = self.session_manager.lock().await.cleanup_inactive_sessions().await?;
            optimized_count += cleaned;
            
            if cleaned > 0 {
                info!("Cleaned up {} inactive sessions to reduce memory usage", cleaned);
            }
        }
        
        // CPU使用量が警告レベルを超えている場合
        if current_usage.cpu_percent > self.resource_thresholds.cpu_warning_percent {
            // 低優先度のアイドルセッションを終了
            let sessions = self.session_manager.lock().await.list_sessions().await?;
            let idle_low_priority: Vec<_> = sessions.iter()
                .filter(|s| s.priority == SessionPriority::Low && s.idle_seconds() > 300)
                .collect();
            
            for session in idle_low_priority.iter().take(2) { // 最大2つまで
                if let Err(e) = self.terminate_session_with_priority(&session.id).await {
                    error!("Failed to terminate idle low-priority session {}: {}", session.id, e);
                } else {
                    optimized_count += 1;
                    info!("Terminated idle low-priority session: {}", session.id);
                }
            }
        }
        
        // 状態を更新
        self.update_state().await?;
        
        info!("Resource optimization completed, optimized {} sessions", optimized_count);
        Ok(optimized_count)
    }
    
    /// Get comprehensive session statistics
    pub async fn get_comprehensive_statistics(&self) -> Result<SessionStatistics> {
        let base_stats = self.session_manager.lock().await.get_session_statistics().await?;
        
        // 追加の統計情報を含める
        Ok(base_stats)
    }
}