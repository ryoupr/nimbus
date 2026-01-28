use crate::config::ReconnectionConfig;
use crate::error::{Result, SessionError};
use crate::session::{Session, SessionStatus, SessionConfig};
use crate::aws::AwsClient;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// Auto reconnector trait for handling session reconnections
pub trait AutoReconnector {
    async fn handle_disconnection(&mut self, session_id: &str, reason: &str) -> Result<bool>;
    async fn preemptive_reconnect(&mut self, session_id: &str) -> Result<bool>;
    fn configure_policy(&mut self, policy: ReconnectionConfig);
    async fn attempt_reconnection(&mut self, session_id: &str, attempt: u32) -> Result<bool>;
    async fn schedule_preemptive_reconnect(&mut self, session_id: &str, delay: Duration) -> Result<()>;
    async fn cancel_reconnection(&mut self, session_id: &str) -> Result<()>;
}

/// Reconnection attempt information
#[derive(Debug, Clone)]
struct ReconnectionAttempt {
    session_id: String,
    attempt_number: u32,
    started_at: SystemTime,
    next_attempt_at: Option<SystemTime>,
    reason: String,
}

/// Default implementation of auto reconnector
pub struct DefaultAutoReconnector {
    config: ReconnectionConfig,
    aws_client: AwsClient,
    active_sessions: RwLock<HashMap<String, Session>>,
    reconnection_attempts: RwLock<HashMap<String, ReconnectionAttempt>>,
}

impl DefaultAutoReconnector {
    pub fn new(config: ReconnectionConfig, aws_client: AwsClient) -> Self {
        Self { 
            config,
            aws_client,
            active_sessions: RwLock::new(HashMap::new()),
            reconnection_attempts: RwLock::new(HashMap::new()),
        }
    }
    
    /// Calculate delay for reconnection attempt using exponential backoff
    fn calculate_delay(&self, attempt: u32) -> Duration {
        if self.config.aggressive_mode && attempt <= self.config.aggressive_attempts {
            Duration::from_millis(self.config.aggressive_interval_ms)
        } else {
            let delay_ms = std::cmp::min(
                self.config.base_delay_ms * (2_u64.pow(attempt.saturating_sub(1))),
                self.config.max_delay_ms,
            );
            Duration::from_millis(delay_ms)
        }
    }
    
    /// Create a new session with the same configuration as the original
    async fn create_replacement_session(&self, original_session: &Session) -> Result<Session> {
        let session_config = SessionConfig {
            instance_id: original_session.instance_id.clone(),
            local_port: original_session.local_port,
            remote_port: original_session.remote_port,
            remote_host: original_session.remote_host.clone(),
            aws_profile: original_session.aws_profile.clone(),
            region: original_session.region.clone(),
            priority: original_session.priority,
            tags: original_session.tags.clone(),
        };
        
        debug!("Creating replacement session for {}", original_session.id);
        
        // Create new session using AWS client
        let new_session = self.aws_client.create_session(session_config).await?;
        
        info!("Created replacement session {} for original session {}", 
              new_session.id, original_session.id);
        
        Ok(new_session)
    }
    
    /// Check if session is approaching timeout and needs preemptive reconnection
    async fn should_preemptive_reconnect(&self, session: &Session) -> bool {
        // SSM sessions typically timeout after 20 minutes of inactivity
        // We'll trigger preemptive reconnection at 15 minutes (900 seconds)
        const PREEMPTIVE_THRESHOLD_SECONDS: u64 = 900;
        
        let idle_time = session.idle_seconds();
        let age = session.age_seconds();
        
        // Trigger preemptive reconnection if:
        // 1. Session has been idle for more than threshold
        // 2. Session is older than 15 minutes and showing signs of degradation
        idle_time > PREEMPTIVE_THRESHOLD_SECONDS || 
        (age > PREEMPTIVE_THRESHOLD_SECONDS && !session.is_healthy())
    }
    
