use serde::{Deserialize, Serialize};
use std::time::{SystemTime, Duration};
use uuid::Uuid;
use std::fmt;

/// Session status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Connecting,
    Active,
    Inactive,
    Reconnecting,
    Terminated,
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionStatus::Active => write!(f, "Active"),
            SessionStatus::Connecting => write!(f, "Connecting"),
            SessionStatus::Inactive => write!(f, "Inactive"),
            SessionStatus::Reconnecting => write!(f, "Reconnecting"),
            SessionStatus::Terminated => write!(f, "Terminated"),
        }
    }
}

/// Session priority for resource management
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SessionPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for SessionPriority {
    fn default() -> Self {
        SessionPriority::Normal
    }
}

impl fmt::Display for SessionPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionPriority::Low => write!(f, "Low"),
            SessionPriority::Normal => write!(f, "Normal"),
            SessionPriority::High => write!(f, "High"),
            SessionPriority::Critical => write!(f, "Critical"),
        }
    }
}

/// Session structure representing an SSM connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub instance_id: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub status: SessionStatus,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
    pub process_id: Option<u32>,
    pub connection_count: u32,
    pub data_transferred: u64,
    pub aws_profile: Option<String>,
    pub region: String,
    /// Priority for resource management and scheduling
    pub priority: SessionPriority,
    /// User-defined tags for session organization
    pub tags: std::collections::HashMap<String, String>,
}

impl Session {
    /// Create a new session
    pub fn new(
        instance_id: String,
        local_port: u16,
        remote_port: u16,
        aws_profile: Option<String>,
        region: String,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::new_v4().to_string(),
            instance_id,
            local_port,
            remote_port,
            status: SessionStatus::Connecting,
            created_at: now,
            last_activity: now,
            process_id: None,
            connection_count: 0,
            data_transferred: 0,
            aws_profile,
            region,
            priority: SessionPriority::default(),
            tags: std::collections::HashMap::new(),
        }
    }
    
    /// Create a new session with priority
    pub fn with_priority(
        instance_id: String,
        local_port: u16,
        remote_port: u16,
        aws_profile: Option<String>,
        region: String,
        priority: SessionPriority,
    ) -> Self {
        let mut session = Self::new(instance_id, local_port, remote_port, aws_profile, region);
        session.priority = priority;
        session
    }
    
    /// Set session priority
    pub fn set_priority(&mut self, priority: SessionPriority) {
        self.priority = priority;
    }
    
    /// Add a tag to the session
    pub fn add_tag(&mut self, key: String, value: String) {
        self.tags.insert(key, value);
    }
    
    /// Remove a tag from the session
    pub fn remove_tag(&mut self, key: &str) -> Option<String> {
        self.tags.remove(key)
    }
    
    /// Get a tag value
    pub fn get_tag(&self, key: &str) -> Option<&String> {
        self.tags.get(key)
    }
    
    /// Check if session is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Active)
    }
    
    /// Check if session is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, SessionStatus::Active | SessionStatus::Connecting)
    }
    
    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }
    
    /// Get session age in seconds
    pub fn age_seconds(&self) -> u64 {
        self.created_at
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    
    /// Get time since last activity in seconds
    pub fn idle_seconds(&self) -> u64 {
        self.last_activity
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    
    /// Calculate resource weight based on priority and activity
    pub fn resource_weight(&self) -> f64 {
        let base_weight = match self.priority {
            SessionPriority::Critical => 4.0,
            SessionPriority::High => 2.0,
            SessionPriority::Normal => 1.0,
            SessionPriority::Low => 0.5,
        };
        
        // アクティブなセッションは重みを増加
        if self.is_active() {
            base_weight * 1.5
        } else {
            base_weight
        }
    }
}

/// Session configuration for creating new sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub instance_id: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub aws_profile: Option<String>,
    pub region: String,
    pub priority: SessionPriority,
    pub tags: std::collections::HashMap<String, String>,
}

