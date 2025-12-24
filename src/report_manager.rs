use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::{DiagnosticResult, DiagnosticConfig, DiagnosticStatus, Severity};
use crate::auto_fix::{FixResult, FixAction};
use crate::suggestion_generator::FixSuggestion;

/// Report format options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReportFormat {
    Json,
    Yaml,
    Text,
    Html,
}

impl std::fmt::Display for ReportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportFormat::Json => write!(f, "json"),
            ReportFormat::Yaml => write!(f, "yaml"),
            ReportFormat::Text => write!(f, "text"),
            ReportFormat::Html => write!(f, "html"),
        }
    }
}

/// Overall diagnostic status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OverallStatus {
    Healthy,
    MinorIssues,
    MajorIssues,
    CriticalIssues,
    ConnectionImpossible,
}

impl OverallStatus {
    /// Determine overall status from diagnostic results
    pub fn from_results(results: &[DiagnosticResult]) -> Self {
        let critical_count = results.iter()
            .filter(|r| r.severity == Severity::Critical && r.status == DiagnosticStatus::Error)
            .count();
        
        let high_error_count = results.iter()
            .filter(|r| r.severity == Severity::High && r.status == DiagnosticStatus::Error)
            .count();
        
        let error_count = results.iter()
            .filter(|r| r.status == DiagnosticStatus::Error)
            .count();
        
        let warning_count = results.iter()
            .filter(|r| r.status == DiagnosticStatus::Warning)
            .count();

        if critical_count > 0 || high_error_count >= 3 {
            OverallStatus::ConnectionImpossible
        } else if high_error_count > 0 || error_count >= 2 {
            OverallStatus::CriticalIssues
        } else if error_count > 0 {
            OverallStatus::MajorIssues
        } else if warning_count > 0 {
            OverallStatus::MinorIssues
        } else {
            OverallStatus::Healthy
        }
    }
}

/// Summary of diagnostic results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    pub total_checks: usize,
    pub passed: usize,
    pub warnings: usize,
    pub errors: usize,
    pub critical_issues: usize,
    pub auto_fixable_issues: usize,
    pub total_duration: Duration,
}

impl DiagnosticSummary {
    /// Create summary from diagnostic results
    pub fn from_results(results: &[DiagnosticResult]) -> Self {
        let total_checks = results.len();
        let passed = results.iter().filter(|r| r.status == DiagnosticStatus::Success).count();
        let warnings = results.iter().filter(|r| r.status == DiagnosticStatus::Warning).count();
        let errors = results.iter().filter(|r| r.status == DiagnosticStatus::Error).count();
        let critical_issues = results.iter()
            .filter(|r| r.severity == Severity::Critical && r.status == DiagnosticStatus::Error)
            .count();
        let auto_fixable_issues = results.iter().filter(|r| r.auto_fixable).count();
        let total_duration = results.iter().map(|r| r.duration).sum();

        Self {
            total_checks,
            passed,
            warnings,
            errors,
            critical_issues,
            auto_fixable_issues,
            total_duration,
        }
    }
}

/// AWS API error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsApiError {
    pub service: String,
    pub operation: String,
    pub error_code: Option<String>,
    pub error_message: String,
    pub request_id: Option<String>,
    pub timestamp: SystemTime,
    pub retry_count: u32,
}

/// Network diagnostic details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDiagnosticDetails {
    pub target: String,
    pub test_type: String,
    pub latency_ms: Option<u64>,
    pub packet_loss_percent: Option<f64>,
    pub success: bool,
    pub error_message: Option<String>,
    pub routing_info: Option<HashMap<String, serde_json::Value>>,
}