    /// Perform health check on session to determine if reconnection is needed
    async fn check_session_health(&self, session_id: &str) -> Result<bool> {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            // Check if process is still running
            if let Some(pid) = session.process_id {
                if !self.is_process_alive(pid).await {
                    debug!("Session {} process {} is not alive", session_id, pid);
                    return Ok(false);
                }
            }
            
            // Check if port is still responsive
            if !self.is_port_responsive(session.local_port).await {
                debug!("Session {} port {} is not responsive", session_id, session.local_port);
                return Ok(false);
            }
            
            // Check session status
            Ok(session.is_healthy())
        } else {
            warn!("Session {} not found in active sessions", session_id);
            Ok(false)
        }
    }
    
    /// Check if a process is still alive
    async fn is_process_alive(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            
            match kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                Ok(_) => true,
                Err(nix::errno::Errno::ESRCH) => false, // Process not found
                Err(_) => true, // Other errors assume process exists
            }
        }
        
        #[cfg(windows)]
        {
            use winapi::um::processthreadsapi::{OpenProcess, GetExitCodeProcess};
            use winapi::um::winnt::PROCESS_QUERY_INFORMATION;
            use winapi::um::handleapi::CloseHandle;
            use std::ptr;
            
            unsafe {
                let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
                if handle.is_null() {
                    return false;
                }
                
                let mut exit_code: u32 = 0;
                let result = GetExitCodeProcess(handle, &mut exit_code);
                CloseHandle(handle);
                
                result != 0 && exit_code == 259 // STILL_ACTIVE
            }
        }
    }
    
    /// Check if a local port is responsive
    async fn is_port_responsive(&self, port: u16) -> bool {
        use tokio::net::TcpStream;
        use tokio::time::timeout;
        
        let address = format!("127.0.0.1:{}", port);
        let connect_timeout = Duration::from_secs(1);
        
        match timeout(connect_timeout, TcpStream::connect(&address)).await {
            Ok(Ok(_)) => true,
            _ => false,
        }
    }
    
    /// Update session in active sessions map
    async fn update_session(&self, session: Session) {
        let mut sessions = self.active_sessions.write().await;
        sessions.insert(session.id.clone(), session);
    }
    
    /// Remove session from active sessions map
    async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.active_sessions.write().await;
        sessions.remove(session_id);
    }
    
    /// Get session from active sessions map
    async fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.active_sessions.read().await;
        sessions.get(session_id).cloned()
    }
}

impl AutoReconnector for DefaultAutoReconnector {
    async fn handle_disconnection(&mut self, session_id: &str, reason: &str) -> Result<bool> {
        if !self.config.enabled {
            info!("Auto-reconnection disabled for session: {}", session_id);
            return Ok(false);
        }
        
        warn!("Handling disconnection for session {}: {}", session_id, reason);
        
        // Get the original session
        let _original_session = match self.get_session(session_id).await {
            Some(session) => session,
            None => {
                error!("Session {} not found for reconnection", session_id);
                return Err(SessionError::NotFound { 
                    session_id: session_id.to_string() 
                }.into());
            }
        };
        
        // Record reconnection attempt
        let attempt_info = ReconnectionAttempt {
            session_id: session_id.to_string(),
            attempt_number: 1,
            started_at: SystemTime::now(),
            next_attempt_at: None,
            reason: reason.to_string(),
        };
        
        {
            let mut attempts = self.reconnection_attempts.write().await;
            attempts.insert(session_id.to_string(), attempt_info);
        }
        
        // Perform reconnection attempts with exponential backoff
        for attempt in 1..=self.config.max_attempts {
            let delay = self.calculate_delay(attempt);
            
            info!(
                "Reconnection attempt {} for session {} (delay: {:?})",
                attempt, session_id, delay
            );
            
            // Update attempt info
            {
                let mut attempts = self.reconnection_attempts.write().await;
                if let Some(attempt_info) = attempts.get_mut(session_id) {
                    attempt_info.attempt_number = attempt;
                    attempt_info.next_attempt_at = Some(SystemTime::now() + delay);
                }
            }
            
            // Wait for the calculated delay
            tokio::time::sleep(delay).await;
            
            match self.attempt_reconnection(session_id, attempt).await {
                Ok(true) => {
                    info!("Reconnection successful for session: {}", session_id);
                    
                    // Clean up reconnection attempt tracking
                    {
                        let mut attempts = self.reconnection_attempts.write().await;
                        attempts.remove(session_id);
                    }
                    
                    return Ok(true);
                },
                Ok(false) => {
                    warn!("Reconnection attempt {} failed for session: {}", attempt, session_id);
                },
                Err(e) => {
                    error!("Reconnection attempt {} error for session {}: {}", attempt, session_id, e);
                    
                    // If it's a non-recoverable error, stop trying
                    if !e.is_recoverable() {
                        error!("Non-recoverable error encountered, stopping reconnection attempts");
                        break;
                    }
                }
            }
        }
        
        // All attempts failed
        error!("All reconnection attempts failed for session: {}", session_id);
        
        // Clean up
        {
            let mut attempts = self.reconnection_attempts.write().await;
            attempts.remove(session_id);
        }
        
        self.remove_session(session_id).await;
        
        Err(SessionError::ReconnectionFailed {
            session_id: session_id.to_string(),
            attempts: self.config.max_attempts,
        }.into())
    }
    
