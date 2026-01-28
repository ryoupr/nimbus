use crate::error::Result;
use sysinfo::{System, Pid};
use std::time::{Duration, Instant};
use tracing::{info, warn, error, debug};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Resource usage information
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub process_count: usize,
}

/// Resource limits configuration optimized for EC2 Connect requirements
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory usage in MB (default: 10MB for Rust optimization)
    pub max_memory_mb: u64,
    /// Maximum CPU usage percentage (default: 0.5% for Rust optimization)
    pub max_cpu_percent: f64,
    /// Maximum number of processes
    pub max_processes: usize,
    /// Warning threshold as percentage of limit (default: 80%)
    pub warning_threshold: f64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 10,      // 10MB limit for Rust optimization
            max_cpu_percent: 0.5,   // 0.5% CPU limit for Rust optimization
            max_processes: 5,
            warning_threshold: 0.8,
        }
    }
}

/// Resource monitor for tracking system resource usage with Rust optimization
pub struct ResourceMonitor {
    system: Arc<RwLock<System>>,
    limits: ResourceLimits,
    monitoring_interval: Duration,
    low_power_mode: bool,
    start_time: Instant,
    last_cleanup: Instant,
    monitoring_active: bool,
    cpu_samples: Vec<f64>,
    memory_samples: Vec<f64>,
}

impl ResourceMonitor {
    /// Create new resource monitor with optimized defaults for Rust
    pub fn new() -> Self {
        Self::with_limits(ResourceLimits::default())
    }
    
    /// Create resource monitor with custom limits
    pub fn with_limits(limits: ResourceLimits) -> Self {
        Self {
            system: Arc::new(RwLock::new(System::new_all())),
            limits,
            monitoring_interval: Duration::from_secs(5),
            low_power_mode: false,
            start_time: Instant::now(),
            last_cleanup: Instant::now(),
            monitoring_active: false,
            cpu_samples: Vec::with_capacity(60), // Keep 5 minutes of samples
            memory_samples: Vec::with_capacity(60),
        }
    }
    
    /// Get current resource usage with enhanced accuracy
    pub async fn get_current_usage(&self) -> Result<ResourceUsage> {
        let mut system = self.system.write().await;
        system.refresh_all();
        
        // Get memory usage for current process with better accuracy
        let current_pid = std::process::id();
        let (memory_mb, cpu_percent) = if let Some(process) = system.process(Pid::from(current_pid as usize)) {
            let memory_bytes = process.memory();
            let memory_mb = memory_bytes as f64 / 1024.0 / 1024.0;
            let cpu_percent = process.cpu_usage() as f64;
            (memory_mb, cpu_percent)
        } else {
            (0.0, 0.0)
        };
        
        // Count EC2 Connect related processes more accurately
        let process_count = system
            .processes()
            .values()
            .filter(|p| {
                let name = p.name().to_lowercase();
                let cmd = p.cmd().join(" ").to_lowercase();
                name.contains("ec2-connect") || 
                name.contains("ec2_connect") ||
                (name.contains("aws") && cmd.contains("ssm")) ||
                cmd.contains("ec2-connect")
            })
            .count();
        
        let usage = ResourceUsage {
            memory_mb,
            cpu_percent,
            process_count,
        };
        
        debug!("Current resource usage: memory={:.2}MB, cpu={:.2}%, processes={}", 
               usage.memory_mb, usage.cpu_percent, usage.process_count);
        
        Ok(usage)
    }
    
    /// Check if resource limits are exceeded with detailed reporting
    pub async fn check_limits(&self) -> Result<Vec<ResourceViolation>> {
        let usage = self.get_current_usage().await?;
        let mut violations = Vec::new();
        
        // Check memory limit (10MB for Rust optimization)
        if usage.memory_mb > self.limits.max_memory_mb as f64 {
            violations.push(ResourceViolation::MemoryExceeded {
                current: usage.memory_mb,
                limit: self.limits.max_memory_mb as f64,
            });
            error!("Memory limit exceeded: {:.2}MB > {}MB", 
                   usage.memory_mb, self.limits.max_memory_mb);
        }
        
        // Check CPU limit (0.5% for Rust optimization)
        if usage.cpu_percent > self.limits.max_cpu_percent {
            violations.push(ResourceViolation::CpuExceeded {
                current: usage.cpu_percent,
                limit: self.limits.max_cpu_percent,
            });
            error!("CPU limit exceeded: {:.2}% > {}%", 
                   usage.cpu_percent, self.limits.max_cpu_percent);
        }
        
        // Check process count
        if usage.process_count > self.limits.max_processes {
            violations.push(ResourceViolation::ProcessCountExceeded {
                current: usage.process_count,
                limit: self.limits.max_processes,
            });
            error!("Process count limit exceeded: {} > {}", 
                   usage.process_count, self.limits.max_processes);
        }
        
        if !violations.is_empty() {
            warn!("Resource limit violations detected: {} violations", violations.len());
        }
        
        Ok(violations)
    }
    
