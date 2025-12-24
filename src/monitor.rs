use crate::session::{Session, SessionEvent, SessionHealth as SessionHealthData};
use crate::error::{Result, SessionError};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time;
use tracing::{info, warn, error, debug};
use sysinfo::{System, Pid};
use std::process::Command;

/// Session health information for monitoring
#[derive(Debug, Clone)]
pub struct SessionHealth {
    pub is_healthy: bool,
    pub last_activity: Instant,
    pub connection_count: u32,
    pub data_transferred: u64,
    pub response_time_ms: Option<u64>,
    pub process_alive: bool,
    pub port_responsive: bool,
    pub network_activity: bool,
    pub heartbeat_success: bool,
    pub last_heartbeat: Option<Instant>,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
}

impl SessionHealth {
    pub fn new() -> Self {
        Self {
            is_healthy: false,
            last_activity: Instant::now(),
            connection_count: 0,
            data_transferred: 0,
            response_time_ms: None,
            process_alive: false,
            port_responsive: false,
            network_activity: false,
            heartbeat_success: false,
            last_heartbeat: None,
            network_bytes_sent: 0,
            network_bytes_received: 0,
        }
    }
    
    pub fn from_session(session: &Session) -> Self {
        let idle_duration = Duration::from_secs(session.idle_seconds());
        let now = Instant::now();
        let last_activity = if idle_duration <= now.elapsed() {
            now - idle_duration
        } else {
            now
        };
        
        Self {
            is_healthy: session.is_healthy(),
            last_activity,
            connection_count: session.connection_count,
            data_transferred: session.data_transferred,
            response_time_ms: None,
            process_alive: session.process_id.is_some(),
            port_responsive: false,
            network_activity: session.idle_seconds() < 30,
            heartbeat_success: false,
            last_heartbeat: None,
            network_bytes_sent: 0,
            network_bytes_received: 0,
        }
    }
    
    /// Update health status based on checks
    pub fn update_health_status(&mut self) {
        // セッションが健全と判定される条件:
        // 1. プロセスが生きている
        // 2. ポートが応答している
        // 3. 最近のハートビートが成功している
        // 4. ネットワーク活動があるか、最後のアクティビティが30秒以内
        self.is_healthy = self.process_alive && 
                         self.port_responsive && 
                         self.heartbeat_success &&
                         (self.network_activity || self.last_activity.elapsed() < Duration::from_secs(30));
    }
    
    /// Check if session requires attention (degraded but not completely unhealthy)
    pub fn requires_attention(&self) -> bool {
        !self.is_healthy && (self.process_alive || self.port_responsive)
    }
}

/// Session monitoring events
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    HealthDegraded(String),
    TimeoutPredicted(Duration),
    ActivityDetected,
    ConnectionLost,
    ProcessTerminated,
    HeartbeatFailed,
    HeartbeatSuccess,
    NetworkActivityDetected,
    NetworkActivityStopped,
    SessionIdle,
    SessionActive,
    PortUnresponsive,
    PortResponsive,
    HighLatency(u64), // milliseconds
    ResourceUsageHigh,
}

impl MonitorEvent {
    /// Convert SessionEvent to MonitorEvent
    pub fn from_session_event(event: &SessionEvent) -> Self {
        match event {
            SessionEvent::HealthDegraded(msg) => MonitorEvent::HealthDegraded(msg.clone()),
            SessionEvent::TimeoutPredicted(duration) => MonitorEvent::TimeoutPredicted(*duration),
            SessionEvent::ActivityDetected => MonitorEvent::ActivityDetected,
            SessionEvent::ConnectionLost => MonitorEvent::ConnectionLost,
            SessionEvent::ProcessTerminated => MonitorEvent::ProcessTerminated,
            SessionEvent::HeartbeatFailed => MonitorEvent::HeartbeatFailed,
            SessionEvent::NetworkActivityDetected => MonitorEvent::NetworkActivityDetected,
            SessionEvent::SessionIdle => MonitorEvent::SessionIdle,
        }
    }
}

/// Session monitor trait for monitoring session health
#[async_trait::async_trait]
pub trait SessionMonitor: Send + Sync {
    /// Start monitoring a session with continuous health checks
    /// 要件 1.1: セッション監視機能はセッションの健全性を継続的に監視する
    async fn start_monitoring(&mut self, session_id: &str) -> Result<()>;
    