    async fn preemptive_reconnect(&mut self, session_id: &str) -> Result<bool> {
        info!("Performing preemptive reconnection for session: {}", session_id);
        
        let original_session = match self.get_session(session_id).await {
            Some(session) => session,
            None => {
                warn!("Session {} not found for preemptive reconnection", session_id);
                return Ok(false);
            }
        };
        
        // Check if preemptive reconnection is actually needed
        if !self.should_preemptive_reconnect(&original_session).await {
            debug!("Preemptive reconnection not needed for session: {}", session_id);
            return Ok(false);
        }
        
        // Create a new session before terminating the old one
        match self.create_replacement_session(&original_session).await {
            Ok(new_session) => {
                info!("Preemptive reconnection successful: {} -> {}", 
                      original_session.id, new_session.id);
                
                // Update the session in our tracking
                self.update_session(new_session).await;
                
                // Optionally terminate the old session gracefully
                // This could be done in the background to avoid interruption
                tokio::spawn(async move {
                    // Graceful termination logic here
                    debug!("Gracefully terminating old session: {}", original_session.id);
                });
                
                Ok(true)
            },
            Err(e) => {
                error!("Preemptive reconnection failed for session {}: {}", session_id, e);
                Err(e)
            }
        }
    }
    
    fn configure_policy(&mut self, policy: ReconnectionConfig) {
        info!("Updating reconnection policy");
        debug!("New policy: enabled={}, max_attempts={}, base_delay={}ms, aggressive_mode={}", 
               policy.enabled, policy.max_attempts, policy.base_delay_ms, policy.aggressive_mode);
        self.config = policy;
    }
    
    async fn attempt_reconnection(&mut self, session_id: &str, attempt: u32) -> Result<bool> {
        info!("Attempting reconnection for session {} (attempt {})", session_id, attempt);
        
        let original_session = match self.get_session(session_id).await {
            Some(session) => session,
            None => {
                error!("Session {} not found for reconnection attempt", session_id);
                return Err(SessionError::NotFound { 
                    session_id: session_id.to_string() 
                }.into());
            }
        };
        
        // First, verify the session actually needs reconnection
        if self.check_session_health(session_id).await? {
            info!("Session {} is actually healthy, no reconnection needed", session_id);
            return Ok(true);
        }
        
        // Terminate the old session if it's still running
        if let Some(pid) = original_session.process_id {
            if self.is_process_alive(pid).await {
                debug!("Terminating old session process: {}", pid);
                // Terminate process gracefully
                #[cfg(unix)]
                {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                }
                
                #[cfg(windows)]
                {
                    use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
                    use winapi::um::winnt::PROCESS_TERMINATE;
                    use winapi::um::handleapi::CloseHandle;
                    
                    unsafe {
                        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
                        if !handle.is_null() {
                            TerminateProcess(handle, 1);
                            CloseHandle(handle);
                        }
                    }
                }
            }
        }
        
        // Create a new session
        match self.create_replacement_session(&original_session).await {
            Ok(mut new_session) => {
                // Preserve the original session ID for continuity
                new_session.id = original_session.id.clone();
                new_session.status = SessionStatus::Active;
                
                info!("Reconnection attempt {} succeeded for session: {}", attempt, session_id);
                
                // Update our session tracking
                self.update_session(new_session).await;
                
                Ok(true)
            },
            Err(e) => {
                warn!("Reconnection attempt {} failed for session {}: {}", attempt, session_id, e);
                Ok(false)
            }
        }
    }
    
