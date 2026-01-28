#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, SystemTime};
use std::sync::{Arc, Mutex};
use sysinfo::{System, Pid};
use tokio::time::Instant;
use tracing::{info, warn, error, debug};
use async_trait::async_trait;
use uuid::Uuid;

use crate::diagnostic::{DiagnosticResult, Severity};

/// Information about a port and its usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: u16,
    pub is_available: bool,
    pub process_name: Option<String>,
    pub process_id: Option<u32>,
    pub process_user: Option<String>,
    pub protocol: String,
}

impl PortInfo {
    pub fn available(port: u16) -> Self {
        Self {
            port,
            is_available: true,
            process_name: None,
            process_id: None,
            process_user: None,
            protocol: "tcp".to_string(),
        }
    }

    pub fn occupied(port: u16, process_info: Option<ProcessInfo>) -> Self {
        if let Some(info) = process_info {
            Self {
                port,
                is_available: false,
                process_name: Some(info.name),
                process_id: Some(info.pid),
                process_user: Some(info.user),
                protocol: "tcp".to_string(),
            }
        } else {
            Self {
                port,
                is_available: false,
                process_name: None,
                process_id: None,
                process_user: None,
                protocol: "tcp".to_string(),
            }
        }
    }
}

/// Information about a process using a port
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub command_line: String,
    pub can_terminate: bool,
}

impl ProcessInfo {
    pub fn new(pid: u32, name: String, user: String, command_line: String) -> Self {
        // Determine if the process can be safely terminated
        let can_terminate = !Self::is_system_critical(&name);
        
        Self {
            pid,
            name,
            user,
            command_line,
            can_terminate,
        }
    }

    /// Check if a process is system-critical and should not be terminated
    pub fn is_system_critical(process_name: &str) -> bool {
        let critical_processes = [
            "kernel", "init", "systemd", "kthreadd", "ksoftirqd", "migration",
            "rcu_", "watchdog", "sshd", "systemd-", "dbus", "NetworkManager",
            "explorer.exe", "winlogon.exe", "csrss.exe", "smss.exe", "wininit.exe",
            "services.exe", "lsass.exe", "dwm.exe", "conhost.exe",
        ];

        let name_lower = process_name.to_lowercase();
        critical_processes.iter().any(|&critical| {
            name_lower.contains(critical) || name_lower.starts_with(critical)
        })
    }
}

/// Port suggestion with alternatives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSuggestion {
    pub original_port: u16,
    pub suggested_ports: Vec<u16>,
    pub reason: String,
}

impl PortSuggestion {
    pub fn new(original_port: u16, suggested_ports: Vec<u16>, reason: String) -> Self {
        Self {
            original_port,
            suggested_ports,
            reason,
        }
    }
}

/// Port reservation to prevent conflicts
#[derive(Debug)]
pub struct PortReservation {
    pub port: u16,
    pub reserved_at: SystemTime,
    pub reservation_id: String,
    _listener: Option<TcpListener>,
}

impl PortReservation {
    pub fn new(port: u16, listener: TcpListener) -> Self {
        Self {
            port,
            reserved_at: SystemTime::now(),
            reservation_id: Uuid::new_v4().to_string(),
            _listener: Some(listener),
        }
    }

    pub fn age(&self) -> Duration {
        self.reserved_at.elapsed().unwrap_or(Duration::ZERO)
    }
}

// Implement Clone for PortReservation (needed for the reservation system)
impl Clone for PortReservation {
    fn clone(&self) -> Self {
        Self {
            port: self.port,
            reserved_at: self.reserved_at,
            reservation_id: self.reservation_id.clone(),
            _listener: None, // Don't clone the listener
        }
    }
}

/// Trait for port diagnostics implementations
#[async_trait]
pub trait PortDiagnostics {
    /// Check if a specific port is available
    async fn check_port_availability(&self, port: u16) -> Result<PortInfo, Box<dyn std::error::Error>>;
    
    /// Find the process using a specific port
    async fn find_process_using_port(&self, port: u16) -> Result<Option<ProcessInfo>, Box<dyn std::error::Error>>;
    
    /// Suggest alternative ports within a range
    async fn suggest_alternative_ports(&self, port: u16, range: u16) -> Result<PortSuggestion, Box<dyn std::error::Error>>;
    
    /// Reserve a port to prevent conflicts
    async fn reserve_port(&mut self, port: u16) -> Result<PortReservation, Box<dyn std::error::Error>>;
    
    /// Release a port reservation
    async fn release_port(&mut self, reservation: PortReservation) -> Result<(), Box<dyn std::error::Error>>;
    
    /// Run comprehensive port diagnostics
    async fn diagnose_port(&self, port: u16) -> DiagnosticResult;
}