/// Comprehensive diagnostic report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub report_id: String,
    pub timestamp: SystemTime,
    pub config: DiagnosticConfig,
    pub summary: DiagnosticSummary,
    pub results: Vec<DiagnosticResult>,
    pub fix_suggestions: Vec<FixSuggestion>,
    pub auto_fixes_applied: Vec<FixResult>,
    pub overall_status: OverallStatus,
    pub aws_api_errors: Vec<AwsApiError>,
    pub network_diagnostics: Vec<NetworkDiagnosticDetails>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DiagnosticReport {
    /// Create a new diagnostic report
    pub fn new(
        config: DiagnosticConfig,
        results: Vec<DiagnosticResult>,
        fix_suggestions: Vec<FixSuggestion>,
        auto_fixes_applied: Vec<FixResult>,
    ) -> Self {
        let report_id = Uuid::new_v4().to_string();
        let timestamp = SystemTime::now();
        let summary = DiagnosticSummary::from_results(&results);
        let overall_status = OverallStatus::from_results(&results);

        Self {
            report_id,
            timestamp,
            config,
            summary,
            results,
            fix_suggestions,
            auto_fixes_applied,
            overall_status,
            aws_api_errors: Vec::new(),
            network_diagnostics: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add AWS API error to the report
    pub fn add_aws_api_error(&mut self, error: AwsApiError) {
        self.aws_api_errors.push(error);
    }

    /// Add network diagnostic details
    pub fn add_network_diagnostic(&mut self, diagnostic: NetworkDiagnosticDetails) {
        self.network_diagnostics.push(diagnostic);
    }

    /// Add metadata to the report
    pub fn add_metadata(&mut self, key: String, value: serde_json::Value) {
        self.metadata.insert(key, value);
    }

    /// Get report file extension based on format
    pub fn get_file_extension(format: &ReportFormat) -> &'static str {
        match format {
            ReportFormat::Json => "json",
            ReportFormat::Yaml => "yaml",
            ReportFormat::Text => "txt",
            ReportFormat::Html => "html",
        }
    }

    /// Generate filename for the report
    pub fn generate_filename(&self, format: &ReportFormat) -> String {
        let timestamp = self.timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        format!(
            "ssm-diagnostic-report-{}-{}.{}",
            self.config.instance_id,
            timestamp,
            Self::get_file_extension(format)
        )
    }
}

/// Report manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportManagerConfig {
    pub output_directory: PathBuf,
    pub default_format: ReportFormat,
    pub auto_save: bool,
    pub include_sensitive_data: bool,
    pub max_report_age_days: u32,
}

impl Default for ReportManagerConfig {
    fn default() -> Self {
        // Use standard data directory for reports
        let reports_dir = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
            .map(|d| d.join("ec2-connect").join("reports"))
            .unwrap_or_else(|| PathBuf::from("reports"));
            
        Self {
            output_directory: reports_dir,
            default_format: ReportFormat::Json,
            auto_save: true,
            include_sensitive_data: false,
            max_report_age_days: 30,
        }
    }
}

/// Report manager for generating and saving diagnostic reports
pub struct ReportManager {
    config: ReportManagerConfig,
}

impl ReportManager {
    /// Create a new report manager with default configuration
    pub fn new() -> Self {
        Self {
            config: ReportManagerConfig::default(),
        }
    }

    /// Create a new report manager with custom configuration
    pub fn with_config(config: ReportManagerConfig) -> Self {
        Self { config }
    }

    /// Generate a comprehensive diagnostic report
    pub async fn generate_report(
        &self,
        diagnostic_config: DiagnosticConfig,
        results: Vec<DiagnosticResult>,
        fix_suggestions: Vec<FixSuggestion>,
        auto_fixes_applied: Vec<FixResult>,
    ) -> anyhow::Result<DiagnosticReport> {
        info!("Generating diagnostic report for instance: {}", diagnostic_config.instance_id);
        
        let mut report = DiagnosticReport::new(
            diagnostic_config,
            results,
            fix_suggestions,
            auto_fixes_applied,
        );

        // Add system metadata
        report.add_metadata("system_info".to_string(), serde_json::json!({
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "rust_version": env!("CARGO_PKG_VERSION"),
        }));

        // Add AWS region info if available
        if let Some(region) = &report.config.region {
            report.add_metadata("aws_region".to_string(), serde_json::Value::String(region.clone()));
        }

        debug!("Generated report with ID: {}", report.report_id);
        Ok(report)
    }

