use crate::error::Result;
use std::time::{Duration, Instant};
use std::process::Command;
use std::net::{TcpStream, SocketAddr};
use std::str::FromStr;
use sysinfo::{System, Pid};
use tracing::{info, warn, error, debug};
use async_trait::async_trait;

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub is_healthy: bool,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
    pub details: Option<String>,
}

/// Health checker trait for monitoring system and session health
#[async_trait]
pub trait HealthChecker {
    /// Check SSM session health
    async fn check_ssm_session(&self, session_id: &str) -> Result<HealthCheckResult>;
    
    /// Check network connectivity
    async fn check_network_connectivity(&self, target: &str) -> Result<HealthCheckResult>;
    
    /// Check resource availability
    async fn check_resource_availability(&self) -> Result<ResourceAvailability>;
    
    /// Perform comprehensive health check
    async fn comprehensive_health_check(&self, session_id: &str) -> Result<ComprehensiveHealthResult>;
    
    /// Send early warning notification
    async fn send_early_warning(&self, session_id: &str, warning: &str) -> Result<()>;
}

/// Default implementation of health checker
pub struct DefaultHealthChecker {
    check_interval: Duration,
    system: System,
    warning_threshold_ms: u64,
    error_threshold_ms: u64,
}

impl DefaultHealthChecker {
    pub fn new(check_interval: Duration) -> Self {
        Self {
            check_interval,
            system: System::new_all(),
            warning_threshold_ms: 1000,  // 1 second warning threshold
            error_threshold_ms: 5000,    // 5 second error threshold
        }
    }
    
    pub fn with_thresholds(check_interval: Duration, warning_ms: u64, error_ms: u64) -> Self {
        Self {
            check_interval,
            system: System::new_all(),
            warning_threshold_ms: warning_ms,
            error_threshold_ms: error_ms,
        }
    }
    
    /// Check if a process is running by name
    fn is_process_running(&mut self, process_name: &str) -> bool {
        self.system.refresh_processes();
        self.system.processes().values().any(|process| {
            process.name().to_string().to_lowercase().contains(&process_name.to_lowercase())
        })
    }
    
    /// Check if a process is running by PID
    fn is_process_running_by_pid(&mut self, pid: u32) -> bool {
        self.system.refresh_processes();
        self.system.process(Pid::from(pid as usize)).is_some()
    }
    
    /// Test TCP connectivity to a host:port
    async fn test_tcp_connectivity(&self, host: &str, port: u16, timeout: Duration) -> Result<bool> {
        let addr = format!("{}:{}", host, port);
        let socket_addr = match SocketAddr::from_str(&addr) {
            Ok(addr) => addr,
            Err(_) => {
                // Try to resolve hostname
                let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&addr).await?.collect();
                if addrs.is_empty() {
                    return Ok(false);
                }
                addrs[0]
            }
        };
        
        let result = tokio::time::timeout(timeout, async {
            TcpStream::connect(socket_addr)
        }).await;
        
        match result {
            Ok(Ok(_)) => Ok(true),
            Ok(Err(_)) => Ok(false),
            Err(_) => Ok(false), // Timeout
        }
    }
    
    /// Execute AWS CLI command to check SSM session
    async fn check_aws_ssm_session(&self, session_id: &str) -> Result<bool> {
        let output = Command::new("aws")
            .args(&["ssm", "describe-sessions", "--session-id", session_id])
            .output();
            
        match output {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Check if session is in Active state
                    Ok(stdout.contains("\"Status\": \"Connected\"") || stdout.contains("\"Status\": \"Active\""))
                } else {
                    debug!("AWS CLI command failed: {}", String::from_utf8_lossy(&output.stderr));
                    Ok(false)
                }
            }
            Err(e) => {
                debug!("Failed to execute AWS CLI: {}", e);
                Ok(false)
            }
        }
    }
}

