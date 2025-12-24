use crate::error::{Ec2ConnectError, ConfigError, AwsError, SessionError, ConnectionError, ResourceError, UiError};
use std::collections::HashMap;

/// User-friendly error messages and help system
pub struct UserMessageSystem {
    help_messages: HashMap<String, HelpMessage>,
}

#[derive(Debug, Clone)]
pub struct HelpMessage {
    pub title: String,
    pub description: String,
    pub solutions: Vec<Solution>,
    pub related_docs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Solution {
    pub step: u32,
    pub description: String,
    pub command: Option<String>,
    pub example: Option<String>,
}

impl UserMessageSystem {
    pub fn new() -> Self {
        let mut system = Self {
            help_messages: HashMap::new(),
        };
        system.initialize_help_messages();
        system
    }

    /// Get user-friendly error message with solutions
    pub fn get_error_message(&self, error: &Ec2ConnectError) -> UserErrorMessage {
        match error {
            Ec2ConnectError::Config(config_error) => self.handle_config_error(config_error),
            Ec2ConnectError::Aws(aws_error) => self.handle_aws_error(aws_error),
            Ec2ConnectError::Session(session_error) => self.handle_session_error(session_error),
            Ec2ConnectError::Connection(connection_error) => self.handle_connection_error(connection_error),
            Ec2ConnectError::Resource(resource_error) => self.handle_resource_error(resource_error),
            Ec2ConnectError::Ui(ui_error) => self.handle_ui_error(ui_error),
            _ => UserErrorMessage {
                title: "予期しないエラー".to_string(),
                message: error.to_string(),
                severity: "medium".to_string(),
                solutions: vec![
                    "アプリケーションを再起動してください".to_string(),
                    "問題が続く場合は、ログファイルを確認してください".to_string(),
                ],
                help_command: Some("ec2-connect --help".to_string()),
            },
        }
    }

    fn handle_config_error(&self, error: &ConfigError) -> UserErrorMessage {
        match error {
            ConfigError::FileNotFound { path } => UserErrorMessage {
                title: "設定ファイルが見つかりません".to_string(),
                message: format!("設定ファイル '{}' が見つかりません。", path),
                severity: "medium".to_string(),
                solutions: vec![
                    "設定ファイルのサンプルをコピーして編集してください".to_string(),
                    format!("cp {}.example {}", path, path),
                    "設定ファイルのパスが正しいか確認してください".to_string(),
                ],
                help_command: Some("ec2-connect config --help".to_string()),
            },
            ConfigError::Invalid { message } => UserErrorMessage {
                title: "設定ファイルの内容が無効です".to_string(),
                message: format!("設定エラー: {}", message),
                severity: "high".to_string(),
                solutions: vec![
                    "設定ファイルの構文を確認してください".to_string(),
                    "JSON/TOML形式が正しいか確認してください".to_string(),
                    "設定ファイルのサンプルと比較してください".to_string(),
                ],
                help_command: Some("ec2-connect config validate".to_string()),
            },
            ConfigError::ValidationFailed { field } => UserErrorMessage {
                title: "設定の検証に失敗しました".to_string(),
                message: format!("フィールド '{}' の値が無効です。", field),
                severity: "medium".to_string(),
                solutions: vec![
                    format!("フィールド '{}' の値を確認してください", field),
                    "許可される値については、ドキュメントを参照してください".to_string(),
                ],
                help_command: Some("ec2-connect config --help".to_string()),
            },
            ConfigError::PermissionDenied { path } => UserErrorMessage {
                title: "設定ファイルへのアクセスが拒否されました".to_string(),
                message: format!("ファイル '{}' への読み取り権限がありません。", path),
                severity: "high".to_string(),
                solutions: vec![
                    "ファイルの権限を確認してください".to_string(),
                    format!("chmod 644 {}", path),
                    "管理者権限で実行してみてください".to_string(),
                ],
                help_command: None,
            },
        }
    }

