use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug};

use crate::diagnostic::{DiagnosticResult, DiagnosticStatus, Severity};
use crate::auto_fix::{FixAction, FixActionType, RiskLevel};
use crate::error::Result;

/// Problem severity classification for suggestions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub enum ProblemSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl From<Severity> for ProblemSeverity {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Critical => ProblemSeverity::Critical,
            Severity::High => ProblemSeverity::High,
            Severity::Medium => ProblemSeverity::Medium,
            Severity::Low => ProblemSeverity::Low,
            Severity::Info => ProblemSeverity::Low,
        }
    }
}

/// A detailed suggestion for fixing a problem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub problem_type: String,
    pub severity: ProblemSeverity,
    pub title: String,
    pub description: String,
    pub steps: Vec<String>,
    pub cli_commands: Vec<String>,
    pub aws_console_steps: Vec<String>,
    pub code_examples: HashMap<String, String>,
    pub prerequisites: Vec<String>,
    pub estimated_time: String,
    pub risk_assessment: String,
    pub verification_steps: Vec<String>,
}

impl FixSuggestion {
    pub fn new(
        problem_type: String,
        severity: ProblemSeverity,
        title: String,
        description: String,
    ) -> Self {
        Self {
            problem_type,
            severity,
            title,
            description,
            steps: Vec::new(),
            cli_commands: Vec::new(),
            aws_console_steps: Vec::new(),
            code_examples: HashMap::new(),
            prerequisites: Vec::new(),
            estimated_time: "Unknown".to_string(),
            risk_assessment: "Low risk".to_string(),
            verification_steps: Vec::new(),
        }
    }

    pub fn with_steps(mut self, steps: Vec<String>) -> Self {
        self.steps = steps;
        self
    }

    pub fn with_cli_commands(mut self, commands: Vec<String>) -> Self {
        self.cli_commands = commands;
        self
    }

    pub fn with_console_steps(mut self, steps: Vec<String>) -> Self {
        self.aws_console_steps = steps;
        self
    }

    pub fn with_code_example(mut self, language: String, code: String) -> Self {
        self.code_examples.insert(language, code);
        self
    }

    pub fn with_prerequisites(mut self, prerequisites: Vec<String>) -> Self {
        self.prerequisites = prerequisites;
        self
    }

    pub fn with_estimated_time(mut self, time: String) -> Self {
        self.estimated_time = time;
        self
    }

    pub fn with_risk_assessment(mut self, risk: String) -> Self {
        self.risk_assessment = risk;
        self
    }

    pub fn with_verification_steps(mut self, steps: Vec<String>) -> Self {
        self.verification_steps = steps;
        self
    }
}

/// Trait for suggestion generator implementations
pub trait SuggestionGenerator {
    /// Generate detailed fix suggestions from diagnostic results
    fn generate_suggestions(&self, diagnostics: &[DiagnosticResult]) -> Result<Vec<FixSuggestion>>;
    
    /// Generate IAM policy JSON examples
    fn generate_iam_policy_json(&self, required_permissions: &[String]) -> String;
    
    /// Generate VPC endpoint creation CLI commands
    fn generate_vpc_endpoint_commands(&self, region: &str, vpc_id: &str) -> Vec<String>;
    
    /// Generate security group configuration examples
    fn generate_security_group_examples(&self, group_id: &str) -> FixSuggestion;
    
    /// Generate SSM agent fix commands
    fn generate_ssm_agent_commands(&self, platform: &str) -> Vec<String>;
    
    /// Classify problem severity
    fn classify_problem_severity(&self, result: &DiagnosticResult) -> ProblemSeverity;
}

/// Default implementation of the suggestion generator
pub struct DefaultSuggestionGenerator {
    region: String,
}

impl DefaultSuggestionGenerator {
    pub fn new(region: String) -> Self {
        Self { region }
    }