    /// Save report to file in specified format
    pub async fn save_report(
        &self,
        report: &DiagnosticReport,
        format: ReportFormat,
        custom_path: Option<PathBuf>,
    ) -> anyhow::Result<PathBuf> {
        // Ensure output directory exists
        fs::create_dir_all(&self.config.output_directory).await?;

        let file_path = if let Some(path) = custom_path {
            path
        } else {
            self.config.output_directory.join(report.generate_filename(&format))
        };

        info!("Saving report to: {}", file_path.display());

        let content = match format {
            ReportFormat::Json => self.format_as_json(report)?,
            ReportFormat::Yaml => self.format_as_yaml(report)?,
            ReportFormat::Text => self.format_as_text(report),
            ReportFormat::Html => self.format_as_html(report),
        };

        fs::write(&file_path, content).await?;
        info!("Report saved successfully to: {}", file_path.display());

        Ok(file_path)
    }

    /// Save report in multiple formats
    pub async fn save_report_multiple_formats(
        &self,
        report: &DiagnosticReport,
        formats: Vec<ReportFormat>,
    ) -> anyhow::Result<Vec<PathBuf>> {
        let mut saved_paths = Vec::new();

        for format in formats {
            match self.save_report(report, format.clone(), None).await {
                Ok(path) => saved_paths.push(path),
                Err(e) => {
                    error!("Failed to save report in {} format: {}", format, e);
                    return Err(e);
                }
            }
        }

        Ok(saved_paths)
    }

    /// Format report as JSON
    fn format_as_json(&self, report: &DiagnosticReport) -> anyhow::Result<String> {
        let json = if self.config.include_sensitive_data {
            serde_json::to_string_pretty(report)?
        } else {
            let sanitized_report = self.sanitize_report(report);
            serde_json::to_string_pretty(&sanitized_report)?
        };
        Ok(json)
    }

    /// Format report as YAML
    fn format_as_yaml(&self, report: &DiagnosticReport) -> anyhow::Result<String> {
        let report_to_serialize = if self.config.include_sensitive_data {
            report
        } else {
            &self.sanitize_report(report)
        };
        
        let yaml = serde_yaml::to_string(report_to_serialize)?;
        Ok(yaml)
    }