/// Default implementation of port diagnostics
pub struct DefaultPortDiagnostics {
    system: Arc<Mutex<System>>,
    reservations: Arc<Mutex<HashMap<u16, PortReservation>>>,
}

impl DefaultPortDiagnostics {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        Self {
            system: Arc::new(Mutex::new(system)),
            reservations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a port is available by attempting to bind to it
    fn is_port_available(&self, port: u16) -> bool {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Get process information using sysinfo
    fn get_process_info_by_port(&self, port: u16) -> Option<ProcessInfo> {
        if let Ok(mut system) = self.system.lock() {
            system.refresh_processes();
            
            // Try to find the process using netstat-like functionality
            if let Some(process_info) = self.find_process_by_port_system_specific(port) {
                return Some(process_info);
            }

            // Fallback: scan all processes for network connections
            for (pid, process) in system.processes() {
                let pid_u32 = pid.as_u32();
                let name = process.name().to_string();
                let user = process.user_id().map(|u| u.to_string()).unwrap_or_else(|| "unknown".to_string());
                let cmd = process.cmd().join(" ");

                // This is a simplified approach - in a real implementation,
                // we would need to check the process's network connections
                if self.process_likely_uses_port(process, port) {
                    return Some(ProcessInfo::new(pid_u32, name, user, cmd));
                }
            }
        }

        None
    }

    /// Platform-specific method to find process by port
    #[cfg(unix)]
    fn find_process_by_port_system_specific(&self, port: u16) -> Option<ProcessInfo> {
        // Use lsof on Unix systems
        let output = Command::new("lsof")
            .args(&["-i", &format!(":{}", port), "-t"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return self.get_process_info_by_pid(pid);
                }
            }
        }

        // Fallback to netstat
        let output = Command::new("netstat")
            .args(&["-tlnp"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    if line.contains(&format!(":{}", port)) && line.contains("LISTEN") {
                        // Parse the PID from netstat output
                        if let Some(pid_info) = line.split_whitespace().last() {
                            if let Some(pid_str) = pid_info.split('/').next() {
                                if let Ok(pid) = pid_str.parse::<u32>() {
                                    return self.get_process_info_by_pid(pid);
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Platform-specific method to find process by port on Windows
    #[cfg(windows)]
    fn find_process_by_port_system_specific(&self, port: u16) -> Option<ProcessInfo> {
        // Use netstat on Windows
        let output = Command::new("netstat")
            .args(&["-ano"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    if line.contains(&format!(":{}", port)) && line.contains("LISTENING") {
                        // Parse the PID from netstat output
                        if let Some(pid_str) = line.split_whitespace().last() {
                            if let Ok(pid) = pid_str.parse::<u32>() {
                                return self.get_process_info_by_pid(pid);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Get process information by PID
    fn get_process_info_by_pid(&self, pid: u32) -> Option<ProcessInfo> {
        if let Ok(mut system) = self.system.lock() {
            system.refresh_processes();
            
            if let Some(process) = system.process(Pid::from(pid as usize)) {
                let name = process.name().to_string();
                let user = process.user_id().map(|u| u.to_string()).unwrap_or_else(|| "unknown".to_string());
                let cmd = process.cmd().join(" ");
                
                Some(ProcessInfo::new(pid, name, user, cmd))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Heuristic to determine if a process likely uses a specific port
    fn process_likely_uses_port(&self, process: &sysinfo::Process, port: u16) -> bool {
        let name = process.name().to_lowercase();
        let cmd = process.cmd().join(" ").to_lowercase();
        
        // Check if the port number appears in the command line
        cmd.contains(&port.to_string()) ||
        // Check for common server processes
        (port == 80 && (name.contains("nginx") || name.contains("apache") || name.contains("httpd"))) ||
        (port == 443 && (name.contains("nginx") || name.contains("apache") || name.contains("httpd"))) ||
        (port == 22 && name.contains("sshd")) ||
        (port == 3306 && name.contains("mysql")) ||
        (port == 5432 && name.contains("postgres"))
    }

    /// Find available ports in a range around the target port
    fn find_available_ports_in_range(&self, center_port: u16, range: u16) -> Vec<u16> {
        let start_port = center_port.saturating_sub(range);
        let end_port = center_port.saturating_add(range).min(65535);
        
        let mut available_ports = Vec::new();
        
        for port in start_port..=end_port {
            if port != center_port && self.is_port_available(port) {
                available_ports.push(port);
            }
        }
        
        // Sort by proximity to the original port
        available_ports.sort_by_key(|&port| {
            (port as i32 - center_port as i32).abs()
        });
        
        // Return up to 5 suggestions
        available_ports.truncate(5);
        available_ports
    }

    /// Clean up expired reservations
    fn cleanup_expired_reservations(&self) {
        let expiry_duration = Duration::from_secs(300); // 5 minutes
        
        if let Ok(mut reservations) = self.reservations.lock() {
            reservations.retain(|_port, reservation| {
                reservation.reserved_at.elapsed().unwrap_or(Duration::ZERO) < expiry_duration
            });
        }
    }
}

#[async_trait]
impl PortDiagnostics for DefaultPortDiagnostics {
    async fn check_port_availability(&self, port: u16) -> Result<PortInfo, Box<dyn std::error::Error>> {
        debug!("Checking availability of port {}", port);
        
        if self.is_port_available(port) {
            info!("Port {} is available", port);
            Ok(PortInfo::available(port))
        } else {
            info!("Port {} is occupied", port);
            // Try to get process information
            let process_info = self.get_process_info_by_port(port);
            Ok(PortInfo::occupied(port, process_info))
        }
    }

    async fn find_process_using_port(&self, port: u16) -> Result<Option<ProcessInfo>, Box<dyn std::error::Error>> {
        debug!("Finding process using port {}", port);
        
        let process_info = self.get_process_info_by_port(port);
        
        if let Some(ref info) = process_info {
            info!("Found process using port {}: {} (PID: {})", port, info.name, info.pid);
        } else {
            debug!("No process found using port {}", port);
        }
        
        Ok(process_info)
    }

    async fn suggest_alternative_ports(&self, port: u16, range: u16) -> Result<PortSuggestion, Box<dyn std::error::Error>> {
        debug!("Suggesting alternative ports for {} within range ±{}", port, range);
        
        let available_ports = self.find_available_ports_in_range(port, range);
        
        let reason = if available_ports.is_empty() {
            format!("No available ports found within ±{} range of port {}", range, port)
        } else {
            format!("Found {} alternative port(s) within ±{} range", available_ports.len(), range)
        };
        
        info!("Port suggestion for {}: {} alternatives found", port, available_ports.len());
        
        Ok(PortSuggestion::new(port, available_ports, reason))
    }

    async fn reserve_port(&mut self, port: u16) -> Result<PortReservation, Box<dyn std::error::Error>> {
        debug!("Attempting to reserve port {}", port);
        
        // Clean up expired reservations first
        self.cleanup_expired_reservations();
        
        // Check if port is already reserved
        if let Ok(reservations) = self.reservations.lock() {
            if reservations.contains_key(&port) {
                return Err(format!("Port {} is already reserved", port).into());
            }
        }
        
        // Try to bind to the port
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(listener) => {
                let reservation = PortReservation::new(port, listener);
                info!("Successfully reserved port {} with ID {}", port, reservation.reservation_id);
                
                if let Ok(mut reservations) = self.reservations.lock() {
                    reservations.insert(port, reservation.clone());
                }
                Ok(reservation)
            }
            Err(e) => {
                error!("Failed to reserve port {}: {}", port, e);
                Err(format!("Failed to reserve port {}: {}", port, e).into())
            }
        }
    }

    async fn release_port(&mut self, reservation: PortReservation) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Releasing port reservation for port {}", reservation.port);
        
        if let Ok(mut reservations) = self.reservations.lock() {
            if let Some(removed_reservation) = reservations.remove(&reservation.port) {
                if removed_reservation.reservation_id == reservation.reservation_id {
                    info!("Successfully released port reservation for port {}", reservation.port);
                    // The TcpListener will be dropped automatically, releasing the port
                    Ok(())
                } else {
                    warn!("Reservation ID mismatch for port {}", reservation.port);
                    Err(format!("Reservation ID mismatch for port {}", reservation.port).into())
                }
            } else {
                warn!("No reservation found for port {}", reservation.port);
                Err(format!("No reservation found for port {}", reservation.port).into())
            }
        } else {
            Err("Failed to acquire lock on reservations".into())
        }
    }

    async fn diagnose_port(&self, port: u16) -> DiagnosticResult {
        let start_time = Instant::now();
        info!("Running comprehensive port diagnostics for port {}", port);
        
        // Check port availability
        let port_info = match self.check_port_availability(port).await {
            Ok(info) => info,
            Err(e) => {
                error!("Port availability check failed: {}", e);
                return DiagnosticResult::error(
                    "local_port_availability".to_string(),
                    format!("Port availability check failed: {}", e),
                    start_time.elapsed(),
                    Severity::High,
                );
            }
        };

        if port_info.is_available {
            // Port is available
            info!("Port {} is available for use", port);
            DiagnosticResult::success(
                "local_port_availability".to_string(),
                format!("Port {} is available for use", port),
                start_time.elapsed(),
            ).with_details(serde_json::to_value(&port_info).unwrap_or_default())
        } else {
            // Port is occupied - provide detailed information
            let mut message = format!("Port {} is currently in use", port);
            let mut severity = Severity::Medium;
            let mut auto_fixable = false;

            if let Some(ref process_name) = port_info.process_name {
                message.push_str(&format!(" by process: {}", process_name));
                
                if let Some(pid) = port_info.process_id {
                    message.push_str(&format!(" (PID: {})", pid));
                }
                
                // Check if the process can be safely terminated
                if !ProcessInfo::is_system_critical(process_name) {
                    auto_fixable = true;
                    severity = Severity::Low;
                    message.push_str(" - Process can be safely terminated if needed");
                } else {
                    severity = Severity::High;
                    message.push_str(" - System critical process, cannot be terminated");
                }
            }

            // Try to suggest alternative ports
            match self.suggest_alternative_ports(port, 10).await {
                Ok(suggestion) => {
                    if !suggestion.suggested_ports.is_empty() {
                        message.push_str(&format!(
                            ". Alternative ports available: {}",
                            suggestion.suggested_ports.iter()
                                .take(3)
                                .map(|p| p.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
                Err(e) => {
                    warn!("Failed to suggest alternative ports: {}", e);
                }
            }

            warn!("Port {} diagnostic completed with issues: {}", port, message);
            DiagnosticResult::warning(
                "local_port_availability".to_string(),
                message,
                start_time.elapsed(),
                severity,
            )
            .with_details(serde_json::to_value(&port_info).unwrap_or_default())
            .with_auto_fixable(auto_fixable)
        }
    }
}

impl Default for DefaultPortDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_info_creation() {
        let available_port = PortInfo::available(8080);
        assert!(available_port.is_available);
        assert_eq!(available_port.port, 8080);
        assert!(available_port.process_name.is_none());

        let process_info = ProcessInfo::new(
            1234,
            "test_process".to_string(),
            "user".to_string(),
            "test command".to_string(),
        );
        let occupied_port = PortInfo::occupied(8080, Some(process_info));
        assert!(!occupied_port.is_available);
        assert_eq!(occupied_port.port, 8080);
        assert!(occupied_port.process_name.is_some());
    }

    #[test]
    fn test_process_info_system_critical() {
        assert!(ProcessInfo::is_system_critical("systemd"));
        assert!(ProcessInfo::is_system_critical("kernel"));
        assert!(ProcessInfo::is_system_critical("explorer.exe"));
        assert!(ProcessInfo::is_system_critical("lsass.exe"));
        assert!(!ProcessInfo::is_system_critical("firefox"));
        assert!(!ProcessInfo::is_system_critical("my_app"));
    }

    #[test]
    fn test_port_suggestion() {
        let suggestion = PortSuggestion::new(
            8080,
            vec![8081, 8082, 8083],
            "Test suggestion".to_string(),
        );
        
        assert_eq!(suggestion.original_port, 8080);
        assert_eq!(suggestion.suggested_ports.len(), 3);
        assert_eq!(suggestion.suggested_ports[0], 8081);
    }

    #[tokio::test]
    async fn test_port_diagnostics_creation() {
        let diagnostics = DefaultPortDiagnostics::new();
        
        // Test that we can create the diagnostics instance
        assert!(diagnostics.reservations.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_port_availability_check() {
        let diagnostics = DefaultPortDiagnostics::new();
        
        // Test with a likely available port (high port number)
        let result = diagnostics.check_port_availability(65432).await;
        assert!(result.is_ok());
        
        let port_info = result.unwrap();
        assert_eq!(port_info.port, 65432);
        // Note: We can't guarantee the port is available, so we just check the structure
    }

    #[tokio::test]
    async fn test_alternative_port_suggestion() {
        let diagnostics = DefaultPortDiagnostics::new();
        
        let result = diagnostics.suggest_alternative_ports(8080, 5).await;
        assert!(result.is_ok());
        
        let suggestion = result.unwrap();
        assert_eq!(suggestion.original_port, 8080);
        // The suggested ports list may be empty if all ports in range are occupied
    }

    #[tokio::test]
    async fn test_port_reservation() {
        let mut diagnostics = DefaultPortDiagnostics::new();
        
        // Try to reserve a high port number that's likely available
        let result = diagnostics.reserve_port(65431).await;
        
        if result.is_ok() {
            let reservation = result.unwrap();
            assert_eq!(reservation.port, 65431);
            assert!(!reservation.reservation_id.is_empty());
            
            // Test releasing the reservation
            let release_result = diagnostics.release_port(reservation).await;
            assert!(release_result.is_ok());
        }
        // If the port is not available, that's also a valid test outcome
    }

    #[tokio::test]
    async fn test_port_diagnostics() {
        let diagnostics = DefaultPortDiagnostics::new();
        
        // Test diagnostics on a high port number
        let result = diagnostics.diagnose_port(65430).await;
        
        assert_eq!(result.item_name, "local_port_availability");
        assert!(result.duration > Duration::ZERO);
        // The status depends on whether the port is actually available
    }
}