    fn handle_aws_error(&self, error: &AwsError) -> UserErrorMessage {
        match error {
            AwsError::AuthenticationFailed { message } => UserErrorMessage {
                title: "AWS認証に失敗しました".to_string(),
                message: format!("認証エラー: {}", message),
                severity: "high".to_string(),
                solutions: vec![
                    "AWS認証情報を確認してください".to_string(),
                    "aws configure list で設定を確認".to_string(),
                    "AWS CLIが正しくインストールされているか確認".to_string(),
                    "IAM権限が適切に設定されているか確認".to_string(),
                ],
                help_command: Some("aws configure --help".to_string()),
            },
            AwsError::InvalidCredentials => UserErrorMessage {
                title: "AWS認証情報が無効です".to_string(),
                message: "提供された認証情報が無効または期限切れです。".to_string(),
                severity: "high".to_string(),
                solutions: vec![
                    "AWS認証情報を更新してください".to_string(),
                    "aws configure で新しい認証情報を設定".to_string(),
                    "一時的な認証情報の場合、セッショントークンを確認".to_string(),
                ],
                help_command: Some("aws sts get-caller-identity".to_string()),
            },
            AwsError::RegionNotFound { region } => UserErrorMessage {
                title: "AWSリージョンが見つかりません".to_string(),
                message: format!("リージョン '{}' が見つからないか、利用できません。", region),
                severity: "medium".to_string(),
                solutions: vec![
                    "リージョン名のスペルを確認してください".to_string(),
                    "利用可能なリージョンのリストを確認".to_string(),
                    "aws ec2 describe-regions でリージョンを確認".to_string(),
                ],
                help_command: Some("aws ec2 describe-regions".to_string()),
            },
            AwsError::InstanceNotFound { instance_id } => UserErrorMessage {
                title: "EC2インスタンスが見つかりません".to_string(),
                message: format!("インスタンス '{}' が見つからないか、アクセスできません。", instance_id),
                severity: "medium".to_string(),
                solutions: vec![
                    "インスタンスIDが正しいか確認してください".to_string(),
                    "インスタンスが実行中か確認".to_string(),
                    "適切なリージョンを選択しているか確認".to_string(),
                    "IAM権限でインスタンスにアクセスできるか確認".to_string(),
                ],
                help_command: Some("aws ec2 describe-instances".to_string()),
            },
            AwsError::SsmServiceError { message } => UserErrorMessage {
                title: "SSMサービスエラー".to_string(),
                message: format!("SSMサービスエラー: {}", message),
                severity: "medium".to_string(),
                solutions: vec![
                    "SSMエージェントがインスタンスで実行されているか確認".to_string(),
                    "インスタンスにSSM用のIAMロールが設定されているか確認".to_string(),
                    "VPCエンドポイントまたはインターネットアクセスが利用可能か確認".to_string(),
                ],
                help_command: Some("aws ssm describe-instance-information".to_string()),
            },
            AwsError::NetworkError { message } => UserErrorMessage {
                title: "ネットワークエラー".to_string(),
                message: format!("ネットワーク接続エラー: {}", message),
                severity: "medium".to_string(),
                solutions: vec![
                    "インターネット接続を確認してください".to_string(),
                    "ファイアウォール設定を確認".to_string(),
                    "プロキシ設定が必要な場合は設定を確認".to_string(),
                    "しばらく待ってから再試行".to_string(),
                ],
                help_command: None,
            },
            AwsError::Timeout { operation } => UserErrorMessage {
                title: "操作がタイムアウトしました".to_string(),
                message: format!("操作 '{}' がタイムアウトしました。", operation),
                severity: "medium".to_string(),
                solutions: vec![
                    "ネットワーク接続を確認してください".to_string(),
                    "しばらく待ってから再試行".to_string(),
                    "タイムアウト設定を調整".to_string(),
                ],
                help_command: None,
            },
            _ => UserErrorMessage {
                title: "AWSエラー".to_string(),
                message: error.to_string(),
                severity: "medium".to_string(),
                solutions: vec!["AWS設定を確認してください".to_string()],
                help_command: Some("aws configure list".to_string()),
            },
        }
    }

    fn handle_session_error(&self, error: &SessionError) -> UserErrorMessage {
        match error {
            SessionError::NotFound { session_id } => UserErrorMessage {
                title: "セッションが見つかりません".to_string(),
                message: format!("セッション '{}' が見つかりません。", session_id),
                severity: "low".to_string(),
                solutions: vec![
                    "新しいセッションを作成してください".to_string(),
                    "アクティブなセッションのリストを確認".to_string(),
                ],
                help_command: Some("ec2-connect list-sessions".to_string()),
            },
            SessionError::CreationFailed { reason } => UserErrorMessage {
                title: "セッションの作成に失敗しました".to_string(),
                message: format!("セッション作成エラー: {}", reason),
                severity: "medium".to_string(),
                solutions: vec![
                    "インスタンスが実行中か確認してください".to_string(),
                    "SSMエージェントが動作しているか確認".to_string(),
                    "ネットワーク接続を確認".to_string(),
                    "しばらく待ってから再試行".to_string(),
                ],
                help_command: Some("ec2-connect status".to_string()),
            },
            SessionError::LimitExceeded { max_sessions } => UserErrorMessage {
                title: "セッション数の上限に達しました".to_string(),
                message: format!("同時セッション数の上限（{}セッション）に達しました。", max_sessions),
                severity: "medium".to_string(),
                solutions: vec![
                    "不要なセッションを終了してください".to_string(),
                    "ec2-connect list-sessions で確認".to_string(),
                    "ec2-connect terminate <session-id> で終了".to_string(),
                ],
                help_command: Some("ec2-connect list-sessions".to_string()),
            },
            SessionError::Unhealthy { session_id } => UserErrorMessage {
                title: "セッションが不健全な状態です".to_string(),
                message: format!("セッション '{}' が不健全な状態です。", session_id),
                severity: "medium".to_string(),
                solutions: vec![
                    "セッションを再起動してください".to_string(),
                    "ネットワーク接続を確認".to_string(),
                    "インスタンスの状態を確認".to_string(),
                ],
                help_command: Some("ec2-connect restart-session".to_string()),
            },
            _ => UserErrorMessage {
                title: "セッションエラー".to_string(),
                message: error.to_string(),
                severity: "medium".to_string(),
                solutions: vec!["セッションを再作成してください".to_string()],
                help_command: Some("ec2-connect --help".to_string()),
            },
        }
    }