    /// Format report as plain text
    fn format_as_text(&self, report: &DiagnosticReport) -> String {
        let mut output = String::new();
        
        // Header
        output.push_str("=".repeat(80).as_str());
        output.push_str("\nSSM Connection Diagnostic Report\n");
        output.push_str("=".repeat(80).as_str());
        output.push('\n');
        
        // Basic info
        output.push_str(&format!("Report ID: {}\n", report.report_id));
        output.push_str(&format!("Instance ID: {}\n", report.config.instance_id));
        output.push_str(&format!("Timestamp: {:?}\n", report.timestamp));
        output.push_str(&format!("Overall Status: {:?}\n", report.overall_status));
        output.push('\n');
        
        // Summary
        output.push_str("SUMMARY\n");
        output.push_str("-".repeat(40).as_str());
        output.push('\n');
        output.push_str(&format!("Total Checks: {}\n", report.summary.total_checks));
        output.push_str(&format!("Passed: {}\n", report.summary.passed));
        output.push_str(&format!("Warnings: {}\n", report.summary.warnings));
        output.push_str(&format!("Errors: {}\n", report.summary.errors));
        output.push_str(&format!("Critical Issues: {}\n", report.summary.critical_issues));
        output.push_str(&format!("Auto-fixable Issues: {}\n", report.summary.auto_fixable_issues));
        output.push_str(&format!("Total Duration: {:?}\n", report.summary.total_duration));
        output.push('\n');
        
        // Diagnostic results
        output.push_str("DIAGNOSTIC RESULTS\n");
        output.push_str("-".repeat(40).as_str());
        output.push('\n');
        
        for result in &report.results {
            let status_icon = match result.status {
                DiagnosticStatus::Success => "✅",
                DiagnosticStatus::Warning => "⚠️",
                DiagnosticStatus::Error => "❌",
                DiagnosticStatus::Skipped => "⏭️",
            };
            
            output.push_str(&format!(
                "{} {} [{}] - {} ({:?})\n",
                status_icon,
                result.item_name,
                format!("{:?}", result.severity),
                result.message,
                result.duration
            ));
        }
        output.push('\n');
        
        // Fix suggestions
        if !report.fix_suggestions.is_empty() {
            output.push_str("FIX SUGGESTIONS\n");
            output.push_str("-".repeat(40).as_str());
            output.push('\n');
            
            for (i, suggestion) in report.fix_suggestions.iter().enumerate() {
                output.push_str(&format!("{}. {} [{}]\n", i + 1, suggestion.title, format!("{:?}", suggestion.severity)));
                output.push_str(&format!("   Description: {}\n", suggestion.description));
                if !suggestion.steps.is_empty() {
                    output.push_str("   Steps:\n");
                    for (j, step) in suggestion.steps.iter().enumerate() {
                        output.push_str(&format!("     {}. {}\n", j + 1, step));
                    }
                }
                output.push('\n');
            }
        }
        
        // Applied fixes
        if !report.auto_fixes_applied.is_empty() {
            output.push_str("APPLIED AUTO-FIXES\n");
            output.push_str("-".repeat(40).as_str());
            output.push('\n');
            
            for fix in &report.auto_fixes_applied {
                let status_icon = if fix.success { "✅" } else { "❌" };
                output.push_str(&format!(
                    "{} {} - {}\n",
                    status_icon,
                    fix.action.description,
                    fix.message
                ));
            }
            output.push('\n');
        }
        
        // AWS API errors
        if !report.aws_api_errors.is_empty() {
            output.push_str("AWS API ERRORS\n");
            output.push_str("-".repeat(40).as_str());
            output.push('\n');
            
            for error in &report.aws_api_errors {
                output.push_str(&format!("Service: {} | Operation: {}\n", error.service, error.operation));
                if let Some(code) = &error.error_code {
                    output.push_str(&format!("Error Code: {}\n", code));
                }
                output.push_str(&format!("Message: {}\n", error.error_message));
                if let Some(request_id) = &error.request_id {
                    output.push_str(&format!("Request ID: {}\n", request_id));
                }
                output.push_str(&format!("Retry Count: {}\n", error.retry_count));
                output.push('\n');
            }
        }
        
        // Network diagnostics
        if !report.network_diagnostics.is_empty() {
            output.push_str("NETWORK DIAGNOSTICS\n");
            output.push_str("-".repeat(40).as_str());
            output.push('\n');
            
            for diag in &report.network_diagnostics {
                let status_icon = if diag.success { "✅" } else { "❌" };
                output.push_str(&format!("{} {} ({})\n", status_icon, diag.target, diag.test_type));
                
                if let Some(latency) = diag.latency_ms {
                    output.push_str(&format!("  Latency: {}ms\n", latency));
                }
                if let Some(packet_loss) = diag.packet_loss_percent {
                    output.push_str(&format!("  Packet Loss: {:.2}%\n", packet_loss));
                }
                if let Some(error) = &diag.error_message {
                    output.push_str(&format!("  Error: {}\n", error));
                }
                output.push('\n');
            }
        }
        
        output.push_str("=".repeat(80).as_str());
        output.push('\n');
        
        output
    }