    /// Stop monitoring a session
    async fn stop_monitoring(&mut self, session_id: &str) -> Result<()>;
    
    /// Check current session health status
    async fn check_session_health(&self, session_id: &str) -> Result<SessionHealth>;
    
    /// Predict when session might timeout
    async fn predict_timeout(&self, session_id: &str) -> Result<Option<Duration>>;
    
    /// Register callback for monitoring events
    /// 要件 1.5: ユーザーが明示的に切断を要求していない間、アクティブなセッション状態を維持する
    fn register_callback(&mut self, callback: Box<dyn Fn(SessionEvent) + Send + Sync>);
    
    /// Check if a session is currently being monitored
    async fn is_monitoring(&self, session_id: &str) -> bool;
    
    /// Get list of all monitored sessions
    async fn get_monitored_sessions(&self) -> Vec<String>;
    
    /// Perform immediate heartbeat check
    async fn perform_heartbeat(&self, session_id: &str) -> Result<bool>;
    
    /// Get detailed network activity information
    async fn get_network_activity(&self, session_id: &str) -> Result<NetworkActivity>;
    
    /// Force health check update
    async fn force_health_check(&self, session_id: &str) -> Result<SessionHealth>;
}

/// Network activity information
#[derive(Debug, Clone)]
pub struct NetworkActivity {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connections_active: u32,
    pub last_activity: Instant,
    pub activity_detected: bool,
}

/// Default implementation of session monitor
pub struct DefaultSessionMonitor {
    monitoring_interval: Duration,
    heartbeat_interval: Duration,
    callbacks: Vec<Box<dyn Fn(SessionEvent) + Send + Sync>>,
    active_monitors: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    system: Arc<Mutex<System>>,
    session_timeouts: Arc<RwLock<HashMap<String, SystemTime>>>,
    network_stats: Arc<RwLock<HashMap<String, NetworkActivity>>>,
    event_sender: Arc<Mutex<Option<mpsc::UnboundedSender<MonitorEvent>>>>,
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<MonitorEvent>>>>,
}

