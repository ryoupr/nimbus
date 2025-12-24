use serde::{Deserialize, Serialize};
use std::time::{SystemTime, Instant, Duration};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::error::Result;
use tracing::{info, warn, debug, error};

/// Trait for performance monitoring functionality
pub trait PerformanceMonitorTrait: Send + Sync {
    /// Record connection establishment time
    fn record_connection_time(&self, session_id: &str, duration: f64) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Measure and record latency
    fn measure_latency(&self, session_id: &str, endpoint: Option<&str>) -> impl std::future::Future<Output = Result<f64>> + Send;
    
    /// Select optimal route for a region
    fn select_optimal_route(&self, region: &str) -> impl std::future::Future<Output = Result<Option<ConnectionRoute>>> + Send;
    
    /// Optimize connection based on performance data
    fn optimize_connection(&self, session_id: &str) -> impl std::future::Future<Output = Result<bool>> + Send;
    
    /// Get performance statistics for a session
    fn get_session_stats(&self, session_id: &str) -> impl std::future::Future<Output = Option<SessionStats>> + Send;
    
    /// Generate overall performance report
    fn generate_report(&self) -> impl std::future::Future<Output = PerformanceReport> + Send;
}

/// Performance metrics for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub session_id: String,
    pub connection_time: f64,
    pub latency: f64,
    pub throughput: f64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub timestamp: SystemTime,
    pub route_info: Option<RouteInfo>,
    pub optimization_applied: bool,
}

/// Route information for connection optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub endpoint: String,
    pub region: String,
    pub latency_ms: f64,
    pub is_optimal: bool,
}

/// Connection route for optimization
#[derive(Debug, Clone)]
pub struct ConnectionRoute {
    pub endpoint: String,
    pub region: String,
    pub priority: u8, // 1 = highest priority
    pub last_latency: Option<f64>,
    pub success_rate: f64,
}

impl ConnectionRoute {
    pub fn new(endpoint: String, region: String, priority: u8) -> Self {
        Self {
            endpoint,
            region,
            priority,
            last_latency: None,
            success_rate: 1.0,
        }
    }
    
    pub fn update_performance(&mut self, latency: f64, success: bool) {
        self.last_latency = Some(latency);
        // Simple exponential moving average for success rate
        let alpha = 0.1;
        self.success_rate = alpha * (if success { 1.0 } else { 0.0 }) + (1.0 - alpha) * self.success_rate;
    }
    
    pub fn score(&self) -> f64 {
        let latency_score = match self.last_latency {
            Some(lat) => 1000.0 / (lat + 1.0), // Lower latency = higher score
            None => 100.0, // Default score for untested routes
        };
        let priority_score = (10 - self.priority as u32) as f64 * 10.0;
        let success_score = self.success_rate * 100.0;
        
        latency_score + priority_score + success_score
    }
}

impl PerformanceMetrics {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            connection_time: 0.0,
            latency: 0.0,
            throughput: 0.0,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            timestamp: SystemTime::now(),
            route_info: None,
            optimization_applied: false,
        }
    }
    
    pub fn with_connection_time(mut self, connection_time: f64) -> Self {
        self.connection_time = connection_time;
        self
    }
    
    pub fn with_latency(mut self, latency: f64) -> Self {
        self.latency = latency;
        self
    }
    
    pub fn with_throughput(mut self, throughput: f64) -> Self {
        self.throughput = throughput;
        self
    }
    
    pub fn with_resource_usage(mut self, cpu_usage: f64, memory_usage: f64) -> Self {
        self.cpu_usage = cpu_usage;
        self.memory_usage = memory_usage;
        self
    }
    
    pub fn with_route_info(mut self, route_info: RouteInfo) -> Self {
        self.route_info = Some(route_info);
        self
    }
    
    pub fn with_optimization(mut self, applied: bool) -> Self {
        self.optimization_applied = applied;
        self
    }
    
    pub fn age_seconds(&self) -> u64 {
        self.timestamp
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    
    /// Check if performance is degraded based on thresholds
    pub fn is_degraded(&self, latency_threshold: f64, connection_time_threshold: f64) -> bool {
        self.latency > latency_threshold || self.connection_time > connection_time_threshold
    }
}