    /// Enable low power mode with aggressive optimization
    pub async fn enable_low_power_mode(&mut self) -> Result<()> {
        if self.low_power_mode {
            return Ok(());
        }
        
        info!("Enabling low power mode for resource optimization");
        self.low_power_mode = true;
        
        // Significantly reduce monitoring frequency in low power mode
        self.monitoring_interval = Duration::from_secs(30);
        
        // Implement additional power saving measures
        self.optimize_for_low_power().await?;
        
        info!("Low power mode enabled: monitoring interval increased to {}s", 
              self.monitoring_interval.as_secs());
        
        Ok(())
    }
    
    /// Optimize resources for low power operation
    async fn optimize_for_low_power(&mut self) -> Result<()> {
        debug!("Applying low power optimizations");
        
        // Clear sample buffers to reduce memory usage
        self.cpu_samples.clear();
        self.memory_samples.clear();
        self.cpu_samples.shrink_to_fit();
        self.memory_samples.shrink_to_fit();
        
        // Force garbage collection by dropping and recreating system info
        {
            let mut system = self.system.write().await;
            *system = System::new();
        }
        
        debug!("Low power optimizations applied");
        Ok(())
    }
    
    /// Optimize resource usage with intelligent strategies
    pub async fn optimize_resources(&mut self) -> Result<OptimizationResult> {
        let start_time = Instant::now();
        let usage_before = self.get_current_usage().await?;
        
        info!("Starting resource optimization - current usage: {:.2}MB memory, {:.2}% CPU", 
              usage_before.memory_mb, usage_before.cpu_percent);
        
        let mut actions_taken = Vec::new();
        
        // Memory optimization
        if usage_before.memory_mb > (self.limits.max_memory_mb as f64 * self.limits.warning_threshold) {
            warn!("High memory usage detected: {:.2}MB (threshold: {:.2}MB)", 
                  usage_before.memory_mb, 
                  self.limits.max_memory_mb as f64 * self.limits.warning_threshold);
            
            self.cleanup_memory().await?;
            actions_taken.push("memory_cleanup".to_string());
        }
        
        // CPU optimization
        if usage_before.cpu_percent > (self.limits.max_cpu_percent * self.limits.warning_threshold) {
            warn!("High CPU usage detected: {:.2}% (threshold: {:.2}%)", 
                  usage_before.cpu_percent, 
                  self.limits.max_cpu_percent * self.limits.warning_threshold);
            
            if !self.low_power_mode {
                self.enable_low_power_mode().await?;
                actions_taken.push("low_power_mode_enabled".to_string());
            }
        }
        
        // Periodic cleanup
        if self.last_cleanup.elapsed() > Duration::from_secs(300) { // 5 minutes
            self.periodic_cleanup().await?;
            self.last_cleanup = Instant::now();
            actions_taken.push("periodic_cleanup".to_string());
        }
        
        let usage_after = self.get_current_usage().await?;
        let optimization_time = start_time.elapsed();
        
        let result = OptimizationResult {
            memory_before_mb: usage_before.memory_mb,
            memory_after_mb: usage_after.memory_mb,
            cpu_before_percent: usage_before.cpu_percent,
            cpu_after_percent: usage_after.cpu_percent,
            actions_taken,
            optimization_time,
        };
        
        info!("Resource optimization completed in {:?}: memory {:.2}MB -> {:.2}MB, CPU {:.2}% -> {:.2}%", 
              optimization_time, 
              usage_before.memory_mb, usage_after.memory_mb,
              usage_before.cpu_percent, usage_after.cpu_percent);
        
        Ok(result)
    }
    
    /// Clean up memory usage with aggressive strategies
    async fn cleanup_memory(&mut self) -> Result<()> {
        info!("Performing aggressive memory cleanup");
        
        // Clear sample buffers
        self.cpu_samples.clear();
        self.memory_samples.clear();
        
        // Shrink vectors to minimum capacity
        self.cpu_samples.shrink_to_fit();
        self.memory_samples.shrink_to_fit();
        
        // Refresh system info to clear internal caches
        {
            let mut system = self.system.write().await;
            system.refresh_memory();
        }
        
        debug!("Memory cleanup completed");
        Ok(())
    }
    