    /// Format report as HTML
    fn format_as_html(&self, report: &DiagnosticReport) -> String {
        let mut html = String::new();
        
        // HTML header
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<title>SSM Connection Diagnostic Report</title>\n");
        html.push_str("<style>\n");
        html.push_str("body { font-family: Arial, sans-serif; margin: 20px; }\n");
        html.push_str(".header { background-color: #f0f0f0; padding: 20px; border-radius: 5px; }\n");
        html.push_str(".summary { margin: 20px 0; }\n");
        html.push_str(".result { margin: 10px 0; padding: 10px; border-left: 4px solid #ccc; }\n");
        html.push_str(".success { border-left-color: #4CAF50; }\n");
        html.push_str(".warning { border-left-color: #FF9800; }\n");
        html.push_str(".error { border-left-color: #F44336; }\n");
        html.push_str(".skipped { border-left-color: #9E9E9E; }\n");
        html.push_str("table { border-collapse: collapse; width: 100%; }\n");
        html.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n");
        html.push_str("th { background-color: #f2f2f2; }\n");
        html.push_str("</style>\n");
        html.push_str("</head>\n<body>\n");
        
        // Header
        html.push_str("<div class=\"header\">\n");
        html.push_str("<h1>SSM Connection Diagnostic Report</h1>\n");
        html.push_str(&format!("<p><strong>Report ID:</strong> {}</p>\n", report.report_id));
        html.push_str(&format!("<p><strong>Instance ID:</strong> {}</p>\n", report.config.instance_id));
        html.push_str(&format!("<p><strong>Timestamp:</strong> {:?}</p>\n", report.timestamp));
        html.push_str(&format!("<p><strong>Overall Status:</strong> {:?}</p>\n", report.overall_status));
        html.push_str("</div>\n");
        
        // Summary
        html.push_str("<div class=\"summary\">\n");
        html.push_str("<h2>Summary</h2>\n");
        html.push_str("<table>\n");
        html.push_str("<tr><th>Metric</th><th>Value</th></tr>\n");
        html.push_str(&format!("<tr><td>Total Checks</td><td>{}</td></tr>\n", report.summary.total_checks));
        html.push_str(&format!("<tr><td>Passed</td><td>{}</td></tr>\n", report.summary.passed));
        html.push_str(&format!("<tr><td>Warnings</td><td>{}</td></tr>\n", report.summary.warnings));
        html.push_str(&format!("<tr><td>Errors</td><td>{}</td></tr>\n", report.summary.errors));
        html.push_str(&format!("<tr><td>Critical Issues</td><td>{}</td></tr>\n", report.summary.critical_issues));
        html.push_str(&format!("<tr><td>Auto-fixable Issues</td><td>{}</td></tr>\n", report.summary.auto_fixable_issues));
        html.push_str(&format!("<tr><td>Total Duration</td><td>{:?}</td></tr>\n", report.summary.total_duration));
        html.push_str("</table>\n");
        html.push_str("</div>\n");
        
        // Diagnostic results
        html.push_str("<h2>Diagnostic Results</h2>\n");
        for result in &report.results {
            let css_class = match result.status {
                DiagnosticStatus::Success => "result success",
                DiagnosticStatus::Warning => "result warning",
                DiagnosticStatus::Error => "result error",
                DiagnosticStatus::Skipped => "result skipped",
            };
            
            let status_icon = match result.status {
                DiagnosticStatus::Success => "✅",
                DiagnosticStatus::Warning => "⚠️",
                DiagnosticStatus::Error => "❌",
                DiagnosticStatus::Skipped => "⏭️",
            };
            
            html.push_str(&format!("<div class=\"{}\">\n", css_class));
            html.push_str(&format!("<h3>{} {} [{}]</h3>\n", status_icon, result.item_name, format!("{:?}", result.severity)));
            html.push_str(&format!("<p>{}</p>\n", result.message));
            html.push_str(&format!("<p><small>Duration: {:?}</small></p>\n", result.duration));
            html.push_str("</div>\n");
        }
        
        // Fix suggestions
        if !report.fix_suggestions.is_empty() {
            html.push_str("<h2>Fix Suggestions</h2>\n");
            for (i, suggestion) in report.fix_suggestions.iter().enumerate() {
                html.push_str("<div class=\"result\">\n");
                html.push_str(&format!("<h3>{}. {} [{}]</h3>\n", i + 1, suggestion.title, format!("{:?}", suggestion.severity)));
                html.push_str(&format!("<p>{}</p>\n", suggestion.description));
                if !suggestion.steps.is_empty() {
                    html.push_str("<ol>\n");
                    for step in &suggestion.steps {
                        html.push_str(&format!("<li>{}</li>\n", step));
                    }
                    html.push_str("</ol>\n");
                }
                html.push_str("</div>\n");
            }
        }
        
        // Applied fixes
        if !report.auto_fixes_applied.is_empty() {
            html.push_str("<h2>Applied Auto-fixes</h2>\n");
            for fix in &report.auto_fixes_applied {
                let status_icon = if fix.success { "✅" } else { "❌" };
                html.push_str("<div class=\"result\">\n");
                html.push_str(&format!("<h3>{} {}</h3>\n", status_icon, fix.action.description));
                html.push_str(&format!("<p>{}</p>\n", fix.message));
                html.push_str("</div>\n");
            }
        }
        
        html.push_str("</body>\n</html>\n");
        html
    }