/// Performance monitor for tracking connection performance
pub struct PerformanceMonitor {
    metrics: Arc<RwLock<HashMap<String, Vec<PerformanceMetrics>>>>,
    routes: Arc<RwLock<HashMap<String, Vec<ConnectionRoute>>>>,
    latency_threshold_ms: f64,
    connection_time_threshold_ms: f64,
    monitoring_enabled: bool,
}

impl PerformanceMonitor {
    pub fn new(latency_threshold_ms: f64) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            routes: Arc::new(RwLock::new(HashMap::new())),
            latency_threshold_ms,
            connection_time_threshold_ms: 150.0, // 150ms target as per README
            monitoring_enabled: true,
        }
    }
    
    /// Configure performance thresholds
    pub fn configure_thresholds(&mut self, latency_ms: f64, connection_time_ms: f64) {
        self.latency_threshold_ms = latency_ms;
        self.connection_time_threshold_ms = connection_time_ms;
        info!("Performance thresholds updated: latency={}ms, connection_time={}ms", 
               latency_ms, connection_time_ms);
    }
    
    /// Enable or disable performance monitoring
    pub fn set_monitoring_enabled(&mut self, enabled: bool) {
        self.monitoring_enabled = enabled;
        info!("Performance monitoring {}", if enabled { "enabled" } else { "disabled" });
    }
    
    /// Add available routes for a region
    pub async fn add_routes(&self, region: &str, routes: Vec<ConnectionRoute>) -> Result<()> {
        let mut routes_map = self.routes.write().await;
        routes_map.insert(region.to_string(), routes);
        info!("Added {} routes for region: {}", routes_map.get(region).map(|r| r.len()).unwrap_or(0), region);
        Ok(())
    }
    
    /// Record connection establishment time (要件 4.1)
    pub async fn record_connection_time(&self, session_id: &str, duration: f64) -> Result<()> {
        if !self.monitoring_enabled {
            return Ok(());
        }
        
        info!("Recording connection time for session {}: {:.2}ms", session_id, duration);
        
        let metrics = PerformanceMetrics::new(session_id.to_string())
            .with_connection_time(duration);
        
        let mut metrics_map = self.metrics.write().await;
        metrics_map
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(metrics);
        
        // Check if connection time exceeds threshold (要件 4.4)
        if duration > self.connection_time_threshold_ms {
            warn!("Slow connection detected for session {}: {:.2}ms (threshold: {:.2}ms)", 
                  session_id, duration, self.connection_time_threshold_ms);
            self.log_performance_degradation(session_id, "slow_connection", duration).await?;
        }
        
        Ok(())
    }
    
    /// Measure and record latency (要件 4.2)
    pub async fn measure_latency(&self, session_id: &str, endpoint: Option<&str>) -> Result<f64> {
        if !self.monitoring_enabled {
            return Ok(0.0);
        }
        
        let start = Instant::now();
        
        // Simulate latency measurement - in real implementation, this would ping the endpoint
        let latency = if let Some(endpoint) = endpoint {
            self.ping_endpoint(endpoint).await?
        } else {
            // Default measurement
            tokio::time::sleep(Duration::from_millis(10)).await;
            start.elapsed().as_millis() as f64
        };
        
        // Check if latency exceeds threshold (要件 4.2)
        if latency > self.latency_threshold_ms {
            warn!("High latency detected for session {}: {:.2}ms (threshold: {:.2}ms)", 
                  session_id, latency, self.latency_threshold_ms);
            
            // Attempt optimization (要件 4.2)
            if let Err(e) = self.optimize_connection(session_id).await {
                error!("Failed to optimize connection for session {}: {}", session_id, e);
            }
            
            // Log performance degradation (要件 4.4)
            self.log_performance_degradation(session_id, "high_latency", latency).await?;
        }
        
        // Record latency metrics
        let metrics = PerformanceMetrics::new(session_id.to_string())
            .with_latency(latency);
        
        let mut metrics_map = self.metrics.write().await;
        metrics_map
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(metrics);
        
        info!("Measured latency for session {}: {:.2}ms", session_id, latency);
        Ok(latency)
    }
    
    /// Ping endpoint to measure actual latency
    async fn ping_endpoint(&self, endpoint: &str) -> Result<f64> {
        let start = Instant::now();
        
        // In a real implementation, this would use ICMP ping or TCP connect
        // For now, simulate with a small delay
        tokio::time::sleep(Duration::from_millis(5)).await;
        
        let latency = start.elapsed().as_millis() as f64;
        debug!("Pinged endpoint {}: {:.2}ms", endpoint, latency);
        Ok(latency)
    }
    
    /// Select optimal route (要件 4.3)
    pub async fn select_optimal_route(&self, region: &str) -> Result<Option<ConnectionRoute>> {
        let routes = self.routes.read().await;
        let region_routes = match routes.get(region) {
            Some(routes) => routes,
            None => {
                debug!("No routes available for region: {}", region);
                return Ok(None);
            }
        };
        
        if region_routes.is_empty() {
            return Ok(None);
        }
        
        // Find route with highest score (lowest latency, highest success rate, highest priority)
        let optimal_route = region_routes
            .iter()
            .max_by(|a, b| a.score().partial_cmp(&b.score()).unwrap_or(std::cmp::Ordering::Equal))
            .cloned();
        
        if let Some(ref route) = optimal_route {
            info!("Selected optimal route for region {}: {} (score: {:.2})", 
                  region, route.endpoint, route.score());
        }
        
        Ok(optimal_route)
    }
    
    /// Optimize connection based on performance data (要件 4.2, 4.3)
    pub async fn optimize_connection(&self, session_id: &str) -> Result<bool> {
        info!("Optimizing connection for session: {}", session_id);
        
        // Get current session metrics to determine region/endpoint
        let metrics_map = self.metrics.read().await;
        let session_metrics = metrics_map.get(session_id);
        
        if let Some(metrics) = session_metrics {
            if let Some(latest_metric) = metrics.last() {
                if let Some(route_info) = &latest_metric.route_info {
                    // Try to find a better route
                    if let Some(optimal_route) = self.select_optimal_route(&route_info.region).await? {
                        if optimal_route.endpoint != route_info.endpoint {
                            info!("Found better route for session {}: {} -> {}", 
                                  session_id, route_info.endpoint, optimal_route.endpoint);
                            
                            // Record optimization attempt
                            let optimization_metrics = PerformanceMetrics::new(session_id.to_string())
                                .with_optimization(true)
                                .with_route_info(RouteInfo {
                                    endpoint: optimal_route.endpoint.clone(),
                                    region: optimal_route.region.clone(),
                                    latency_ms: optimal_route.last_latency.unwrap_or(0.0),
                                    is_optimal: true,
                                });
                            
                            drop(metrics_map);
                            let mut metrics_map = self.metrics.write().await;
                            metrics_map
                                .entry(session_id.to_string())
                                .or_insert_with(Vec::new)
                                .push(optimization_metrics);
                            
                            return Ok(true);
                        }
                    }
                }
            }
        }
        
        // No optimization needed or possible
        debug!("No optimization available for session: {}", session_id);
        Ok(false)
    }
    
    /// Log performance degradation with detailed metrics (要件 4.4)
    async fn log_performance_degradation(&self, session_id: &str, degradation_type: &str, value: f64) -> Result<()> {
        let metrics_map = self.metrics.read().await;
        let session_metrics = metrics_map.get(session_id);
        
        let detailed_info = if let Some(metrics) = session_metrics {
            let recent_metrics: Vec<_> = metrics.iter()
                .rev()
                .take(5)
                .collect();
            
            format!(
                "Session: {}, Type: {}, Value: {:.2}, Recent metrics: [{}]",
                session_id,
                degradation_type,
                value,
                recent_metrics.iter()
                    .map(|m| format!("lat:{:.1}ms,conn:{:.1}ms", m.latency, m.connection_time))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            format!("Session: {}, Type: {}, Value: {:.2}, No historical data", 
                    session_id, degradation_type, value)
        };
        
        warn!("Performance degradation detected: {}", detailed_info);
        
        // In a real implementation, this could also:
        // - Write to a performance log file
        // - Send metrics to a monitoring system
        // - Trigger alerts
        
        Ok(())
    }
    
    /// Get performance statistics for a session (要件 4.5)
    pub async fn get_session_stats(&self, session_id: &str) -> Option<SessionStats> {
        let metrics_map = self.metrics.read().await;
        let metrics = metrics_map.get(session_id)?;
        
        if metrics.is_empty() {
            return None;
        }
        
        let avg_connection_time = metrics.iter()
            .map(|m| m.connection_time)
            .sum::<f64>() / metrics.len() as f64;
        
        let avg_latency = metrics.iter()
            .map(|m| m.latency)
            .sum::<f64>() / metrics.len() as f64;
        
        let max_latency = metrics.iter()
            .map(|m| m.latency)
            .fold(0.0, f64::max);
        
        let min_latency = metrics.iter()
            .map(|m| m.latency)
            .filter(|&l| l > 0.0)
            .fold(f64::INFINITY, f64::min);
        
        let optimizations_applied = metrics.iter()
            .filter(|m| m.optimization_applied)
            .count();
        
        let degradation_count = metrics.iter()
            .filter(|m| m.is_degraded(self.latency_threshold_ms, self.connection_time_threshold_ms))
            .count();
        
        Some(SessionStats {
            session_id: session_id.to_string(),
            avg_connection_time,
            avg_latency,
            max_latency,
            min_latency: if min_latency.is_infinite() { 0.0 } else { min_latency },
            total_measurements: metrics.len(),
            optimizations_applied,
            degradation_count,
            current_route: metrics.last()
                .and_then(|m| m.route_info.as_ref())
                .map(|r| r.endpoint.clone()),
        })
    }
    
    /// Get overall performance report (要件 4.5)
    pub async fn generate_report(&self) -> PerformanceReport {
        let metrics_map = self.metrics.read().await;
        let mut total_sessions = 0;
        let mut total_connection_time = 0.0;
        let mut total_latency = 0.0;
        let mut max_latency: f64 = 0.0;
        let mut min_latency: f64 = f64::INFINITY;
        let mut total_optimizations = 0;
        let mut total_degradations = 0;
        let mut total_measurements = 0;
        
        for metrics in metrics_map.values() {
            if !metrics.is_empty() {
                total_sessions += 1;
                total_measurements += metrics.len();
                
                let avg_conn_time = metrics.iter()
                    .map(|m| m.connection_time)
                    .sum::<f64>() / metrics.len() as f64;
                total_connection_time += avg_conn_time;
                
                let avg_lat = metrics.iter()
                    .map(|m| m.latency)
                    .filter(|&l| l > 0.0)
                    .sum::<f64>();
                let lat_count = metrics.iter()
                    .filter(|m| m.latency > 0.0)
                    .count();
                if lat_count > 0 {
                    total_latency += avg_lat / lat_count as f64;
                }
                
                let session_max_lat = metrics.iter()
                    .map(|m| m.latency)
                    .fold(0.0, f64::max);
                max_latency = max_latency.max(session_max_lat);
                
                let session_min_lat = metrics.iter()
                    .map(|m| m.latency)
                    .filter(|&l| l > 0.0)
                    .fold(f64::INFINITY, f64::min);
                if !session_min_lat.is_infinite() {
                    min_latency = min_latency.min(session_min_lat);
                }
                
                total_optimizations += metrics.iter()
                    .filter(|m| m.optimization_applied)
                    .count();
                
                total_degradations += metrics.iter()
                    .filter(|m| m.is_degraded(self.latency_threshold_ms, self.connection_time_threshold_ms))
                    .count();
            }
        }
        
        PerformanceReport {
            total_sessions,
            total_measurements,
            avg_connection_time: if total_sessions > 0 { total_connection_time / total_sessions as f64 } else { 0.0 },
            avg_latency: if total_sessions > 0 { total_latency / total_sessions as f64 } else { 0.0 },
            max_latency,
            min_latency: if min_latency.is_infinite() { 0.0 } else { min_latency },
            optimizations_applied: total_optimizations,
            degradations_detected: total_degradations,
            latency_threshold: self.latency_threshold_ms,
            connection_time_threshold: self.connection_time_threshold_ms,
            timestamp: SystemTime::now(),
        }
    }
    
    /// Update route performance based on measurement results
    pub async fn update_route_performance(&self, region: &str, endpoint: &str, latency: f64, success: bool) -> Result<()> {
        let mut routes_map = self.routes.write().await;
        if let Some(routes) = routes_map.get_mut(region) {
            if let Some(route) = routes.iter_mut().find(|r| r.endpoint == endpoint) {
                route.update_performance(latency, success);
                debug!("Updated route performance for {}: latency={:.2}ms, success={}, score={:.2}", 
                       endpoint, latency, success, route.score());
            }
        }
        Ok(())
    }
    
    /// Get route statistics for a region
    pub async fn get_route_stats(&self, region: &str) -> Vec<RouteStats> {
        let routes_map = self.routes.read().await;
        if let Some(routes) = routes_map.get(region) {
            routes.iter()
                .map(|route| RouteStats {
                    endpoint: route.endpoint.clone(),
                    region: route.region.clone(),
                    priority: route.priority,
                    last_latency: route.last_latency,
                    success_rate: route.success_rate,
                    score: route.score(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

/// Performance statistics for a single session
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub session_id: String,
    pub avg_connection_time: f64,
    pub avg_latency: f64,
    pub max_latency: f64,
    pub min_latency: f64,
    pub total_measurements: usize,
    pub optimizations_applied: usize,
    pub degradation_count: usize,
    pub current_route: Option<String>,
}

/// Overall performance report
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub total_sessions: usize,
    pub total_measurements: usize,
    pub avg_connection_time: f64,
    pub avg_latency: f64,
    pub max_latency: f64,
    pub min_latency: f64,
    pub optimizations_applied: usize,
    pub degradations_detected: usize,
    pub latency_threshold: f64,
    pub connection_time_threshold: f64,
    pub timestamp: SystemTime,
}

/// Route performance statistics
#[derive(Debug, Clone)]
pub struct RouteStats {
    pub endpoint: String,
    pub region: String,
    pub priority: u8,
    pub last_latency: Option<f64>,
    pub success_rate: f64,
    pub score: f64,
}

impl PerformanceMonitorTrait for PerformanceMonitor {
    async fn record_connection_time(&self, session_id: &str, duration: f64) -> Result<()> {
        self.record_connection_time(session_id, duration).await
    }
    
    async fn measure_latency(&self, session_id: &str, endpoint: Option<&str>) -> Result<f64> {
        self.measure_latency(session_id, endpoint).await
    }
    
    async fn select_optimal_route(&self, region: &str) -> Result<Option<ConnectionRoute>> {
        self.select_optimal_route(region).await
    }
    
    async fn optimize_connection(&self, session_id: &str) -> Result<bool> {
        self.optimize_connection(session_id).await
    }
    
    async fn get_session_stats(&self, session_id: &str) -> Option<SessionStats> {
        self.get_session_stats(session_id).await
    }
    
    async fn generate_report(&self) -> PerformanceReport {
        self.generate_report().await
    }
}