    fn handle_connection_error(&self, error: &ConnectionError) -> UserErrorMessage {
        match error {
            ConnectionError::PortInUse { port } => UserErrorMessage {
                title: "ポートが既に使用されています".to_string(),
                message: format!("ポート {} は既に使用されています。", port),
                severity: "medium".to_string(),
                solutions: vec![
                    "別のポート番号を指定してください".to_string(),
                    format!("lsof -i :{} でポートの使用状況を確認", port),
                    "使用中のプロセスを終了するか、別のポートを使用".to_string(),
                ],
                help_command: Some("ec2-connect --port <PORT>".to_string()),
            },
            ConnectionError::Timeout { target } => UserErrorMessage {
                title: "接続がタイムアウトしました".to_string(),
                message: format!("'{}' への接続がタイムアウトしました。", target),
                severity: "medium".to_string(),
                solutions: vec![
                    "ネットワーク接続を確認してください".to_string(),
                    "ターゲットが応答可能か確認".to_string(),
                    "ファイアウォール設定を確認".to_string(),
                    "しばらく待ってから再試行".to_string(),
                ],
                help_command: None,
            },
            _ => UserErrorMessage {
                title: "接続エラー".to_string(),
                message: error.to_string(),
                severity: "medium".to_string(),
                solutions: vec!["ネットワーク設定を確認してください".to_string()],
                help_command: None,
            },
        }
    }

    fn handle_resource_error(&self, error: &ResourceError) -> UserErrorMessage {
        match error {
            ResourceError::MemoryLimitExceeded { current_mb, limit_mb } => UserErrorMessage {
                title: "メモリ使用量が上限を超えました".to_string(),
                message: format!("メモリ使用量: {}MB（上限: {}MB）", current_mb, limit_mb),
                severity: "high".to_string(),
                solutions: vec![
                    "不要なセッションを終了してください".to_string(),
                    "他のアプリケーションを終了してメモリを解放".to_string(),
                    "システムのメモリ使用量を確認".to_string(),
                ],
                help_command: Some("ec2-connect list-sessions".to_string()),
            },
            ResourceError::CpuLimitExceeded { current_percent, limit_percent } => UserErrorMessage {
                title: "CPU使用率が上限を超えました".to_string(),
                message: format!("CPU使用率: {:.1}%（上限: {:.1}%）", current_percent, limit_percent),
                severity: "medium".to_string(),
                solutions: vec![
                    "システムの負荷を確認してください".to_string(),
                    "不要なプロセスを終了".to_string(),
                    "しばらく待ってから再試行".to_string(),
                ],
                help_command: None,
            },
            _ => UserErrorMessage {
                title: "リソースエラー".to_string(),
                message: error.to_string(),
                severity: "medium".to_string(),
                solutions: vec!["システムリソースを確認してください".to_string()],
                help_command: None,
            },
        }
    }

    fn handle_ui_error(&self, error: &UiError) -> UserErrorMessage {
        match error {
            UiError::TerminalInitFailed => UserErrorMessage {
                title: "ターミナルの初期化に失敗しました".to_string(),
                message: "ターミナルUIの初期化に失敗しました。".to_string(),
                severity: "medium".to_string(),
                solutions: vec![
                    "ターミナルが対応しているか確認してください".to_string(),
                    "環境変数TERMを確認".to_string(),
                    "--no-ui オプションでCLIモードを使用".to_string(),
                ],
                help_command: Some("ec2-connect --no-ui".to_string()),
            },
            _ => UserErrorMessage {
                title: "UIエラー".to_string(),
                message: error.to_string(),
                severity: "low".to_string(),
                solutions: vec!["UIを再初期化してください".to_string()],
                help_command: Some("ec2-connect --help".to_string()),
            },
        }
    }

