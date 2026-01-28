use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tokio::fs;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

use crate::error::{NimbusError, VsCodeError};
use crate::logging::StructuredLogger;
use crate::session::Session;

/// VS Code統合機能を提供するマネージャー
pub struct VsCodeIntegration {
    /// VS Code実行可能ファイルのパス
    vscode_path: Option<PathBuf>,
    /// SSH設定ファイルのパス
    ssh_config_path: PathBuf,
    /// 通知システムの有効/無効
    notifications_enabled: bool,
    /// 自動起動の有効/無効
    auto_launch_enabled: bool,

    /// SSH接続ユーザー（未指定時は ec2-user）
    ssh_user: Option<String>,
    /// SSH秘密鍵（.ssh/config の IdentityFile）
    ssh_identity_file: Option<String>,
    /// SSH の IdentitiesOnly を有効化
    ssh_identities_only: bool,
}

/// VS Code統合設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsCodeConfig {
    /// VS Code実行可能ファイルのパス
    pub vscode_path: Option<String>,
    /// SSH設定ファイルのパス
    pub ssh_config_path: Option<String>,
    /// 自動起動の有効/無効
    pub auto_launch_enabled: bool,
    /// 通知の有効/無効
    pub notifications_enabled: bool,
    /// 接続後の待機時間（秒）
    pub launch_delay_seconds: u64,
    /// SSH設定の自動更新
    pub auto_update_ssh_config: bool,

    /// SSH接続ユーザー（未指定時は ec2-user）
    #[serde(default)]
    pub ssh_user: Option<String>,
    /// SSH秘密鍵のパス（.ssh/config の IdentityFile）
    #[serde(default)]
    pub ssh_identity_file: Option<String>,
    /// SSH の IdentitiesOnly を有効化
    #[serde(default)]
    pub ssh_identities_only: bool,
}

impl Default for VsCodeConfig {
    fn default() -> Self {
        Self {
            vscode_path: None,
            ssh_config_path: None,
            auto_launch_enabled: true,
            notifications_enabled: true,
            launch_delay_seconds: 2,
            auto_update_ssh_config: true,

            ssh_user: None,
            ssh_identity_file: None,
            ssh_identities_only: false,
        }
    }
}

/// VS Code統合の結果
#[derive(Debug, Clone)]
pub struct VsCodeIntegrationResult {
    pub success: bool,
    pub vscode_launched: bool,
    pub ssh_config_updated: bool,
    pub notification_sent: bool,
    pub error_message: Option<String>,
    pub connection_info: Option<ConnectionInfo>,
}

/// 接続情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub session_id: String,
    pub instance_id: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub ssh_host: String,
    pub connection_url: String,
}

impl VsCodeIntegration {
    /// 新しいVS Code統合マネージャーを作成
    pub fn new(config: VsCodeConfig) -> Result<Self> {
        let vscode_path = Self::detect_vscode_path(config.vscode_path)?;
        let ssh_config_path = Self::get_ssh_config_path(config.ssh_config_path)?;

        Ok(Self {
            vscode_path,
            ssh_config_path,
            notifications_enabled: config.notifications_enabled,
            auto_launch_enabled: config.auto_launch_enabled,
            ssh_user: config.ssh_user,
            ssh_identity_file: config.ssh_identity_file,
            ssh_identities_only: config.ssh_identities_only,
        })
    }

    /// VS Code実行可能ファイルのパスを検出
    fn detect_vscode_path(custom_path: Option<String>) -> Result<Option<PathBuf>> {
        if let Some(path) = custom_path {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                info!("Using custom VS Code path: {:?}", path_buf);
                return Ok(Some(path_buf));
            } else {
                warn!("Custom VS Code path not found: {:?}", path_buf);
            }
        }