    /// Generate instance-related suggestions
    fn generate_instance_suggestions(&self, result: &DiagnosticResult) -> Vec<FixSuggestion> {
        let mut suggestions = Vec::new();
        let severity = self.classify_problem_severity(result);

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("stopped") || result.message.contains("stopping") {
                let instance_id = result.details.as_ref()
                    .and_then(|d| d.get("instance_id"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("i-xxxxxxxxxxxxxxxxx");

                let suggestion = FixSuggestion::new(
                    "instance_state".to_string(),
                    severity,
                    "Start Stopped EC2 Instance".to_string(),
                    "The target EC2 instance is currently stopped and needs to be started before establishing an SSM connection.".to_string(),
                )
                .with_steps(vec![
                    "Verify the instance ID is correct".to_string(),
                    "Check if you have permission to start the instance".to_string(),
                    "Start the instance using AWS Console or CLI".to_string(),
                    "Wait for the instance to reach 'running' state".to_string(),
                    "Verify SSM agent is running after instance start".to_string(),
                ])
                .with_cli_commands(vec![
                    format!("aws ec2 start-instances --instance-ids {}", instance_id),
                    format!("aws ec2 describe-instances --instance-ids {} --query 'Reservations[0].Instances[0].State.Name'", instance_id),
                ])
                .with_console_steps(vec![
                    "Open the AWS EC2 Console".to_string(),
                    "Navigate to 'Instances' in the left sidebar".to_string(),
                    format!("Find and select instance: {}", instance_id),
                    "Click 'Instance state' dropdown".to_string(),
                    "Select 'Start instance'".to_string(),
                    "Wait for the instance state to change to 'running'".to_string(),
                ])
                .with_prerequisites(vec![
                    "AWS CLI configured with appropriate credentials".to_string(),
                    "ec2:StartInstances permission".to_string(),
                    "ec2:DescribeInstances permission".to_string(),
                ])
                .with_estimated_time("2-5 minutes".to_string())
                .with_risk_assessment("Low risk - Starting an instance is a safe operation".to_string())
                .with_verification_steps(vec![
                    format!("Run: aws ec2 describe-instances --instance-ids {}", instance_id),
                    "Verify the instance state shows 'running'".to_string(),
                    "Test SSM connectivity after instance is running".to_string(),
                ]);

                suggestions.push(suggestion);
            } else if result.message.contains("terminated") {
                let suggestion = FixSuggestion::new(
                    "instance_state".to_string(),
                    ProblemSeverity::Critical,
                    "Instance Terminated - Launch New Instance".to_string(),
                    "The target instance has been terminated and cannot be recovered. You need to launch a new instance.".to_string(),
                )
                .with_steps(vec![
                    "Launch a new EC2 instance with the same configuration".to_string(),
                    "Ensure the new instance has an IAM role with SSM permissions".to_string(),
                    "Install and configure SSM agent if not using an AWS-managed AMI".to_string(),
                    "Update your connection configuration with the new instance ID".to_string(),
                ])
                .with_cli_commands(vec![
                    "aws ec2 run-instances --image-id ami-xxxxxxxxx --instance-type t3.micro --iam-instance-profile Name=SSMInstanceProfile".to_string(),
                ])
                .with_estimated_time("10-15 minutes".to_string())
                .with_risk_assessment("Medium risk - Launching new resources incurs costs".to_string());

                suggestions.push(suggestion);
            }
        }

        suggestions
    }

    /// Generate IAM-related suggestions
    fn generate_iam_suggestions(&self, result: &DiagnosticResult) -> Vec<FixSuggestion> {
        let mut suggestions = Vec::new();
        let severity = self.classify_problem_severity(result);

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("credentials") || result.message.contains("authentication") {
                let suggestion = FixSuggestion::new(
                    "iam_credentials".to_string(),
                    severity,
                    "Fix AWS Credentials Configuration".to_string(),
                    "AWS credentials are not properly configured or have expired.".to_string(),
                )
                .with_steps(vec![
                    "Check current AWS credentials configuration".to_string(),
                    "Verify credentials are not expired".to_string(),
                    "Update credentials using AWS CLI or environment variables".to_string(),
                    "Test credentials with a simple AWS API call".to_string(),
                ])
                .with_cli_commands(vec![
                    "aws configure list".to_string(),
                    "aws sts get-caller-identity".to_string(),
                    "aws configure".to_string(),
                    "aws sso login".to_string(),
                ])
                .with_prerequisites(vec![
                    "Valid AWS access keys or SSO configuration".to_string(),
                    "AWS CLI installed".to_string(),
                ])
                .with_estimated_time("5-10 minutes".to_string())
                .with_verification_steps(vec![
                    "Run: aws sts get-caller-identity".to_string(),
                    "Verify the output shows your expected AWS account and user/role".to_string(),
                ]);

                suggestions.push(suggestion);
            }

            if result.message.contains("permissions") || result.message.contains("access denied") {
                let required_permissions = vec![
                    "ssm:UpdateInstanceInformation".to_string(),
                    "ssm:SendCommand".to_string(),
                    "ssm:ListCommands".to_string(),
                    "ssm:ListCommandInvocations".to_string(),
                    "ssm:DescribeInstanceInformation".to_string(),
                    "ssm:GetConnectionStatus".to_string(),
                    "ssm:StartSession".to_string(),
                    "ssm:TerminateSession".to_string(),
                    "ec2messages:*".to_string(),
                    "ssmmessages:*".to_string(),
                ];

                let policy_json = self.generate_iam_policy_json(&required_permissions);

                let suggestion = FixSuggestion::new(
                    "iam_permissions".to_string(),
                    severity,
                    "Add Required IAM Permissions".to_string(),
                    "The current IAM user/role lacks the necessary permissions for SSM operations.".to_string(),
                )
                .with_steps(vec![
                    "Identify the IAM user or role being used".to_string(),
                    "Create or update an IAM policy with required SSM permissions".to_string(),
                    "Attach the policy to the user or role".to_string(),
                    "For EC2 instances, ensure the instance profile has the required permissions".to_string(),
                    "Test the permissions".to_string(),
                ])
                .with_cli_commands(vec![
                    "aws iam create-policy --policy-name SSMUserPolicy --policy-document file://ssm-policy.json".to_string(),
                    "aws iam attach-user-policy --user-name YourUsername --policy-arn arn:aws:iam::ACCOUNT:policy/SSMUserPolicy".to_string(),
                    "aws iam attach-role-policy --role-name YourRoleName --policy-arn arn:aws:iam::ACCOUNT:policy/SSMUserPolicy".to_string(),
                ])
                .with_console_steps(vec![
                    "Open the AWS IAM Console".to_string(),
                    "Navigate to 'Policies' and click 'Create policy'".to_string(),
                    "Use the JSON editor to paste the policy document".to_string(),
                    "Name the policy (e.g., 'SSMUserPolicy')".to_string(),
                    "Attach the policy to the appropriate user or role".to_string(),
                ])
                .with_code_example("json".to_string(), policy_json)
                .with_prerequisites(vec![
                    "IAM administrative permissions".to_string(),
                    "Knowledge of which user/role needs the permissions".to_string(),
                ])
                .with_estimated_time("10-15 minutes".to_string())
                .with_risk_assessment("Low risk - Adding permissions is generally safe".to_string())
                .with_verification_steps(vec![
                    "aws ssm describe-instance-information".to_string(),
                    "aws ssm start-session --target i-xxxxxxxxxxxxxxxxx".to_string(),
                ]);

                suggestions.push(suggestion);
            }
        }

        suggestions
    }