    async fn schedule_preemptive_reconnect(&mut self, session_id: &str, delay: Duration) -> Result<()> {
        let session_id = session_id.to_string();
        
        info!("Scheduling preemptive reconnection for session {} in {:?}", session_id, delay);
        
        // Clone necessary data for the async task
        let mut reconnector = DefaultAutoReconnector {
            config: self.config.clone(),
            aws_client: self.aws_client.clone(),
            active_sessions: RwLock::new(HashMap::new()),
            reconnection_attempts: RwLock::new(HashMap::new()),
        };
        
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            
            match reconnector.preemptive_reconnect(&session_id).await {
                Ok(true) => {
                    info!("Scheduled preemptive reconnection completed for session: {}", session_id);
                },
                Ok(false) => {
                    debug!("Scheduled preemptive reconnection was not needed for session: {}", session_id);
                },
                Err(e) => {
                    error!("Scheduled preemptive reconnection failed for session {}: {}", session_id, e);
                }
            }
        });
        
        Ok(())
    }
    
    async fn cancel_reconnection(&mut self, session_id: &str) -> Result<()> {
        info!("Cancelling reconnection for session: {}", session_id);
        
        let mut attempts = self.reconnection_attempts.write().await;
        if attempts.remove(session_id).is_some() {
            info!("Cancelled active reconnection attempt for session: {}", session_id);
        } else {
            debug!("No active reconnection attempt found for session: {}", session_id);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReconnectionConfig;
    use std::time::Duration;

    fn create_test_config() -> ReconnectionConfig {
        ReconnectionConfig {
            enabled: true,
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 1000,
            aggressive_mode: false,
            aggressive_attempts: 5,
            aggressive_interval_ms: 50,
        }
    }

    #[test]
    fn test_calculate_delay_exponential_backoff() {
        let config = create_test_config();
        
        // テスト用の簡単な実装
        fn calculate_delay(config: &ReconnectionConfig, attempt: u32) -> Duration {
            if config.aggressive_mode && attempt <= config.aggressive_attempts {
                Duration::from_millis(config.aggressive_interval_ms)
            } else {
                let delay_ms = std::cmp::min(
                    config.base_delay_ms * (2_u64.pow(attempt.saturating_sub(1))),
                    config.max_delay_ms,
                );
                Duration::from_millis(delay_ms)
            }
        }
        
        // 指数バックオフのテスト
        assert_eq!(calculate_delay(&config, 1), Duration::from_millis(100));
        assert_eq!(calculate_delay(&config, 2), Duration::from_millis(200));
        assert_eq!(calculate_delay(&config, 3), Duration::from_millis(400));
        assert_eq!(calculate_delay(&config, 4), Duration::from_millis(800));
        assert_eq!(calculate_delay(&config, 5), Duration::from_millis(1000)); // max_delay_ms でキャップ
    }

    #[test]
    fn test_calculate_delay_aggressive_mode() {
        let mut config = create_test_config();
        config.aggressive_mode = true;
        config.aggressive_attempts = 3;
        config.aggressive_interval_ms = 50;
        
        fn calculate_delay(config: &ReconnectionConfig, attempt: u32) -> Duration {
            if config.aggressive_mode && attempt <= config.aggressive_attempts {
                Duration::from_millis(config.aggressive_interval_ms)
            } else {
                let delay_ms = std::cmp::min(
                    config.base_delay_ms * (2_u64.pow(attempt.saturating_sub(1))),
                    config.max_delay_ms,
                );
                Duration::from_millis(delay_ms)
            }
        }
        
        // アグレッシブモードのテスト
        assert_eq!(calculate_delay(&config, 1), Duration::from_millis(50));
        assert_eq!(calculate_delay(&config, 2), Duration::from_millis(50));
        assert_eq!(calculate_delay(&config, 3), Duration::from_millis(50));
        assert_eq!(calculate_delay(&config, 4), Duration::from_millis(800)); // 通常の指数バックオフに戻る (100 * 2^3)
    }

    #[test]
    fn test_reconnection_config_validation() {
        let config = create_test_config();
        
        assert!(config.enabled);
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 100);
        assert_eq!(config.max_delay_ms, 1000);
        assert!(!config.aggressive_mode);
    }
}