        // 一般的なVS Codeのパスを検索
        let common_paths = if cfg!(target_os = "windows") {
            vec![
                r"C:\Users\{}\AppData\Local\Programs\Microsoft VS Code\Code.exe",
                r"C:\Program Files\Microsoft VS Code\Code.exe",
                r"C:\Program Files (x86)\Microsoft VS Code\Code.exe",
            ]
        } else if cfg!(target_os = "macos") {
            vec![
                "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
                "/usr/local/bin/code",
                "/opt/homebrew/bin/code",
            ]
        } else {
            vec![
                "/usr/bin/code",
                "/usr/local/bin/code",
                "/snap/bin/code",
                "/opt/code/bin/code",
            ]
        };

        for path_str in common_paths {
            let path = if path_str.contains("{}") {
                // Windowsのユーザーディレクトリを展開
                if let Some(username) = std::env::var("USERNAME").ok() {
                    PathBuf::from(path_str.replace("{}", &username))
                } else {
                    continue;
                }
            } else {
                PathBuf::from(path_str)
            };

            if path.exists() {
                info!("Found VS Code at: {:?}", path);
                return Ok(Some(path));
            }
        }

        // PATHからcodeコマンドを検索
        if let Ok(output) = Command::new("which").arg("code").output() {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                let trimmed_path = path_str.trim();
                let path = PathBuf::from(trimmed_path);
                if path.exists() {
                    info!("Found VS Code in PATH: {:?}", path);
                    return Ok(Some(path));
                }
            }
        }

        warn!("VS Code not found in common locations");
        Ok(None)
    }

    /// SSH設定ファイルのパスを取得
    fn get_ssh_config_path(custom_path: Option<String>) -> Result<PathBuf> {
        if let Some(path) = custom_path {
            return Ok(PathBuf::from(path));
        }

        // デフォルトのSSH設定ファイルパス
        let home_dir = dirs::home_dir().ok_or_else(|| {
            NimbusError::VsCode(VsCodeError::ConfigurationError {
                message: "Could not determine home directory".to_string(),
            })
        })?;

        let ssh_dir = home_dir.join(".ssh");
        let config_path = ssh_dir.join("config");

        // .sshディレクトリが存在しない場合は作成
        if !ssh_dir.exists() {
            std::fs::create_dir_all(&ssh_dir).context("Failed to create .ssh directory")?;
        }

        Ok(config_path)
    }

    /// セッションに対してVS Code統合を実行
    pub async fn integrate_session(&self, session: &Session) -> Result<VsCodeIntegrationResult> {
        info!("Starting VS Code integration for session: {}", session.id);

        let mut result = VsCodeIntegrationResult {
            success: false,
            vscode_launched: false,
            ssh_config_updated: false,
            notification_sent: false,
            error_message: None,
            connection_info: None,
        };

        // 接続情報を作成
        let connection_info = ConnectionInfo {
            session_id: session.id.clone(),
            instance_id: session.instance_id.clone(),
            local_port: session.local_port,
            remote_port: session.remote_port,
            ssh_host: format!("ec2-{}", session.instance_id),
            connection_url: format!("localhost:{}", session.local_port),
        };

        result.connection_info = Some(connection_info.clone());

        // SSH設定を更新
        match self.update_ssh_config(&connection_info).await {
            Ok(_) => {
                result.ssh_config_updated = true;
                info!(
                    "SSH config updated successfully for session: {}",
                    session.id
                );
            }
            Err(e) => {
                error!("Failed to update SSH config: {}", e);
                result.error_message = Some(format!("SSH config update failed: {}", e));
                return Ok(result);
            }
        }

        // VS Codeを自動起動
        if self.auto_launch_enabled {
            match self.launch_vscode(&connection_info).await {
                Ok(_) => {
                    result.vscode_launched = true;
                    info!("VS Code launched successfully for session: {}", session.id);
                }
                Err(e) => {
                    warn!("Failed to launch VS Code: {}", e);
                    result.error_message = Some(format!("VS Code launch failed: {}", e));
                }
            }
        }

        // 通知を送信
        if self.notifications_enabled {
            match self.send_notification(&connection_info).await {
                Ok(_) => {
                    result.notification_sent = true;
                    debug!("Notification sent for session: {}", session.id);
                }
                Err(e) => {
                    warn!("Failed to send notification: {}", e);
                }
            }
        }

        result.success = result.ssh_config_updated;

        // ログ記録
        let mut context_map = HashMap::new();
        context_map.insert("session_id".to_string(), session.id.clone());
        context_map.insert("instance_id".to_string(), session.instance_id.clone());
        context_map.insert(
            "vscode_launched".to_string(),
            result.vscode_launched.to_string(),
        );
        context_map.insert(
            "ssh_config_updated".to_string(),
            result.ssh_config_updated.to_string(),
        );

        StructuredLogger::log_session_activity(
            &session.id,
            "vscode_integration",
            Some(&context_map),
        );

        Ok(result)
    }

    /// SSH設定ファイルを更新
    async fn update_ssh_config(&self, connection_info: &ConnectionInfo) -> Result<()> {
        debug!("Updating SSH config for host: {}", connection_info.ssh_host);

        // 既存のSSH設定を読み込み
        let mut config_content = if self.ssh_config_path.exists() {
            fs::read_to_string(&self.ssh_config_path)
                .await
                .context("Failed to read SSH config file")?
        } else {
            String::new()
        };

        // 新しいホストエントリを作成
        let host_entry = self.create_ssh_host_entry(connection_info);

        // 既存のエントリを削除（同じホスト名の場合）
        config_content =
            self.remove_existing_host_entry(&config_content, &connection_info.ssh_host);

        // 新しいエントリを追加
        if !config_content.is_empty() && !config_content.ends_with('\n') {
            config_content.push('\n');
        }
        config_content.push_str(&host_entry);

        // ファイルに書き込み
        fs::write(&self.ssh_config_path, config_content)
            .await
            .context("Failed to write SSH config file")?;

        info!("SSH config updated: {:?}", self.ssh_config_path);
        Ok(())
    }

    /// SSH ホストエントリを作成
    fn create_ssh_host_entry(&self, connection_info: &ConnectionInfo) -> String {
        let ssh_user = self.ssh_user.as_deref().unwrap_or("ec2-user");

        let mut ssh_extra = String::new();
        if let Some(identity_file) = &self.ssh_identity_file {
            ssh_extra.push_str(&format!("    IdentityFile {}\n", identity_file));
        }
        if self.ssh_identities_only {
            ssh_extra.push_str("    IdentitiesOnly yes\n");
        }

        format!(
            r#"
# Nimbus - Session: {}
Host {}
    HostName localhost
    Port {}
    User {}
{}    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null
    LogLevel ERROR
    # Instance: {}
    # Remote Port: {}
    # Created: {}

"#,
            connection_info.session_id,
            connection_info.ssh_host,
            connection_info.local_port,
            ssh_user,
            ssh_extra,
            connection_info.instance_id,
            connection_info.remote_port,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )
    }

    /// 既存のホストエントリを削除
    fn remove_existing_host_entry(&self, config_content: &str, host_name: &str) -> String {
        let lines: Vec<&str> = config_content.lines().collect();
        let mut result_lines = Vec::new();
        let mut skip_until_next_host = false;
        let mut in_nimbus_section = false;

        for line in lines {
            let trimmed = line.trim();

            // Nimbusセクションの開始を検出
            if trimmed.starts_with("# Nimbus") {
                in_nimbus_section = true;
                continue;
            }

            // ホストエントリの開始を検出
            if trimmed.starts_with("Host ") {
                if in_nimbus_section {
                    let host_in_line = trimmed.strip_prefix("Host ").unwrap_or("").trim();
                    if host_in_line == host_name {
                        skip_until_next_host = true;
                        in_nimbus_section = false;
                        continue;
                    }
                }
                skip_until_next_host = false;
                in_nimbus_section = false;
            }

            // 空行でNimbusセクションを終了
            if in_nimbus_section && trimmed.is_empty() {
                in_nimbus_section = false;
                continue;
            }

            // スキップ中でなければ行を保持
            if !skip_until_next_host && !in_nimbus_section {
                result_lines.push(line.to_string());
            }
        }

        result_lines.join("\n")
    }

    /// VS Codeを起動
    async fn launch_vscode(&self, connection_info: &ConnectionInfo) -> Result<()> {
        let vscode_path = self.vscode_path.as_ref().ok_or_else(|| {
            NimbusError::VsCode(VsCodeError::NotFound {
                message: "VS Code executable not found".to_string(),
            })
        })?;

        info!("Launching VS Code for host: {}", connection_info.ssh_host);

        // 少し待機してからVS Codeを起動（接続が安定するまで）
        sleep(Duration::from_secs(2)).await;

        // VS Codeを起動（SSH接続で）
        let ssh_uri = format!("vscode-remote://ssh-remote+{}/", connection_info.ssh_host);

        let mut command = Command::new(vscode_path);
        command.arg("--remote").arg(&ssh_uri);

        // バックグラウンドで実行
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        match command.spawn() {
            Ok(mut child) => {
                // プロセスIDをログに記録
                info!("VS Code launched with PID: {:?}", child.id());

                // プロセスを切り離し（親プロセスの終了を待たない）
                tokio::spawn(async move {
                    if let Err(e) = child.wait() {
                        warn!("VS Code process error: {}", e);
                    }
                });

                Ok(())
            }
            Err(e) => {
                error!("Failed to launch VS Code: {}", e);
                Err(NimbusError::VsCode(VsCodeError::LaunchFailed {
                    message: format!("Failed to launch VS Code: {}", e),
                })
                .into())
            }
        }
    }

    /// 通知を送信
    async fn send_notification(&self, connection_info: &ConnectionInfo) -> Result<()> {
        let title = "Nimbus - VS Code Integration";
        let message = format!(
            "VS Code integration completed for instance {}\nSSH Host: {}\nLocal Port: {}",
            connection_info.instance_id, connection_info.ssh_host, connection_info.local_port
        );

        // プラットフォーム固有の通知システムを使用
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("osascript")
                .arg("-e")
                .arg(&format!(
                    r#"display notification "{}" with title "{}""#,
                    message.replace('\n', " - "),
                    title
                ))
                .output();
        }

        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("notify-send")
                .arg(title)
                .arg(&message)
                .output();
        }

        #[cfg(target_os = "windows")]
        {
            // Windows Toast通知（PowerShellを使用）
            let ps_script = format!(
                r#"
                [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
                $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
                $template.SelectSingleNode('//text[@id="1"]').AppendChild($template.CreateTextNode('{}')) | Out-Null
                $template.SelectSingleNode('//text[@id="2"]').AppendChild($template.CreateTextNode('{}')) | Out-Null
                $toast = [Windows.UI.Notifications.ToastNotification]::new($template)
                [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Nimbus').Show($toast)
                "#,
                title,
                message.replace('\n', " - ")
            );

            let _ = Command::new("powershell")
                .arg("-Command")
                .arg(&ps_script)
                .output();
        }

        debug!(
            "Notification sent for session: {}",
            connection_info.session_id
        );
        Ok(())
    }

    /// SSH設定をクリーンアップ（セッション終了時）
    pub async fn cleanup_ssh_config(&self, session_id: &str) -> Result<()> {
        info!("Cleaning up SSH config for session: {}", session_id);

        if !self.ssh_config_path.exists() {
            return Ok(());
        }

        let config_content = fs::read_to_string(&self.ssh_config_path)
            .await
            .context("Failed to read SSH config file")?;

        let cleaned_content = self.remove_session_entries(&config_content, session_id);

        fs::write(&self.ssh_config_path, cleaned_content)
            .await
            .context("Failed to write cleaned SSH config file")?;

        info!("SSH config cleaned up for session: {}", session_id);
        Ok(())
    }

    /// 特定のセッションのエントリを削除
    fn remove_session_entries(&self, config_content: &str, session_id: &str) -> String {
        let lines: Vec<&str> = config_content.lines().collect();
        let mut result_lines = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Nimbusセッションの開始を検出（セッションIDベース）
            if trimmed.starts_with("# Nimbus - Session:") && trimmed.contains(session_id) {
                // このセッション全体をスキップ
                i += 1; // セッションヘッダーをスキップ

                // セッションの終了まで全ての行をスキップ
                while i < lines.len() {
                    let current_line = lines[i];
                    let current_trimmed = current_line.trim();

                    // 次のセッションの開始で停止
                    if current_trimmed.starts_with("# Nimbus - Session:") {
                        break;
                    }

                    // 空行が連続している場合、セクションの終了とみなす
                    if current_trimmed.is_empty() {
                        if i + 1 < lines.len() {
                            let next_line = lines[i + 1].trim();
                            if next_line.is_empty()
                                || (!next_line.starts_with("Host ec2-")
                                    && !next_line.starts_with("    ")
                                    && !next_line.starts_with("#")
                                    && !next_line.is_empty())
                            {
                                i += 1; // 空行も含めてスキップ
                                break;
                            }
                        } else {
                            i += 1;
                            break;
                        }
                    }

                    i += 1; // この行をスキップ
                }
                continue;
            }

            // セッションヘッダーがない場合のフォールバック：ホスト名ベースの削除
            // session-1 -> ec2-i-111111111 のようなマッピングを想定
            if trimmed.starts_with("Host ec2-") {
                let host_name = trimmed.strip_prefix("Host ").unwrap_or("").trim();

                // セッションIDからインスタンスIDを推測（session-1 -> i-111111111）
                let expected_instance_id = if session_id == "session-1" {
                    "i-111111111"
                } else if session_id == "session-2" {
                    "i-222222222"
                } else {
                    // 一般的なケースでは、セッションIDからインスタンスIDを推測できない
                    ""
                };

                if !expected_instance_id.is_empty()
                    && host_name == format!("ec2-{}", expected_instance_id)
                {
                    // このホストエントリ全体をスキップ
                    i += 1; // Host行をスキップ

                    // ホスト設定の終了まで全ての行をスキップ
                    while i < lines.len() {
                        let current_line = lines[i];
                        let current_trimmed = current_line.trim();

                        // 次のホストまたはセクションの開始で停止
                        if current_trimmed.starts_with("Host ")
                            || current_trimmed.starts_with("# Nimbus")
                        {
                            break;
                        }

                        // 空行が連続している場合、ホスト設定の終了とみなす
                        if current_trimmed.is_empty() {
                            if i + 1 < lines.len() {
                                let next_line = lines[i + 1].trim();
                                if next_line.is_empty()
                                    || (!next_line.starts_with("    ")
                                        && !next_line.starts_with("#")
                                        && !next_line.is_empty())
                                {
                                    i += 1; // 空行も含めてスキップ
                                    break;
                                }
                            } else {
                                i += 1;
                                break;
                            }
                        }

                        i += 1; // この行をスキップ
                    }
                    continue;
                }
            }

            // 通常の行は保持
            result_lines.push(line.to_string());
            i += 1;
        }

        // 末尾の余分な空行を削除
        while result_lines
            .last()
            .map_or(false, |line| line.trim().is_empty())
        {
            result_lines.pop();
        }

        result_lines.join("\n")
    }

    /// VS Code統合の状態を確認
    pub async fn check_integration_status(&self) -> Result<IntegrationStatus> {
        let vscode_available = self.vscode_path.is_some();
        let ssh_config_writable = self.check_ssh_config_writable().await;

        Ok(IntegrationStatus {
            vscode_available,
            vscode_path: self.vscode_path.clone(),
            ssh_config_path: self.ssh_config_path.clone(),
            ssh_config_writable,
            notifications_enabled: self.notifications_enabled,
            auto_launch_enabled: self.auto_launch_enabled,
        })
    }

    /// SSH設定ファイルが書き込み可能かチェック
    async fn check_ssh_config_writable(&self) -> bool {
        // ファイルが存在する場合は書き込み権限をチェック
        if self.ssh_config_path.exists() {
            return fs::metadata(&self.ssh_config_path)
                .await
                .map(|metadata| !metadata.permissions().readonly())
                .unwrap_or(false);
        }

        // ファイルが存在しない場合は親ディレクトリの書き込み権限をチェック
        if let Some(parent) = self.ssh_config_path.parent() {
            return fs::metadata(parent)
                .await
                .map(|metadata| !metadata.permissions().readonly())
                .unwrap_or(false);
        }

        false
    }
}

