#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
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
    pub remote_host: Option<String>,
    pub status: SessionStatus,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
    pub process_id: Option<u32>,
    pub connection_count: u32,
    pub data_transferred: u64,
    pub aws_profile: Option<String>,
    pub region: String,
    pub priority: SessionPriority,
    pub tags: std::collections::HashMap<String, String>,
}

impl Session {
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
            remote_host: None,
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
    
    pub fn add_tag(&mut self, key: String, value: String) {
        self.tags.insert(key, value);
    }
    
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Active)
    }
    
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, SessionStatus::Active | SessionStatus::Connecting)
    }
    
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }
    
    pub fn age_seconds(&self) -> u64 {
        self.created_at
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    
    pub fn idle_seconds(&self) -> u64 {
        self.last_activity
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    
    pub fn resource_weight(&self) -> f64 {
        let base_weight = match self.priority {
            SessionPriority::Critical => 4.0,
            SessionPriority::High => 2.0,
            SessionPriority::Normal => 1.0,
            SessionPriority::Low => 0.5,
        };
        if self.is_active() { base_weight * 1.5 } else { base_weight }
    }
}

/// Session configuration for creating new sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub instance_id: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub remote_host: Option<String>,
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
            remote_host: None,
            aws_profile,
            region,
            priority: SessionPriority::default(),
            tags: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_remote_host(mut self, host: Option<String>) -> Self {
        self.remote_host = host;
        self
    }
    
    pub fn with_priority(mut self, priority: SessionPriority) -> Self {
        self.priority = priority;
        self
    }
}

/// Session event enumeration for monitoring
#[derive(Debug, Clone)]
pub enum SessionEvent {
    HealthDegraded(String),
    TimeoutPredicted(std::time::Duration),
    ActivityDetected,
    ConnectionLost,
    ProcessTerminated,
    HeartbeatFailed,
    NetworkActivityDetected,
    SessionIdle,
}