    /// Sanitize report by removing sensitive information
    fn sanitize_report(&self, report: &DiagnosticReport) -> DiagnosticReport {
        let mut sanitized = report.clone();
        
        // Remove sensitive AWS details
        for error in &mut sanitized.aws_api_errors {
            error.request_id = None;
        }
        
        // Remove sensitive metadata
        sanitized.metadata.retain(|key, _| {
            !key.to_lowercase().contains("secret") &&
            !key.to_lowercase().contains("password") &&
            !key.to_lowercase().contains("token")
        });
        
        // Sanitize diagnostic result details
        for result in &mut sanitized.results {
            if let Some(details) = &mut result.details {
                if let Some(obj) = details.as_object_mut() {
                    obj.retain(|key, _| {
                        !key.to_lowercase().contains("secret") &&
                        !key.to_lowercase().contains("password") &&
                        !key.to_lowercase().contains("token")
                    });
                }
            }
        }
        
        sanitized
    }

    /// Clean up old reports based on configuration
    pub async fn cleanup_old_reports(&self) -> anyhow::Result<usize> {
        info!("Cleaning up old reports older than {} days", self.config.max_report_age_days);
        
        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(self.config.max_report_age_days as u64 * 24 * 60 * 60))
            .unwrap_or(SystemTime::UNIX_EPOCH);
        