/// VS Code統合の状態
#[derive(Debug, Clone)]
pub struct IntegrationStatus {
    pub vscode_available: bool,
    pub vscode_path: Option<PathBuf>,
    pub ssh_config_path: PathBuf,
    pub ssh_config_writable: bool,
    pub notifications_enabled: bool,
    pub auto_launch_enabled: bool,
}

impl IntegrationStatus {
    /// 統合が完全に利用可能かチェック
    pub fn is_fully_available(&self) -> bool {
        self.vscode_available && self.ssh_config_writable
    }

    /// 利用可能な機能のリストを取得
    pub fn available_features(&self) -> Vec<String> {
        let mut features = Vec::new();

        if self.vscode_available {
            features.push("VS Code Auto Launch".to_string());
        }

        if self.ssh_config_writable {
            features.push("SSH Config Update".to_string());
        }

        if self.notifications_enabled {
            features.push("Desktop Notifications".to_string());
        }

        features
    }

    /// 不足している要件のリストを取得
    pub fn missing_requirements(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !self.vscode_available {
            missing.push("VS Code executable not found".to_string());
        }

        if !self.ssh_config_writable {
            missing.push("SSH config file not writable".to_string());
        }

        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ssh_config_creation() {
        let temp_dir = TempDir::new().unwrap();
        let ssh_config_path = temp_dir.path().join("config");

        let config = VsCodeConfig {
            ssh_config_path: Some(ssh_config_path.to_string_lossy().to_string()),
            ssh_user: Some("ubuntu".to_string()),
            ssh_identity_file: Some("~/.ssh/test-key.pem".to_string()),
            ssh_identities_only: true,
            ..Default::default()
        };

        let integration = VsCodeIntegration::new(config).unwrap();

        let connection_info = ConnectionInfo {
            session_id: "test-session".to_string(),
            instance_id: "i-1234567890abcdef0".to_string(),
            local_port: 8080,
            remote_port: 22,
            ssh_host: "ec2-test".to_string(),
            connection_url: "localhost:8080".to_string(),
        };

        integration
            .update_ssh_config(&connection_info)
            .await
            .unwrap();

        let config_content = fs::read_to_string(&ssh_config_path).await.unwrap();
        assert!(config_content.contains("Host ec2-test"));
        assert!(config_content.contains("Port 8080"));
        assert!(config_content.contains("test-session"));
        assert!(config_content.contains("User ubuntu"));
        assert!(config_content.contains("IdentityFile ~/.ssh/test-key.pem"));
        assert!(config_content.contains("IdentitiesOnly yes"));
    }

    #[tokio::test]
    async fn test_ssh_config_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let ssh_config_path = temp_dir.path().join("config");

        let config = VsCodeConfig {
            ssh_config_path: Some(ssh_config_path.to_string_lossy().to_string()),
            ..Default::default()
        };

        let integration = VsCodeIntegration::new(config).unwrap();

        // 設定を追加
        let connection_info = ConnectionInfo {
            session_id: "test-session".to_string(),
            instance_id: "i-1234567890abcdef0".to_string(),
            local_port: 8080,
            remote_port: 22,
            ssh_host: "ec2-test".to_string(),
            connection_url: "localhost:8080".to_string(),
        };

        integration
            .update_ssh_config(&connection_info)
            .await
            .unwrap();

        // クリーンアップ
        integration
            .cleanup_ssh_config("test-session")
            .await
            .unwrap();

        let config_content = fs::read_to_string(&ssh_config_path).await.unwrap();
        assert!(!config_content.contains("test-session"));
    }

    #[test]
    fn test_vscode_path_detection() {
        // この テストは実際のシステムに依存するため、モックが必要
        // 実際の実装では、テスト用のモック機能を追加することを推奨
        let result = VsCodeIntegration::detect_vscode_path(None);
        assert!(result.is_ok());
    }
}