    /// Perform periodic cleanup operations
    async fn periodic_cleanup(&mut self) -> Result<()> {
        debug!("Performing periodic cleanup");
        
        // Limit sample buffer sizes
        const MAX_SAMPLES: usize = 60;
        
        if self.cpu_samples.len() > MAX_SAMPLES {
            self.cpu_samples.drain(0..self.cpu_samples.len() - MAX_SAMPLES);
        }
        
        if self.memory_samples.len() > MAX_SAMPLES {
            self.memory_samples.drain(0..self.memory_samples.len() - MAX_SAMPLES);
        }
        
        debug!("Periodic cleanup completed");
        Ok(())
    }
    
    /// Get resource efficiency metrics with comprehensive analysis
    pub async fn get_efficiency_metrics(&self) -> Result<EfficiencyMetrics> {
        let usage = self.get_current_usage().await?;
        
        // Calculate efficiency percentages
        let memory_efficiency = ((self.limits.max_memory_mb as f64 - usage.memory_mb) 
            / self.limits.max_memory_mb as f64 * 100.0).max(0.0);
        
        let cpu_efficiency = ((self.limits.max_cpu_percent - usage.cpu_percent) 
            / self.limits.max_cpu_percent * 100.0).max(0.0);
        
        // Calculate uptime
        let uptime_seconds = self.start_time.elapsed().as_secs();
        
        let metrics = EfficiencyMetrics {
            memory_efficiency_percent: memory_efficiency,
            cpu_efficiency_percent: cpu_efficiency,
            low_power_mode_active: self.low_power_mode,
            uptime_seconds,
        };
        
        debug!("Efficiency metrics: memory={:.1}%, cpu={:.1}%, uptime={}s", 
               memory_efficiency, cpu_efficiency, uptime_seconds);
        
        Ok(metrics)
    }
    
    /// Get monitoring status and statistics
    pub fn get_monitoring_status(&self) -> MonitoringStatus {
        MonitoringStatus {
            active: self.monitoring_active,
            low_power_mode: self.low_power_mode,
            monitoring_interval: self.monitoring_interval,
            uptime: self.start_time.elapsed(),
            sample_count: self.cpu_samples.len(),
        }
    }
    
    /// Check if system is operating within optimal parameters
    pub async fn is_operating_optimally(&self) -> Result<bool> {
        let usage = self.get_current_usage().await?;
        let violations = self.check_limits().await?;
        
        // Check if we're within optimal thresholds (50% of limits)
        let optimal_memory_threshold = self.limits.max_memory_mb as f64 * 0.5;
        let optimal_cpu_threshold = self.limits.max_cpu_percent * 0.5;
        
        let is_optimal = violations.is_empty() &&
            usage.memory_mb <= optimal_memory_threshold &&
            usage.cpu_percent <= optimal_cpu_threshold;
        
        debug!("Operating optimally: {} (memory: {:.2}MB <= {:.2}MB, cpu: {:.2}% <= {:.2}%)", 
               is_optimal, usage.memory_mb, optimal_memory_threshold, 
               usage.cpu_percent, optimal_cpu_threshold);
        
        Ok(is_optimal)
    }
}

/// Resource violation types
#[derive(Debug, Clone)]
pub enum ResourceViolation {
    MemoryExceeded { current: f64, limit: f64 },
    CpuExceeded { current: f64, limit: f64 },
    ProcessCountExceeded { current: usize, limit: usize },
}

/// Resource efficiency metrics with comprehensive tracking
#[derive(Debug, Clone)]
pub struct EfficiencyMetrics {
    pub memory_efficiency_percent: f64,
    pub cpu_efficiency_percent: f64,
    pub low_power_mode_active: bool,
    pub uptime_seconds: u64,
}

/// Resource optimization result
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub memory_before_mb: f64,
    pub memory_after_mb: f64,
    pub cpu_before_percent: f64,
    pub cpu_after_percent: f64,
    pub actions_taken: Vec<String>,
    pub optimization_time: Duration,
}

/// Monitoring status information
#[derive(Debug, Clone)]
pub struct MonitoringStatus {
    pub active: bool,
    pub low_power_mode: bool,
    pub monitoring_interval: Duration,
    pub uptime: Duration,
    pub sample_count: usize,
}