    /// Generate network-related suggestions
    fn generate_network_suggestions(&self, result: &DiagnosticResult) -> Vec<FixSuggestion> {
        let mut suggestions = Vec::new();
        let severity = self.classify_problem_severity(result);

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("VPC endpoint") {
                let vpc_id = result.details.as_ref()
                    .and_then(|d| d.get("vpc_id"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("vpc-xxxxxxxxxxxxxxxxx");

                let commands = self.generate_vpc_endpoint_commands(&self.region, vpc_id);

                let suggestion = FixSuggestion::new(
                    "vpc_endpoints".to_string(),
                    severity,
                    "Create Required VPC Endpoints".to_string(),
                    "SSM requires VPC endpoints for private subnet connectivity or when internet access is restricted.".to_string(),
                )
                .with_steps(vec![
                    "Identify the VPC and subnets where your instance is located".to_string(),
                    "Create VPC endpoints for SSM services".to_string(),
                    "Configure security groups for the endpoints".to_string(),
                    "Update route tables if necessary".to_string(),
                    "Test connectivity".to_string(),
                ])
                .with_cli_commands(commands)
                .with_console_steps(vec![
                    "Open the VPC Console".to_string(),
                    "Navigate to 'Endpoints' in the left sidebar".to_string(),
                    "Click 'Create endpoint'".to_string(),
                    "Select 'AWS services' and search for SSM services".to_string(),
                    "Create endpoints for: ssm, ssmmessages, ec2messages".to_string(),
                    "Select the appropriate VPC and subnets".to_string(),
                    "Configure security groups to allow HTTPS (port 443)".to_string(),
                ])
                .with_prerequisites(vec![
                    "VPC administrative permissions".to_string(),
                    "Knowledge of VPC and subnet configuration".to_string(),
                ])
                .with_estimated_time("15-30 minutes".to_string())
                .with_risk_assessment("Low risk - VPC endpoints don't affect existing connectivity".to_string())
                .with_verification_steps(vec![
                    "aws ec2 describe-vpc-endpoints".to_string(),
                    "Test SSM connectivity from the instance".to_string(),
                ]);

                suggestions.push(suggestion);
            }

            if result.message.contains("security group") {
                let sg_suggestion = self.generate_security_group_examples("sg-xxxxxxxxxxxxxxxxx");
                suggestions.push(sg_suggestion);
            }
        }

        suggestions
    }

    /// Generate SSM agent-related suggestions
    fn generate_ssm_agent_suggestions(&self, result: &DiagnosticResult) -> Vec<FixSuggestion> {
        let mut suggestions = Vec::new();
        let severity = self.classify_problem_severity(result);

        if result.status == DiagnosticStatus::Error {
            let platform = result.details.as_ref()
                .and_then(|d| d.get("platform"))
                .and_then(|p| p.as_str())
                .unwrap_or("linux");

            let commands = self.generate_ssm_agent_commands(platform);

            let suggestion = FixSuggestion::new(
                "ssm_agent".to_string(),
                severity,
                "Fix SSM Agent Issues".to_string(),
                "The SSM agent is not running properly or is not registered with the SSM service.".to_string(),
            )
            .with_steps(vec![
                "Connect to the instance via SSH or console".to_string(),
                "Check SSM agent status".to_string(),
                "Restart or reinstall the SSM agent if necessary".to_string(),
                "Verify the agent registration".to_string(),
                "Check agent logs for errors".to_string(),
            ])
            .with_cli_commands(commands)
            .with_prerequisites(vec![
                "SSH or console access to the instance".to_string(),
                "Root/administrator privileges on the instance".to_string(),
                "Proper IAM instance profile attached".to_string(),
            ])
            .with_estimated_time("10-20 minutes".to_string())
            .with_risk_assessment("Medium risk - Restarting services may briefly interrupt operations".to_string())
            .with_verification_steps(vec![
                "Check agent status on the instance".to_string(),
                "Verify instance appears in SSM console".to_string(),
                "Test SSM connectivity".to_string(),
            ]);

            suggestions.push(suggestion);
        }

        suggestions
    }

    /// Generate port-related suggestions
    fn generate_port_suggestions(&self, result: &DiagnosticResult) -> Vec<FixSuggestion> {
        let mut suggestions = Vec::new();
        let severity = self.classify_problem_severity(result);

        if result.status == DiagnosticStatus::Error {
            if result.message.contains("port in use") || result.message.contains("already bound") {
                let port = result.details.as_ref()
                    .and_then(|d| d.get("port"))
                    .and_then(|p| p.as_u64())
                    .unwrap_or(8080) as u16;

                let process_name = result.details.as_ref()
                    .and_then(|d| d.get("process_info"))
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");

                let suggestion = FixSuggestion::new(
                    "port_conflict".to_string(),
                    severity,
                    format!("Resolve Port {} Conflict", port),
                    format!("Port {} is currently in use by process '{}' and cannot be used for port forwarding.", port, process_name),
                )
                .with_steps(vec![
                    format!("Identify the process using port {}", port),
                    "Determine if the process can be safely terminated".to_string(),
                    "Either terminate the process or use an alternative port".to_string(),
                    "Test the port availability".to_string(),
                ])
                .with_cli_commands(vec![
                    format!("netstat -tulpn | grep :{}", port),
                    format!("lsof -i :{}", port),
                    "kill -TERM <PID>".to_string(),
                    format!("nc -z localhost {}", port + 1),
                ])
                .with_prerequisites(vec![
                    "Administrative privileges to terminate processes".to_string(),
                    "Knowledge of which processes are safe to terminate".to_string(),
                ])
                .with_estimated_time("5-10 minutes".to_string())
                .with_risk_assessment("Medium risk - Terminating processes may affect other applications".to_string())
                .with_verification_steps(vec![
                    format!("Verify port {} is no longer in use", port),
                    "Test port forwarding with the freed port".to_string(),
                ]);

                suggestions.push(suggestion);

                // Also suggest using alternative ports
                let alt_suggestion = FixSuggestion::new(
                    "port_alternative".to_string(),
                    ProblemSeverity::Low,
                    "Use Alternative Port".to_string(),
                    format!("Instead of resolving the conflict, use a different port for forwarding."),
                )
                .with_steps(vec![
                    "Choose an alternative port number".to_string(),
                    "Update your connection configuration".to_string(),
                    "Test the new port".to_string(),
                ])
                .with_cli_commands(vec![
                    format!("# Try ports {}, {}, {}", port + 1, port + 2, port + 3),
                    format!("nc -z localhost {}", port + 1),
                ])
                .with_estimated_time("2-5 minutes".to_string())
                .with_risk_assessment("No risk - Using different ports is safe".to_string());

                suggestions.push(alt_suggestion);
            }
        }

        suggestions
    }
}

impl SuggestionGenerator for DefaultSuggestionGenerator {
    fn generate_suggestions(&self, diagnostics: &[DiagnosticResult]) -> Result<Vec<FixSuggestion>> {
        info!("Generating detailed fix suggestions for {} diagnostic results", diagnostics.len());
        
        let mut all_suggestions = Vec::new();
        
        for result in diagnostics {
            debug!("Generating suggestions for diagnostic item: {}", result.item_name);
            
            let suggestions = match result.item_name.as_str() {
                "instance_state" => self.generate_instance_suggestions(result),
                "iam_permissions" | "iam_credentials" => self.generate_iam_suggestions(result),
                "vpc_endpoints" | "security_groups" | "network_connectivity" => self.generate_network_suggestions(result),
                "ssm_agent" => self.generate_ssm_agent_suggestions(result),
                "local_port_availability" => self.generate_port_suggestions(result),
                _ => {
                    debug!("No specific suggestions for diagnostic item: {}", result.item_name);
                    Vec::new()
                }
            };
            
            all_suggestions.extend(suggestions);
        }
        
        // Sort suggestions by severity (most critical first)
        all_suggestions.sort_by(|a, b| a.severity.cmp(&b.severity));
        
        info!("Generated {} detailed fix suggestions", all_suggestions.len());
        Ok(all_suggestions)
    }