impl SessionConfig {
    pub fn new(
        instance_id: String,
        local_port: u16,
        remote_port: u16,
        aws_profile: Option<String>,
        region: String,
    ) -> Self {
        Self {
            instance_id,
            local_port,
            remote_port,
            aws_profile,
            region,
            priority: SessionPriority::default(),
            tags: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_priority(mut self, priority: SessionPriority) -> Self {
        self.priority = priority;
        self
    }
    
    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }
}

/// Session health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHealth {
    pub is_healthy: bool,
    pub last_activity: SystemTime,
    pub connection_count: u32,
    pub data_transferred: u64,
    pub response_time_ms: Option<u64>,
    pub process_alive: bool,
    pub port_responsive: bool,
    pub network_activity: bool,
    pub heartbeat_success: bool,
    pub last_heartbeat: Option<SystemTime>,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
}

impl SessionHealth {
    pub fn new(session: &Session) -> Self {
        Self {
            is_healthy: session.is_healthy(),
            last_activity: session.last_activity,
            connection_count: session.connection_count,
            data_transferred: session.data_transferred,
            response_time_ms: None,
            process_alive: session.process_id.is_some(),
            port_responsive: false,
            network_activity: false,
            heartbeat_success: false,
            last_heartbeat: None,
            network_bytes_sent: 0,
            network_bytes_received: 0,
        }
    }
    
    pub fn from_session(session: &Session) -> Self {
        Self::new(session)
    }
    
    /// Update health status based on checks
    pub fn update_health_status(&mut self) {
        // セッションが健全と判定される条件:
        // 1. プロセスが生きている
        // 2. ポートが応答している
        // 3. 最近のハートビートが成功している
        // 4. ネットワーク活動があるか、最後のアクティビティが30秒以内
        let activity_recent = self.last_activity
            .elapsed()
            .map(|d| d.as_secs() < 30)
            .unwrap_or(false);
            
        self.is_healthy = self.process_alive && 
                         self.port_responsive && 
                         self.heartbeat_success &&
                         (self.network_activity || activity_recent);
    }
    
    /// Check if session requires attention (degraded but not completely unhealthy)
    pub fn requires_attention(&self) -> bool {
        !self.is_healthy && (self.process_alive || self.port_responsive)
    }
}

/// Session event enumeration for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEvent {
    HealthDegraded(String),
    TimeoutPredicted(Duration),
    ActivityDetected,
    ConnectionLost,
    ProcessTerminated,
    HeartbeatFailed,
    NetworkActivityDetected,
    SessionIdle,
}

/// Resource usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub active_sessions: u32,
}

impl ResourceUsage {
    pub fn new(memory_mb: f64, cpu_percent: f64, active_sessions: u32) -> Self {
        Self {
            memory_mb,
            cpu_percent,
            active_sessions,
        }
    }
    
    pub fn is_within_limits(&self, max_memory_mb: f64, max_cpu_percent: f64) -> bool {
        self.memory_mb <= max_memory_mb && self.cpu_percent <= max_cpu_percent
    }
}

/// Reconnection policy for auto-reconnector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectionPolicy {
    pub enabled: bool,
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub aggressive_mode: bool,
    pub aggressive_attempts: u32,
    pub aggressive_interval: Duration,
}

impl ReconnectionPolicy {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_attempts: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(16),
            aggressive_mode: false,
            aggressive_attempts: 10,
            aggressive_interval: Duration::from_millis(500),
        }
    }
    
    pub fn aggressive() -> Self {
        Self {
            enabled: true,
            max_attempts: 10,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
            aggressive_mode: true,
            aggressive_attempts: 10,
            aggressive_interval: Duration::from_millis(500),
        }
    }
    
    pub fn conservative() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
            aggressive_mode: false,
            aggressive_attempts: 0,
            aggressive_interval: Duration::from_secs(1),
        }
    }
    
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_attempts: 0,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(1),
            aggressive_mode: false,
            aggressive_attempts: 0,
            aggressive_interval: Duration::from_secs(1),
        }
    }
    
    /// Calculate delay for a specific attempt
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if self.aggressive_mode && attempt <= self.aggressive_attempts {
            self.aggressive_interval
        } else {
            let delay = self.base_delay.as_millis() as u64 * (2_u64.pow(attempt.saturating_sub(1)));
            let max_delay = self.max_delay.as_millis() as u64;
            Duration::from_millis(std::cmp::min(delay, max_delay))
        }
    }
}