        let mut deleted_count = 0;
        let mut entries = fs::read_dir(&self.config.output_directory).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Check if it's a report file
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("ssm-diagnostic-report-") {
                    if let Ok(metadata) = entry.metadata().await {
                        if let Ok(modified) = metadata.modified() {
                            if modified < cutoff_time {
                                match fs::remove_file(&path).await {
                                    Ok(_) => {
                                        debug!("Deleted old report: {}", path.display());
                                        deleted_count += 1;
                                    }
                                    Err(e) => {
                                        warn!("Failed to delete old report {}: {}", path.display(), e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        info!("Cleaned up {} old reports", deleted_count);
        Ok(deleted_count)
    }

    /// Send notification about report generation
    pub async fn send_notification(&self, report_path: &Path, report: &DiagnosticReport) -> anyhow::Result<()> {
        info!("Sending notification for report: {}", report_path.display());
        
        let notification_message = format!(
            "SSM Diagnostic Report Generated\n\
            Instance: {}\n\
            Status: {:?}\n\
            Issues: {} errors, {} warnings\n\
            Report saved to: {}",
            report.config.instance_id,
            report.overall_status,
            report.summary.errors,
            report.summary.warnings,
            report_path.display()
        );
        
        // For now, just log the notification
        // In a real implementation, this could send desktop notifications,
        // emails, or integrate with notification services
        info!("Notification: {}", notification_message);
        
        Ok(())
    }

    /// Get report statistics
    pub async fn get_report_statistics(&self) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        let mut stats = HashMap::new();
        let mut total_reports = 0;
        let mut total_size = 0u64;
        
        let mut entries = fs::read_dir(&self.config.output_directory).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("ssm-diagnostic-report-") {
                    total_reports += 1;
                    
                    if let Ok(metadata) = entry.metadata().await {
                        total_size += metadata.len();
                    }
                }
            }
        }
        
        stats.insert("total_reports".to_string(), serde_json::Value::Number(total_reports.into()));
        stats.insert("total_size_bytes".to_string(), serde_json::Value::Number(total_size.into()));
        stats.insert("output_directory".to_string(), serde_json::Value::String(self.config.output_directory.display().to_string()));
        
        Ok(stats)
    }
}

impl Default for ReportManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_diagnostic_result() -> DiagnosticResult {
        DiagnosticResult {
            item_name: "test_item".to_string(),
            status: DiagnosticStatus::Success,
            message: "Test successful".to_string(),
            details: None,
            duration: Duration::from_millis(100),
            severity: Severity::Info,
            auto_fixable: false,
        }
    }

    fn create_test_config() -> DiagnosticConfig {
        DiagnosticConfig::new("i-1234567890abcdef0".to_string())
    }

    #[test]
    fn test_overall_status_determination() {
        let results = vec![
            DiagnosticResult::success("test1".to_string(), "OK".to_string(), Duration::from_millis(100)),
            DiagnosticResult::warning("test2".to_string(), "Warning".to_string(), Duration::from_millis(100), Severity::Medium),
        ];
        
        let status = OverallStatus::from_results(&results);
        assert_eq!(status, OverallStatus::MinorIssues);
    }

    #[test]
    fn test_diagnostic_summary_creation() {
        let results = vec![
            DiagnosticResult::success("test1".to_string(), "OK".to_string(), Duration::from_millis(100)),
            DiagnosticResult::error("test2".to_string(), "Error".to_string(), Duration::from_millis(200), Severity::High),
        ];
        
        let summary = DiagnosticSummary::from_results(&results);
        assert_eq!(summary.total_checks, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.total_duration, Duration::from_millis(300));
    }

    #[test]
    fn test_report_creation() {
        let config = create_test_config();
        let results = vec![create_test_diagnostic_result()];
        let fix_suggestions = vec![];
        let auto_fixes_applied = vec![];
        
        let report = DiagnosticReport::new(config, results, fix_suggestions, auto_fixes_applied);
        
        assert!(!report.report_id.is_empty());
        assert_eq!(report.config.instance_id, "i-1234567890abcdef0");
        assert_eq!(report.summary.total_checks, 1);
        assert_eq!(report.overall_status, OverallStatus::Healthy);
    }

    #[test]
    fn test_report_filename_generation() {
        let config = create_test_config();
        let results = vec![create_test_diagnostic_result()];
        let report = DiagnosticReport::new(config, results, vec![], vec![]);
        
        let filename = report.generate_filename(&ReportFormat::Json);
        assert!(filename.starts_with("ssm-diagnostic-report-i-1234567890abcdef0-"));
        assert!(filename.ends_with(".json"));
    }

    #[tokio::test]
    async fn test_report_manager_creation() {
        let manager = ReportManager::new();
        assert_eq!(manager.config.default_format, ReportFormat::Json);
        assert!(manager.config.auto_save);
    }

    #[tokio::test]
    async fn test_report_generation() {
        let manager = ReportManager::new();
        let config = create_test_config();
        let results = vec![create_test_diagnostic_result()];
        
        let report = manager.generate_report(config, results, vec![], vec![]).await.unwrap();
        
        assert!(!report.report_id.is_empty());
        assert_eq!(report.config.instance_id, "i-1234567890abcdef0");
        assert!(report.metadata.contains_key("system_info"));
    }

    #[tokio::test]
    async fn test_report_saving() {
        let temp_dir = TempDir::new().unwrap();
        let config = ReportManagerConfig {
            output_directory: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let manager = ReportManager::with_config(config);
        let diagnostic_config = create_test_config();
        let results = vec![create_test_diagnostic_result()];
        
        let report = manager.generate_report(diagnostic_config, results, vec![], vec![]).await.unwrap();
        let saved_path = manager.save_report(&report, ReportFormat::Json, None).await.unwrap();
        
        assert!(saved_path.exists());
        assert!(saved_path.extension().unwrap() == "json");
    }

    #[test]
    fn test_text_formatting() {
        let manager = ReportManager::new();
        let config = create_test_config();
        let results = vec![create_test_diagnostic_result()];
        let report = DiagnosticReport::new(config, results, vec![], vec![]);
        
        let text_output = manager.format_as_text(&report);
        
        assert!(text_output.contains("SSM Connection Diagnostic Report"));
        assert!(text_output.contains("i-1234567890abcdef0"));
        assert!(text_output.contains("test_item"));
    }

    #[test]
    fn test_html_formatting() {
        let manager = ReportManager::new();
        let config = create_test_config();
        let results = vec![create_test_diagnostic_result()];
        let report = DiagnosticReport::new(config, results, vec![], vec![]);
        
        let html_output = manager.format_as_html(&report);
        
        assert!(html_output.contains("<!DOCTYPE html>"));
        assert!(html_output.contains("SSM Connection Diagnostic Report"));
        assert!(html_output.contains("i-1234567890abcdef0"));
    }

    #[test]
    fn test_report_sanitization() {
        let manager = ReportManager::with_config(ReportManagerConfig {
            include_sensitive_data: false,
            ..Default::default()
        });
        
        let config = create_test_config();
        let results = vec![create_test_diagnostic_result()];
        let mut report = DiagnosticReport::new(config, results, vec![], vec![]);
        
        // Add sensitive data
        report.add_metadata("secret_key".to_string(), serde_json::Value::String("sensitive".to_string()));
        report.add_metadata("normal_key".to_string(), serde_json::Value::String("normal".to_string()));
        
        let sanitized = manager.sanitize_report(&report);
        
        assert!(!sanitized.metadata.contains_key("secret_key"));
        assert!(sanitized.metadata.contains_key("normal_key"));
    }
}