impl DefaultSessionMonitor {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            monitoring_interval: Duration::from_secs(5), // 5秒間隔での監視
            heartbeat_interval: Duration::from_secs(5),   // 5秒間隔でのハートビート
            callbacks: Vec::new(),
            active_monitors: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            system: Arc::new(Mutex::new(System::new_all())),
            session_timeouts: Arc::new(RwLock::new(HashMap::new())),
            network_stats: Arc::new(RwLock::new(HashMap::new())),
            event_sender: Arc::new(Mutex::new(Some(tx))),
            event_receiver: Arc::new(Mutex::new(Some(rx))),
        }
    }
    
    pub fn with_intervals(monitoring_interval: Duration, heartbeat_interval: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            monitoring_interval,
            heartbeat_interval,
            callbacks: Vec::new(),
            active_monitors: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            system: Arc::new(Mutex::new(System::new_all())),
            session_timeouts: Arc::new(RwLock::new(HashMap::new())),
            network_stats: Arc::new(RwLock::new(HashMap::new())),
            event_sender: Arc::new(Mutex::new(Some(tx))),
            event_receiver: Arc::new(Mutex::new(Some(rx))),
        }
    }
    
    /// Add or update session information
    pub async fn add_session(&self, session: Session) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
    }
    
    /// Remove session information
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        
        let mut timeouts = self.session_timeouts.write().await;
        timeouts.remove(session_id);
    }
    
    /// Check if a port is open and responsive with latency measurement
    async fn check_port_health(&self, port: u16) -> (bool, Option<u64>) {
        debug!("Checking port health for port: {}", port);
        
        let start = Instant::now();
        match tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
        ).await {
            Ok(Ok(_)) => {
                let latency = start.elapsed().as_millis() as u64;
                debug!("Port {} is responsive, latency: {}ms", port, latency);
                (true, Some(latency))
            },
            Ok(Err(e)) => {
                debug!("Port {} connection failed: {}", port, e);
                (false, None)
            },
            Err(_) => {
                debug!("Port {} check timed out", port);
                (false, None)
            }
        }
    }
    
    /// Check if SSM process is running with detailed process information
    async fn check_process_health(&self, process_id: Option<u32>) -> (bool, Option<f32>) {
        if let Some(pid) = process_id {
            debug!("Checking process health for PID: {}", pid);
            
            let mut system = self.system.lock().await;
            system.refresh_processes();
            
            if let Some(process) = system.process(Pid::from(pid as usize)) {
                let cpu_usage = process.cpu_usage();
                debug!("Process {} exists, CPU usage: {}%", pid, cpu_usage);
                (true, Some(cpu_usage))
            } else {
                debug!("Process {} not found", pid);
                (false, None)
            }
        } else {
            debug!("No process ID provided");
            (false, None)
        }
    }
    
    /// Enhanced network activity monitoring with detailed statistics
    async fn check_network_activity(&self, session: &Session) -> Result<NetworkActivity> {
        debug!("Checking network activity for session: {}", session.id);
        
        // システムのネットワーク統計を取得
        let mut system = self.system.lock().await;
        system.refresh_memory();
        
        let total_bytes_sent = 0;
        let total_bytes_received = 0;
        
        // ネットワーク統計は現在のsysinfoバージョンでは利用できないため、
        // 代替手段として基本的な統計を使用
        debug!("Network monitoring temporarily disabled due to API changes");
        
        // ポート固有の接続数をチェック
        let active_connections = self.count_active_connections(session.local_port).await;
        
        let idle_time = session.idle_seconds();
        let activity_detected = idle_time < 30 || active_connections > 0;
        
        let network_activity = NetworkActivity {
            bytes_sent: total_bytes_sent,
            bytes_received: total_bytes_received,
            connections_active: active_connections,
            last_activity: Instant::now() - Duration::from_secs(idle_time),
            activity_detected,
        };
        
        // ネットワーク統計を更新
        let mut stats = self.network_stats.write().await;
        stats.insert(session.id.clone(), network_activity.clone());
        
        debug!("Network activity for session {}: sent={}, received={}, connections={}, activity={}", 
               session.id, total_bytes_sent, total_bytes_received, active_connections, activity_detected);
        
        Ok(network_activity)
    }
    
    /// Count active connections on a specific port
    async fn count_active_connections(&self, port: u16) -> u32 {
        // プラットフォーム固有の実装
        #[cfg(target_os = "windows")]
        {
            self.count_connections_windows(port).await
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.count_connections_unix(port).await
        }
    }
    
    #[cfg(target_os = "windows")]
    async fn count_connections_windows(&self, port: u16) -> u32 {
        match Command::new("netstat")
            .args(&["-an", "-p", "tcp"])
            .output()
        {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                let port_str = format!(":{}", port);
                output_str.lines()
                    .filter(|line| line.contains(&port_str) && line.contains("ESTABLISHED"))
                    .count() as u32
            },
            Err(e) => {
                debug!("Failed to run netstat: {}", e);
                0
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    async fn count_connections_unix(&self, port: u16) -> u32 {
        match Command::new("ss")
            .args(&["-tn", "state", "established"])
            .output()
        {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                let port_str = format!(":{}", port);
                output_str.lines()
                    .filter(|line| line.contains(&port_str))
                    .count() as u32
            },
            Err(_) => {
                // Fallback to netstat if ss is not available
                match Command::new("netstat")
                    .args(&["-tn"])
                    .output()
                {
                    Ok(output) => {
                        let output_str = String::from_utf8_lossy(&output.stdout);
                        let port_str = format!(":{}", port);
                        output_str.lines()
                            .filter(|line| line.contains(&port_str) && line.contains("ESTABLISHED"))
                            .count() as u32
                    },
                    Err(e) => {
                        debug!("Failed to run network commands: {}", e);
                        0
                    }
                }
            }
        }
    }
    
    /// Perform comprehensive heartbeat check on session
    async fn perform_heartbeat_check(&self, session: &Session) -> Result<(bool, SessionHealth)> {
        debug!("Performing comprehensive heartbeat for session: {}", session.id);
        
        let start_time = Instant::now();
        
        // ポートの応答性とレイテンシをチェック
        let (port_responsive, response_time) = self.check_port_health(session.local_port).await;
        
        // プロセスの生存確認とCPU使用率
        let (process_alive, _cpu_usage) = self.check_process_health(session.process_id).await;
        
        // ネットワークアクティビティをチェック
        let network_activity = self.check_network_activity(session).await?;
        
        let heartbeat_duration = start_time.elapsed();
        let heartbeat_success = port_responsive && process_alive;
        
        let mut health = SessionHealth::from_session(session);
        health.port_responsive = port_responsive;
        health.process_alive = process_alive;
        health.network_activity = network_activity.activity_detected;
        health.response_time_ms = response_time;
        health.heartbeat_success = heartbeat_success;
        health.last_heartbeat = Some(Instant::now());
        health.network_bytes_sent = network_activity.bytes_sent;
        health.network_bytes_received = network_activity.bytes_received;
        health.connection_count = network_activity.connections_active;
        
        // 健全性ステータスを更新
        health.update_health_status();
        
        debug!("Heartbeat for session {} completed in {:?}: success={}, port_responsive={}, process_alive={}, network_activity={}", 
               session.id, heartbeat_duration, heartbeat_success, port_responsive, process_alive, network_activity.activity_detected);
        
        Ok((heartbeat_success, health))
    }
    
    /// Emit event to all registered callbacks
    async fn emit_event(&self, event: SessionEvent) {
        debug!("Emitting event: {:?}", event);
        for callback in &self.callbacks {
            callback(event.clone());
        }
        
        // 内部イベントチャネルにも送信
        if let Some(sender) = self.event_sender.lock().await.as_ref() {
            if let Err(e) = sender.send(MonitorEvent::from_session_event(&event)) {
                debug!("Failed to send internal event: {}", e);
            }
        }
    }
    
    /// Monitor session health in background with enhanced monitoring
    async fn monitor_session_health(&self, session_id: String) {
        info!("Starting enhanced health monitoring for session: {}", session_id);
        
        let mut monitoring_interval = time::interval(self.monitoring_interval);
        let mut heartbeat_interval = time::interval(self.heartbeat_interval);
        
        // 前回の状態を追跡
        let mut last_health_status = false;
        let mut last_network_activity = false;
        let mut consecutive_failures = 0;
        
        loop {
            tokio::select! {
                _ = monitoring_interval.tick() => {
                    // 定期的な健全性チェック
                    match self.check_session_health(&session_id).await {
                        Ok(health) => {
                            // 健全性状態の変化を検出
                            if health.is_healthy != last_health_status {
                                if health.is_healthy {
                                    info!("Session health recovered: {}", session_id);
                                    self.emit_event(SessionEvent::ActivityDetected).await;
                                    consecutive_failures = 0;
                                } else {
                                    warn!("Session health degraded: {}", session_id);
                                    self.emit_event(SessionEvent::HealthDegraded(session_id.clone())).await;
                                    consecutive_failures += 1;
                                }
                                last_health_status = health.is_healthy;
                            }
                            
                            // ネットワークアクティビティの変化を検出
                            if health.network_activity != last_network_activity {
                                if health.network_activity {
                                    debug!("Network activity detected for session: {}", session_id);
                                    self.emit_event(SessionEvent::ActivityDetected).await;
                                } else {
                                    debug!("Network activity stopped for session: {}", session_id);
                                }
                                last_network_activity = health.network_activity;
                            }
                            
                            // アイドル状態のチェック
                            if health.last_activity.elapsed() > Duration::from_secs(300) {
                                if health.last_activity.elapsed() > Duration::from_secs(600) {
                                    // 10分以上アイドル
                                    warn!("Session has been idle for over 10 minutes: {}", session_id);
                                }
                                self.emit_event(SessionEvent::SessionIdle).await;
                            }
                            
                            // 高レイテンシの検出
                            if let Some(latency) = health.response_time_ms {
                                if latency > 200 {
                                    warn!("High latency detected: {}ms for session: {}", latency, session_id);
                                }
                            }
                            
                            // 連続失敗の処理
                            if consecutive_failures >= 3 {
                                error!("Session has failed health checks {} times consecutively: {}", consecutive_failures, session_id);
                                self.emit_event(SessionEvent::ConnectionLost).await;
                            }
                        },
                        Err(e) => {
                            error!("Failed to check session health: {}", e);
                            consecutive_failures += 1;
                            if consecutive_failures >= 5 {
                                error!("Too many consecutive health check failures, stopping monitoring: {}", session_id);
                                self.emit_event(SessionEvent::ConnectionLost).await;
                                break;
                            }
                        }
                    }
                    
                    // タイムアウト予測
                    if let Ok(Some(timeout)) = self.predict_timeout(&session_id).await {
                        if timeout < Duration::from_secs(60) { // 1分未満でタイムアウト予測
                            warn!("Session timeout predicted in {:?}: {}", timeout, session_id);
                            self.emit_event(SessionEvent::TimeoutPredicted(timeout)).await;
                        }
                    }
                },
                _ = heartbeat_interval.tick() => {
                    // 精密なハートビートチェック
                    let sessions = self.sessions.read().await;
                    if let Some(session) = sessions.get(&session_id) {
                        match self.perform_heartbeat_check(session).await {
                            Ok((success, health)) => {
                                if success {
                                    debug!("Heartbeat successful for session: {}", session_id);
                                    consecutive_failures = 0;
                                } else {
                                    warn!("Heartbeat failed for session: {}", session_id);
                                    consecutive_failures += 1;
                                    self.emit_event(SessionEvent::HeartbeatFailed).await;
                                    
                                    // プロセス終了の検出
                                    if !health.process_alive {
                                        error!("Process terminated for session: {}", session_id);
                                        self.emit_event(SessionEvent::ProcessTerminated).await;
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Heartbeat check error for session {}: {}", session_id, e);
                                consecutive_failures += 1;
                                self.emit_event(SessionEvent::HeartbeatFailed).await;
                            }
                        }
                    } else {
                        // セッションが見つからない場合は監視を停止
                        warn!("Session not found, stopping monitoring: {}", session_id);
                        break;
                    }
                }
            }
        }
        
        info!("Stopped monitoring session: {}", session_id);
    }
}

#[async_trait::async_trait]
impl SessionMonitor for DefaultSessionMonitor {
    async fn start_monitoring(&mut self, session_id: &str) -> Result<()> {
        let mut active_monitors = self.active_monitors.lock().await;
        
        if active_monitors.contains_key(session_id) {
            debug!("Already monitoring session: {}", session_id);
            return Ok(()); // Already monitoring
        }
        
        let session_id_clone = session_id.to_string();
        let sessions = Arc::clone(&self.sessions);
        let system = Arc::clone(&self.system);
        let monitoring_interval = self.monitoring_interval;
        let heartbeat_interval = self.heartbeat_interval;
        
        // Create a simplified monitor for the background task
        let monitor = SimpleMonitor {
            sessions,
            system,
            monitoring_interval,
            heartbeat_interval,
        };
        
        let monitor_task = tokio::spawn(async move {
            monitor.monitor_session_health(session_id_clone).await;
        });
        
        active_monitors.insert(session_id.to_string(), monitor_task);
        info!("Started monitoring session: {}", session_id);
        
        Ok(())
    }
    
    async fn stop_monitoring(&mut self, session_id: &str) -> Result<()> {
        let mut active_monitors = self.active_monitors.lock().await;
        
        if let Some(handle) = active_monitors.remove(session_id) {
            handle.abort();
            info!("Stopped monitoring session: {}", session_id);
        } else {
            debug!("Session was not being monitored: {}", session_id);
        }
        
        Ok(())
    }
    
    async fn check_session_health(&self, session_id: &str) -> Result<SessionHealth> {
        debug!("Checking health for session: {}", session_id);
        
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| SessionError::NotFound { 
                session_id: session_id.to_string() 
            })?;
        
        let mut health = SessionHealth::from_session(session);
        
        // ポートの応答性をチェック
        let (port_responsive, response_time) = self.check_port_health(session.local_port).await;
        health.port_responsive = port_responsive;
        health.response_time_ms = response_time;
        
        // プロセスの生存確認
        let (process_alive, _cpu_usage) = self.check_process_health(session.process_id).await;
        health.process_alive = process_alive;
        
        // ネットワークアクティビティをチェック
        let network_activity = self.check_network_activity(session).await?;
        health.network_activity = network_activity.activity_detected;
        health.connection_count = network_activity.connections_active;
        health.network_bytes_sent = network_activity.bytes_sent;
        health.network_bytes_received = network_activity.bytes_received;
        
        // 健全性ステータスを更新
        health.update_health_status();
        
        debug!("Session {} health: healthy={}, port_responsive={}, process_alive={}, network_activity={}", 
               session_id, health.is_healthy, health.port_responsive, health.process_alive, health.network_activity);
        
        Ok(health)
    }
    
    async fn predict_timeout(&self, session_id: &str) -> Result<Option<Duration>> {
        debug!("Predicting timeout for session: {}", session_id);
        
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| SessionError::NotFound { 
                session_id: session_id.to_string() 
            })?;
        
        // SSMセッションの標準的なタイムアウトは20分（1200秒）
        const SSM_SESSION_TIMEOUT: Duration = Duration::from_secs(1200);
        
        let session_age = Duration::from_secs(session.age_seconds());
        
        if session_age >= SSM_SESSION_TIMEOUT {
            // 既にタイムアウト時間を超過
            Ok(Some(Duration::from_secs(0)))
        } else {
            let remaining = SSM_SESSION_TIMEOUT - session_age;
            
            // 5分以内にタイムアウトする場合のみ予測として返す
            if remaining <= Duration::from_secs(300) {
                debug!("Session {} will timeout in {:?}", session_id, remaining);
                Ok(Some(remaining))
            } else {
                Ok(None)
            }
        }
    }
    
    fn register_callback(&mut self, callback: Box<dyn Fn(SessionEvent) + Send + Sync>) {
        self.callbacks.push(callback);
        debug!("Registered new callback, total callbacks: {}", self.callbacks.len());
    }
    
    async fn is_monitoring(&self, session_id: &str) -> bool {
        let active_monitors = self.active_monitors.lock().await;
        active_monitors.contains_key(session_id)
    }
    
    async fn get_monitored_sessions(&self) -> Vec<String> {
        let active_monitors = self.active_monitors.lock().await;
        active_monitors.keys().cloned().collect()
    }
    
    async fn perform_heartbeat(&self, session_id: &str) -> Result<bool> {
        debug!("Performing immediate heartbeat for session: {}", session_id);
        
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| SessionError::NotFound { 
                session_id: session_id.to_string() 
            })?;
        
        let (success, _health) = self.perform_heartbeat_check(session).await?;
        Ok(success)
    }
    
    async fn get_network_activity(&self, session_id: &str) -> Result<NetworkActivity> {
        debug!("Getting network activity for session: {}", session_id);
        
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| SessionError::NotFound { 
                session_id: session_id.to_string() 
            })?;
        
        self.check_network_activity(session).await
    }
    
    async fn force_health_check(&self, session_id: &str) -> Result<SessionHealth> {
        debug!("Forcing health check for session: {}", session_id);
        
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| SessionError::NotFound { 
                session_id: session_id.to_string() 
            })?;
        
        let (_success, health) = self.perform_heartbeat_check(session).await?;
        Ok(health)
    }
}