#[async_trait]
impl HealthChecker for DefaultHealthChecker {
    /// Check SSM session health
    async fn check_ssm_session(&self, session_id: &str) -> Result<HealthCheckResult> {
        info!("Checking SSM session health: {}", session_id);
        
        let start = Instant::now();
        let mut is_healthy = false;
        let mut error_message = None;
        let mut details = None;
        
        // Check if AWS CLI is available
        let aws_cli_available = Command::new("aws").arg("--version").output().is_ok();
        
        if !aws_cli_available {
            error_message = Some("AWS CLI not available".to_string());
        } else {
            // Check session status via AWS CLI
            match self.check_aws_ssm_session(session_id).await {
                Ok(session_active) => {
                    if session_active {
                        is_healthy = true;
                        details = Some("Session is active and responsive".to_string());
                    } else {
                        error_message = Some("Session is not active or not found".to_string());
                    }
                }
                Err(e) => {
                    error_message = Some(format!("Failed to check session status: {}", e));
                }
            }
        }
        
        let response_time = start.elapsed().as_millis() as u64;
        
        // Check response time thresholds
        if is_healthy && response_time > self.error_threshold_ms {
            is_healthy = false;
            error_message = Some(format!("Response time too high: {}ms", response_time));
        } else if is_healthy && response_time > self.warning_threshold_ms {
            details = Some(format!("Warning: High response time: {}ms", response_time));
        }
        
        let result = HealthCheckResult {
            is_healthy,
            response_time_ms: response_time,
            error_message,
            details,
        };
        
        if result.is_healthy {
            info!("SSM session {} is healthy ({}ms)", session_id, response_time);
        } else {
            warn!("SSM session {} is unhealthy: {:?}", session_id, result.error_message);
        }
        
        Ok(result)
    }
    
    /// Check network connectivity
    async fn check_network_connectivity(&self, target: &str) -> Result<HealthCheckResult> {
        info!("Checking network connectivity to: {}", target);
        
        let start = Instant::now();
        let mut is_healthy = false;
        let mut error_message = None;
        let mut details = None;
        
        // Parse target (default to port 443 for HTTPS)
        let (host, port) = if target.contains(':') {
            let parts: Vec<&str> = target.split(':').collect();
            (parts[0], parts[1].parse::<u16>().unwrap_or(443))
        } else {
            (target, 443)
        };
        
        // Test TCP connectivity
        let timeout = Duration::from_secs(5);
        match self.test_tcp_connectivity(host, port, timeout).await {
            Ok(connected) => {
                if connected {
                    is_healthy = true;
                    details = Some(format!("Successfully connected to {}:{}", host, port));
                } else {
                    error_message = Some(format!("Failed to connect to {}:{}", host, port));
                }
            }
            Err(e) => {
                error_message = Some(format!("Network connectivity test failed: {}", e));
            }
        }
        
        let response_time = start.elapsed().as_millis() as u64;
        
        // Check response time thresholds
        if is_healthy && response_time > self.error_threshold_ms {
            is_healthy = false;
            error_message = Some(format!("Network response time too high: {}ms", response_time));
        } else if is_healthy && response_time > self.warning_threshold_ms {
            details = Some(format!("Warning: High network latency: {}ms", response_time));
        }
        
        let result = HealthCheckResult {
            is_healthy,
            response_time_ms: response_time,
            error_message,
            details,
        };
        
        if result.is_healthy {
            info!("Network connectivity to {} is healthy ({}ms)", target, response_time);
        } else {
            warn!("Network connectivity to {} failed: {:?}", target, result.error_message);
        }
        
        Ok(result)
    }
    
    /// Check resource availability
    async fn check_resource_availability(&self) -> Result<ResourceAvailability> {
        info!("Checking resource availability");
        
        let mut system = System::new_all();
        system.refresh_all();
        
        // Memory availability
        let total_memory = system.total_memory() as f64 / 1024.0 / 1024.0; // Convert to MB
        let used_memory = system.used_memory() as f64 / 1024.0 / 1024.0;
        let available_memory = total_memory - used_memory;
        
        // CPU availability (average over all cores)
        let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
        let cpu_available = 100.0 - cpu_usage;
        
        // Disk availability - simplified approach without disk enumeration
        let disk_available = 1000.0; // Default fallback value in MB
        let mut network_available = true;
        
        // Network availability (basic check)
        if let Ok(connected) = self.test_tcp_connectivity("8.8.8.8", 53, Duration::from_secs(3)).await {
            network_available = connected;
        }
        
        let availability = ResourceAvailability {
            memory_available_mb: available_memory,
            memory_total_mb: total_memory,
            memory_usage_percent: (used_memory / total_memory) * 100.0,
            cpu_available_percent: cpu_available,
            cpu_usage_percent: cpu_usage,
            disk_available_mb: disk_available,
            network_available,
            process_count: system.processes().len() as u32,
        };
        
        info!("Resource availability - Memory: {:.1}MB/{:.1}MB ({:.1}%), CPU: {:.1}% available, Disk: {:.1}MB, Network: {}",
            available_memory, total_memory, availability.memory_usage_percent,
            cpu_available, disk_available, network_available);
        
        Ok(availability)
    }
    
