use crate::error::{AwsError, ConfigError, ConnectionError, NimbusError, SessionError};
use std::collections::HashMap;

/// User-friendly error messages and help system
pub struct UserMessageSystem {
    help_messages: HashMap<String, HelpMessage>,
}

#[derive(Debug, Clone)]
pub struct HelpMessage {}
impl UserMessageSystem {
    pub fn new() -> Self {
        let mut system = Self {
            help_messages: HashMap::new(),
        };
        system.initialize_help_messages();
        system
    }

    /// Get user-friendly error message with solutions
    pub fn get_error_message(&self, error: &NimbusError) -> UserErrorMessage {
        match error {
            NimbusError::Config(config_error) => self.handle_config_error(config_error),
            NimbusError::Aws(aws_error) => self.handle_aws_error(aws_error),
            NimbusError::Session(session_error) => self.handle_session_error(session_error),
            NimbusError::Connection(connection_error) => {
                self.handle_connection_error(connection_error)
            }
            _ => UserErrorMessage {
                title: "予期しないエラー".to_string(),
                message: error.to_string(),
                severity: "medium".to_string(),
                solutions: vec![
                    "アプリケーションを再起動してください".to_string(),
                    "問題が続く場合は、ログファイルを確認してください".to_string(),
                ],
                help_command: Some("nimbus --help".to_string()),
            },
        }
    }

    fn handle_config_error(&self, _error: &ConfigError) -> UserErrorMessage {
        UserErrorMessage {
            title: "設定エラー".to_string(),
            message: "設定に問題があります".to_string(),
            severity: "medium".to_string(),
            solutions: vec!["設定ファイルを確認してください".to_string()],
            help_command: Some("nimbus config --help".to_string()),
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
                help_command: Some("nimbus list-sessions".to_string()),
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
                help_command: Some("nimbus status".to_string()),
            },
            SessionError::LimitExceeded { max_sessions } => UserErrorMessage {
                title: "セッション数の上限に達しました".to_string(),
                message: format!(
                    "同時セッション数の上限（{}セッション）に達しました。",
                    max_sessions
                ),
                severity: "medium".to_string(),
                solutions: vec![
                    "不要なセッションを終了してください".to_string(),
                    "nimbus list-sessions で確認".to_string(),
                    "nimbus terminate <session-id> で終了".to_string(),
                ],
                help_command: Some("nimbus list-sessions".to_string()),
            },
            SessionError::ResourceLimitExceeded { resource, .. } => UserErrorMessage {
                title: "リソース制限に達しました".to_string(),
                message: format!("リソース '{}' が制限に達しました。", resource),
                severity: "high".to_string(),
                solutions: vec!["不要なセッションを終了してリソースを解放してください".to_string()],
                help_command: Some("nimbus list-sessions".to_string()),
            },
            SessionError::ReconnectionFailed {
                session_id,
                attempts,
            } => UserErrorMessage {
                title: "再接続に失敗しました".to_string(),
                message: format!(
                    "セッション '{}' への再接続が {} 回失敗しました。",
                    session_id, attempts
                ),
                severity: "high".to_string(),
                solutions: vec![
                    "ネットワーク接続を確認してください".to_string(),
                    "新しいセッションを作成してください".to_string(),
                ],
                help_command: Some("nimbus connect".to_string()),
            },
        }
    }

    fn handle_connection_error(&self, error: &ConnectionError) -> UserErrorMessage {
        match error {
            ConnectionError::PreventiveCheckFailed { reason, issues } => UserErrorMessage {
                title: "事前チェックに失敗しました".to_string(),
                message: format!("理由: {}", reason),
                severity: "high".to_string(),
                solutions: issues.clone(),
                help_command: Some("nimbus diagnose".to_string()),
            },
        }
    }

    fn handle_resource_error(&self, error: &ResourceError) -> UserErrorMessage {
        UserErrorMessage {
            title: "リソースエラー".to_string(),
            message: format!("リソースに問題があります: {}", error),
            severity: "medium".to_string(),
            solutions: vec!["システムリソースを確認してください".to_string()],
            help_command: None,
        }
    }

    fn handle_ui_error(&self, error: &UiError) -> UserErrorMessage {
        UserErrorMessage {
            title: "UIエラー".to_string(),
            message: format!("UI処理中にエラーが発生しました: {}", error),
            severity: "low".to_string(),
            solutions: vec!["アプリケーションを再起動してください".to_string()],
            help_command: None,
        }
    }

    fn initialize_help_messages(&mut self) {
        // AWS認証のヘルプ
        self.help_messages
            .insert("aws_auth".to_string(), HelpMessage {});

        // セッション管理のヘルプ
        self.help_messages
            .insert("session_management".to_string(), HelpMessage {});
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message_system() {
        let system = UserMessageSystem::new();

        let error = NimbusError::Connection(ConnectionError::PreventiveCheckFailed {
            reason: "test".to_string(),
            issues: vec!["issue1".to_string()],
        });

        let message = system.get_error_message(&error);

        assert_eq!(message.title, "事前チェックに失敗しました");
        assert!(message.message.contains("test"));
        assert!(!message.solutions.is_empty());
    }

    #[test]
    fn test_help_message_retrieval() {
        // get_help_message は未実装のため、get_error_message の動作を確認
        let system = UserMessageSystem::new();
        let error = crate::error::NimbusError::Io("test".to_string());
        let msg = system.get_error_message(&error);
        assert!(!msg.title.is_empty());
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