    fn initialize_help_messages(&mut self) {
        // AWS認証のヘルプ
        self.help_messages.insert(
            "aws_auth".to_string(),
            HelpMessage {
                title: "AWS認証の設定".to_string(),
                description: "EC2 Connectを使用するには、適切なAWS認証情報が必要です。".to_string(),
                solutions: vec![
                    Solution {
                        step: 1,
                        description: "AWS CLIをインストール".to_string(),
                        command: Some("curl \"https://awscli.amazonaws.com/AWSCLIV2.pkg\" -o \"AWSCLIV2.pkg\"".to_string()),
                        example: None,
                    },
                    Solution {
                        step: 2,
                        description: "AWS認証情報を設定".to_string(),
                        command: Some("aws configure".to_string()),
                        example: Some("Access Key ID, Secret Access Key, Region, Output formatを入力".to_string()),
                    },
                    Solution {
                        step: 3,
                        description: "認証情報を確認".to_string(),
                        command: Some("aws sts get-caller-identity".to_string()),
                        example: None,
                    },
                ],
                related_docs: vec![
                    "https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html".to_string(),
                ],
            },
        );

        // セッション管理のヘルプ
        self.help_messages.insert(
            "session_management".to_string(),
            HelpMessage {
                title: "セッション管理".to_string(),
                description: "EC2インスタンスへのSSMセッションを効率的に管理する方法。".to_string(),
                solutions: vec![
                    Solution {
                        step: 1,
                        description: "アクティブなセッションを確認".to_string(),
                        command: Some("ec2-connect list-sessions".to_string()),
                        example: None,
                    },
                    Solution {
                        step: 2,
                        description: "新しいセッションを作成".to_string(),
                        command: Some("ec2-connect connect <instance-id>".to_string()),
                        example: Some("ec2-connect connect i-1234567890abcdef0".to_string()),
                    },
                    Solution {
                        step: 3,
                        description: "セッションを終了".to_string(),
                        command: Some("ec2-connect terminate <session-id>".to_string()),
                        example: None,
                    },
                ],
                related_docs: vec![
                    "https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager.html".to_string(),
                ],
            },
        );
    }

    pub fn get_help_message(&self, topic: &str) -> Option<&HelpMessage> {
        self.help_messages.get(topic)
    }

    pub fn list_help_topics(&self) -> Vec<String> {
        self.help_messages.keys().cloned().collect()
    }
}

#[derive(Debug, Clone)]
pub struct UserErrorMessage {
    pub title: String,
    pub message: String,
    pub severity: String,
    pub solutions: Vec<String>,
    pub help_command: Option<String>,
}

impl UserErrorMessage {
    /// Format error message for display
    pub fn format_for_display(&self) -> String {
        let mut output = String::new();
        
        // Title with severity indicator
        let severity_icon = match self.severity.as_str() {
            "low" => "⚠️",
            "medium" => "❌",
            "high" => "🚨",
            "critical" => "💥",
            _ => "❓",
        };
        
        output.push_str(&format!("{} {}\n", severity_icon, self.title));
        output.push_str(&format!("\n{}\n", self.message));
        
        if !self.solutions.is_empty() {
            output.push_str("\n解決方法:\n");
            for (i, solution) in self.solutions.iter().enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, solution));
            }
        }
        
        if let Some(help_cmd) = &self.help_command {
            output.push_str(&format!("\nヘルプ: {}\n", help_cmd));
        }
        
        output
    }

    /// Format error message for JSON output
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "title": self.title,
            "message": self.message,
            "severity": self.severity,
            "solutions": self.solutions,
            "help_command": self.help_command
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ConfigError;

    #[test]
    fn test_user_message_system() {
        let system = UserMessageSystem::new();
        
        let error = Ec2ConnectError::Config(ConfigError::FileNotFound {
            path: "config.toml".to_string()
        });
        
        let message = system.get_error_message(&error);
        
        assert_eq!(message.title, "設定ファイルが見つかりません");
        assert!(message.message.contains("config.toml"));
        assert!(!message.solutions.is_empty());
    }

    #[test]
    fn test_help_message_retrieval() {
        let system = UserMessageSystem::new();
        
        let help = system.get_help_message("aws_auth");
        assert!(help.is_some());
        
        let help = help.unwrap();
        assert_eq!(help.title, "AWS認証の設定");
        assert!(!help.solutions.is_empty());
    }

    #[test]
    fn test_error_message_formatting() {
        let message = UserErrorMessage {
            title: "テストエラー".to_string(),
            message: "これはテストメッセージです".to_string(),
            severity: "medium".to_string(),
            solutions: vec!["解決策1".to_string(), "解決策2".to_string()],
            help_command: Some("test --help".to_string()),
        };
        
        let formatted = message.format_for_display();
        assert!(formatted.contains("❌ テストエラー"));
        assert!(formatted.contains("これはテストメッセージです"));
        assert!(formatted.contains("1. 解決策1"));
        assert!(formatted.contains("2. 解決策2"));
        assert!(formatted.contains("ヘルプ: test --help"));
    }
}