    /// Perform comprehensive health check
    async fn comprehensive_health_check(&self, session_id: &str) -> Result<ComprehensiveHealthResult> {
        info!("Performing comprehensive health check for session: {}", session_id);
        
        let start = Instant::now();
        
        // Perform all health checks concurrently
        let (ssm_result, network_result, resource_result) = tokio::join!(
            self.check_ssm_session(session_id),
            self.check_network_connectivity("ssm.amazonaws.com"),
            self.check_resource_availability()
        );
        
        let ssm_health = ssm_result?;
        let network_health = network_result?;
        let resource_availability = resource_result?;
        
        // Determine overall health
        let overall_healthy = ssm_health.is_healthy 
            && network_health.is_healthy 
            && resource_availability.network_available
            && resource_availability.memory_available_mb > 50.0  // At least 50MB available
            && resource_availability.cpu_available_percent > 10.0; // At least 10% CPU available
        
        let total_time = start.elapsed().as_millis() as u64;
        
        let result = ComprehensiveHealthResult {
            overall_healthy,
            ssm_health: ssm_health.clone(),
            network_health: network_health.clone(),
            resource_availability,
            check_duration_ms: total_time,
            timestamp: chrono::Utc::now(),
        };
        
        if result.overall_healthy {
            info!("Comprehensive health check passed for session: {} ({}ms)", session_id, total_time);
        } else {
            warn!("Comprehensive health check failed for session: {} ({}ms)", session_id, total_time);
            
            // Send early warning if health check failed
            let warning = format!("Health check failed - SSM: {}, Network: {}, Resources: low", 
                ssm_health.is_healthy, network_health.is_healthy);
            if let Err(e) = self.send_early_warning(session_id, &warning).await {
                error!("Failed to send early warning: {}", e);
            }
        }
        
        Ok(result)
    }
    
    /// Send early warning notification
    async fn send_early_warning(&self, session_id: &str, warning: &str) -> Result<()> {
        warn!("🚨 Early warning for session {}: {}", session_id, warning);
        
        // Log structured warning for monitoring systems
        tracing::event!(
            tracing::Level::WARN,
            session_id = session_id,
            warning_type = "health_degradation",
            message = warning,
            "Early warning notification"
        );
        
        // TODO: Implement additional notification methods:
        // - Desktop notifications (using system notification APIs)
        // - Email alerts (if configured)
        // - Webhook notifications (if configured)
        // - Slack/Teams integration (if configured)
        
        // For now, we ensure the warning is prominently logged
        eprintln!("⚠️  WARNING: Session {} - {}", session_id, warning);
        
        Ok(())
    }
}

/// Resource availability information
#[derive(Debug, Clone)]
pub struct ResourceAvailability {
    pub memory_available_mb: f64,
    pub memory_total_mb: f64,
    pub memory_usage_percent: f64,
    pub cpu_available_percent: f64,
    pub cpu_usage_percent: f64,
    pub disk_available_mb: f64,
    pub network_available: bool,
    pub process_count: u32,
}

/// Comprehensive health check result
#[derive(Debug, Clone)]
pub struct ComprehensiveHealthResult {
    pub overall_healthy: bool,
    pub ssm_health: HealthCheckResult,
    pub network_health: HealthCheckResult,
    pub resource_availability: ResourceAvailability,
    pub check_duration_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Health monitoring configuration
#[derive(Debug, Clone)]
pub struct HealthConfig {
    pub check_interval: Duration,
    pub warning_threshold_ms: u64,
    pub error_threshold_ms: u64,
    pub memory_warning_threshold_mb: f64,
    pub cpu_warning_threshold_percent: f64,
    pub enable_notifications: bool,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            warning_threshold_ms: 1000,
            error_threshold_ms: 5000,
            memory_warning_threshold_mb: 100.0,
            cpu_warning_threshold_percent: 20.0,
            enable_notifications: true,
        }
    }
}