    fn generate_iam_policy_json(&self, required_permissions: &[String]) -> String {
        let permissions_json = required_permissions
            .iter()
            .map(|p| format!("        \"{}\"", p))
            .collect::<Vec<_>>()
            .join(",\n");

        format!(
            r#"{{
    "Version": "2012-10-17",
    "Statement": [
        {{
            "Effect": "Allow",
            "Action": [
{}
            ],
            "Resource": "*"
        }}
    ]
}}"#,
            permissions_json
        )
    }

    fn generate_vpc_endpoint_commands(&self, region: &str, vpc_id: &str) -> Vec<String> {
        let services = vec!["ssm", "ssmmessages", "ec2messages"];
        let mut commands = Vec::new();

        for service in services {
            commands.push(format!(
                "aws ec2 create-vpc-endpoint --vpc-id {} --service-name com.amazonaws.{}.{} --vpc-endpoint-type Interface --region {}",
                vpc_id, region, service, region
            ));
        }

        commands.push(format!(
            "aws ec2 describe-vpc-endpoints --filters Name=vpc-id,Values={} --region {}",
            vpc_id, region
        ));

        commands
    }

    fn generate_security_group_examples(&self, group_id: &str) -> FixSuggestion {
        FixSuggestion::new(
            "security_group".to_string(),
            ProblemSeverity::High,
            "Configure Security Group for SSM".to_string(),
            "Security group rules need to be updated to allow HTTPS outbound traffic for SSM connectivity.".to_string(),
        )
        .with_steps(vec![
            "Identify the security group attached to your instance".to_string(),
            "Add outbound rule for HTTPS traffic".to_string(),
            "Verify the rule is applied correctly".to_string(),
            "Test SSM connectivity".to_string(),
        ])
        .with_cli_commands(vec![
            format!("aws ec2 describe-security-groups --group-ids {}", group_id),
            format!(
                "aws ec2 authorize-security-group-egress --group-id {} --protocol tcp --port 443 --cidr 0.0.0.0/0",
                group_id
            ),
        ])
        .with_console_steps(vec![
            "Open the EC2 Console".to_string(),
            "Navigate to 'Security Groups'".to_string(),
            format!("Select security group: {}", group_id),
            "Click on the 'Outbound rules' tab".to_string(),
            "Click 'Edit outbound rules'".to_string(),
            "Add rule: Type=HTTPS, Protocol=TCP, Port=443, Destination=0.0.0.0/0".to_string(),
            "Save the changes".to_string(),
        ])
        .with_code_example(
            "terraform".to_string(),
            format!(
                r#"resource "aws_security_group_rule" "ssm_outbound" {{
  type              = "egress"
  from_port         = 443
  to_port           = 443
  protocol          = "tcp"
  cidr_blocks       = ["0.0.0.0/0"]
  security_group_id = "{}"
}}"#,
                group_id
            ),
        )
        .with_prerequisites(vec![
            "EC2 administrative permissions".to_string(),
            "Knowledge of the security group configuration".to_string(),
        ])
        .with_estimated_time("5-10 minutes".to_string())
        .with_risk_assessment("Low risk - Adding outbound rules generally doesn't affect security".to_string())
        .with_verification_steps(vec![
            format!("aws ec2 describe-security-groups --group-ids {}", group_id),
            "Verify the HTTPS outbound rule is present".to_string(),
            "Test SSM connectivity from the instance".to_string(),
        ])
    }

    fn generate_ssm_agent_commands(&self, platform: &str) -> Vec<String> {
        match platform.to_lowercase().as_str() {
            "windows" => vec![
                "Get-Service AmazonSSMAgent".to_string(),
                "Restart-Service AmazonSSMAgent".to_string(),
                "Get-Service AmazonSSMAgent | Select-Object Status".to_string(),
                "Get-EventLog -LogName Application -Source AmazonSSMAgent -Newest 10".to_string(),
            ],
            "amazon" | "rhel" | "centos" => vec![
                "sudo systemctl status amazon-ssm-agent".to_string(),
                "sudo systemctl restart amazon-ssm-agent".to_string(),
                "sudo systemctl enable amazon-ssm-agent".to_string(),
                "sudo journalctl -u amazon-ssm-agent -f".to_string(),
            ],
            "ubuntu" | "debian" => vec![
                "sudo systemctl status snap.amazon-ssm-agent.amazon-ssm-agent".to_string(),
                "sudo systemctl restart snap.amazon-ssm-agent.amazon-ssm-agent".to_string(),
                "sudo systemctl enable snap.amazon-ssm-agent.amazon-ssm-agent".to_string(),
                "sudo journalctl -u snap.amazon-ssm-agent.amazon-ssm-agent -f".to_string(),
            ],
            _ => vec![
                "sudo systemctl status amazon-ssm-agent".to_string(),
                "sudo systemctl restart amazon-ssm-agent".to_string(),
                "sudo systemctl enable amazon-ssm-agent".to_string(),
                "sudo journalctl -u amazon-ssm-agent -f".to_string(),
            ],
        }
    }

    fn classify_problem_severity(&self, result: &DiagnosticResult) -> ProblemSeverity {
        match result.status {
            DiagnosticStatus::Error => {
                match result.severity {
                    Severity::Critical => ProblemSeverity::Critical,
                    Severity::High => ProblemSeverity::High,
                    Severity::Medium => ProblemSeverity::Medium,
                    Severity::Low => ProblemSeverity::Low,
                    Severity::Info => ProblemSeverity::Low,
                }
            }
            DiagnosticStatus::Warning => ProblemSeverity::Medium,
            DiagnosticStatus::Success => ProblemSeverity::Low,
            DiagnosticStatus::Skipped => ProblemSeverity::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{DiagnosticResult, DiagnosticStatus, Severity};
    use std::time::Duration;

    fn create_test_diagnostic(item_name: &str, status: DiagnosticStatus, message: &str) -> DiagnosticResult {
        DiagnosticResult {
            item_name: item_name.to_string(),
            status,
            message: message.to_string(),
            details: None,
            duration: Duration::from_millis(100),
            severity: Severity::High,
            auto_fixable: false,
        }
    }

    #[test]
    fn test_suggestion_generator_creation() {
        let generator = DefaultSuggestionGenerator::new("us-east-1".to_string());
        assert_eq!(generator.region, "us-east-1");
    }

    #[test]
    fn test_problem_severity_classification() {
        let generator = DefaultSuggestionGenerator::new("us-east-1".to_string());
        
        let error_result = create_test_diagnostic("test", DiagnosticStatus::Error, "test error");
        assert_eq!(generator.classify_problem_severity(&error_result), ProblemSeverity::High);
        
        let warning_result = create_test_diagnostic("test", DiagnosticStatus::Warning, "test warning");
        assert_eq!(generator.classify_problem_severity(&warning_result), ProblemSeverity::Medium);
    }

    #[test]
    fn test_iam_policy_json_generation() {
        let generator = DefaultSuggestionGenerator::new("us-east-1".to_string());
        let permissions = vec!["ssm:StartSession".to_string(), "ssm:TerminateSession".to_string()];
        
        let policy = generator.generate_iam_policy_json(&permissions);
        
        assert!(policy.contains("ssm:StartSession"));
        assert!(policy.contains("ssm:TerminateSession"));
        assert!(policy.contains("Version"));
        assert!(policy.contains("2012-10-17"));
    }

    #[test]
    fn test_vpc_endpoint_commands_generation() {
        let generator = DefaultSuggestionGenerator::new("us-west-2".to_string());
        let commands = generator.generate_vpc_endpoint_commands("us-west-2", "vpc-12345");
        
        assert_eq!(commands.len(), 4); // 3 create commands + 1 describe command
        assert!(commands[0].contains("ssm"));
        assert!(commands[1].contains("ssmmessages"));
        assert!(commands[2].contains("ec2messages"));
        assert!(commands[3].contains("describe-vpc-endpoints"));
    }

    #[test]
    fn test_ssm_agent_commands_generation() {
        let generator = DefaultSuggestionGenerator::new("us-east-1".to_string());
        
        let linux_commands = generator.generate_ssm_agent_commands("linux");
        assert!(linux_commands.iter().any(|cmd| cmd.contains("systemctl")));
        
        let windows_commands = generator.generate_ssm_agent_commands("windows");
        assert!(windows_commands.iter().any(|cmd| cmd.contains("AmazonSSMAgent")));
    }

    #[test]
    fn test_fix_suggestion_builder() {
        let suggestion = FixSuggestion::new(
            "test".to_string(),
            ProblemSeverity::High,
            "Test Suggestion".to_string(),
            "Test description".to_string(),
        )
        .with_steps(vec!["Step 1".to_string(), "Step 2".to_string()])
        .with_cli_commands(vec!["command1".to_string()])
        .with_estimated_time("5 minutes".to_string());

        assert_eq!(suggestion.title, "Test Suggestion");
        assert_eq!(suggestion.steps.len(), 2);
        assert_eq!(suggestion.cli_commands.len(), 1);
        assert_eq!(suggestion.estimated_time, "5 minutes");
    }
}