/// Simplified monitor for background tasks
struct SimpleMonitor {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    system: Arc<Mutex<System>>,
    monitoring_interval: Duration,
    heartbeat_interval: Duration,
}

impl SimpleMonitor {
    async fn monitor_session_health(&self, session_id: String) {
        info!("Starting background health monitoring for session: {}", session_id);
        
        let mut monitoring_interval = time::interval(self.monitoring_interval);
        let mut heartbeat_interval = time::interval(self.heartbeat_interval);
        
        loop {
            tokio::select! {
                _ = monitoring_interval.tick() => {
                    // 定期的な健全性チェック
                    if let Err(e) = self.perform_health_check(&session_id).await {
                        error!("Health check failed for session {}: {}", session_id, e);
                    }
                },
                _ = heartbeat_interval.tick() => {
                    // ハートビートチェック
                    if let Err(e) = self.perform_heartbeat_check(&session_id).await {
                        error!("Heartbeat check failed for session {}: {}", session_id, e);
                    }
                }
            }
        }
    }
    
    async fn perform_health_check(&self, session_id: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            debug!("Performing health check for session: {}", session_id);
            
            // 基本的な健全性チェックロジック
            if session.idle_seconds() > 300 { // 5分以上アイドル
                warn!("Session {} has been idle for {} seconds", session_id, session.idle_seconds());
            }
        }
        Ok(())
    }
    
    async fn perform_heartbeat_check(&self, session_id: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            debug!("Performing heartbeat check for session: {}", session_id);
            
            // プロセスの生存確認
            if let Some(pid) = session.process_id {
                let mut system = self.system.lock().await;
                system.refresh_processes();
                
                if system.process(Pid::from(pid as usize)).is_none() {
                    warn!("Process {} for session {} is no longer running", pid, session_id);
                }
            }
        }
        Ok(())
    }
}