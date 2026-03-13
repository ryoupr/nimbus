# Nimbus API Reference

## 概要

Nimbus v3.0 の完全な API リファレンスです。このドキュメントでは、すべてのコマンド、オプション、設定項目、および内部 API について詳しく説明します。

## 目次

- [CLI コマンド](#cli-コマンド)
- [設定 API](#設定-api)
- [セッション管理 API](#セッション管理-api)
- [診断 API](#診断-api)
- [パフォーマンス監視 API](#パフォーマンス監視-api)
- [VS Code 統合 API](#vs-code-統合-api)
- [データベース API](#データベース-api)
- [エラーハンドリング](#エラーハンドリング)

## CLI コマンド

> **Feature Flags について**
>
> 一部のコマンドは Cargo feature flag を有効にしてビルドする必要があります。
>
> | Feature Flag | 対象コマンド |
> |---|---|
> | `performance-monitoring` | `metrics` |
> | `persistence` | `database` |
> | `multi-session` | `multi-session` |
> | `auto-reconnect` | 自動再接続機能 |
>
> すべて有効にするには: `cargo build --features advanced`

### 基本コマンド

#### `connect` - EC2 インスタンスに接続

```bash
nimbus connect [OPTIONS] (--instance-id <INSTANCE_ID> | --target <NAME>)
```

**必須パラメータ（いずれか）:**

- `--instance-id, -i <INSTANCE_ID>` - EC2 インスタンス ID
- `--target <NAME>` - targets ファイルから選択する接続先名

**オプションパラメータ:**

- `--targets-file <PATH>` - targets ファイルのパス（省略時は `~/.config/nimbus/targets.json`）
- `--local-port, -l <PORT>` - ローカルポート番号 (デフォルト: 8080)
- `--remote-port, -r <PORT>` - リモートポート番号 (デフォルト: 80)
- `--remote-host <HOST>` - リモートホスト（踏み台インスタンス経由で内部 ALB 等に接続する場合。`AWS-StartPortForwardingSessionToRemoteHost` を使用）
- `--profile, -p <PROFILE>` - AWS プロファイル名
- `--region <REGION>` - AWS リージョン
- `--priority <PRIORITY>` - セッション優先度 (low, normal, high, critical)
- `--precheck` - 接続前に予防的チェックを実行

**解決ルール:**

- CLI で指定した値が最優先。未指定の項目は targets の値を採用します。

**例:**

```bash
# 基本的な接続
nimbus connect -i i-1234567890abcdef0

# targets ファイルから接続（例: ~/.config/nimbus/targets.json）
nimbus connect --target dev

# targets ファイルのパスを明示
nimbus connect --targets-file ~/.config/nimbus/targets.json --target dev

# カスタムポートとプロファイル
nimbus connect -i i-1234567890abcdef0 -l 8080 -r 443 -p production

# リモートホスト経由でポートフォワード（踏み台経由で内部ALB等に接続）
nimbus connect -i i-1234567890abcdef0 -l 10443 -r 443 \
  --remote-host internal-alb-xxx.ap-northeast-1.elb.amazonaws.com

# 高優先度セッション
nimbus connect -i i-1234567890abcdef0 --priority high

# 接続前チェック付き
nimbus connect -i i-1234567890abcdef0 --precheck
```

**戻り値:**

- 成功時: 0
- 接続失敗: 1
- 設定エラー: 2
- AWS エラー: 3

#### `list` - アクティブセッション一覧

```bash
nimbus list
```

**出力形式:**

```
📋 Active Sessions:
  • Session ID: session-abc123
    Target: i-1234567890abcdef0
    Status: Active
    Region: us-east-1
    Created: 2024-01-15 10:30:00 UTC
```

#### `terminate` - セッション終了

```bash
nimbus terminate <SESSION_ID>
```

**パラメータ:**

- `<SESSION_ID>` - 終了するセッション ID

#### `status` - セッション状態確認

```bash
nimbus status [SESSION_ID]
```

**パラメータ:**

- `[SESSION_ID]` - 特定のセッション ID (省略時は全セッション)

### UI コマンド

#### `tui` - ターミナル UI 起動

```bash
nimbus tui
```

**機能:**

- リアルタイムセッション監視
- リソース使用量表示
- 進捗インジケーター
- 警告・通知表示

**キーバインド:**

- `q` - 終了
- `r` - 更新
- `↑/↓` - ナビゲーション
- `Enter` - 選択

#### `multi-session` - マルチセッション管理 UI

> ⚠️ `multi-session` feature flag が必要です（`cargo build --features multi-session`）

```bash
nimbus multi-session
```

**機能:**

- 複数セッション同時管理
- リソース監視
- セッション優先度制御
- 統合状態表示

**タブ:**

- `1` - セッション一覧
- `2` - リソース監視
- `3` - 警告・通知
- `4` - 詳細情報

### 監視・診断コマンド

#### `metrics` - パフォーマンスメトリクス表示

```bash
nimbus metrics
```

**出力項目:**

- メモリ使用量 (MB)
- CPU 使用率 (%)
- アクティブプロセス数
- リソース制限違反
- 効率性メトリクス

#### `resources` - リソース管理

```bash
nimbus resources
```

**機能:**

- 現在のリソース使用状況
- 最適化の実行
- 監視状態の確認
- 省電力モード制御

#### `health` - ヘルスチェック

```bash
nimbus health [OPTIONS] [SESSION_ID]
```

**オプション:**

- `--comprehensive, -c` - 包括的ヘルスチェック

**チェック項目:**

- SSM セッション健全性
- ネットワーク接続性
- リソース可用性
- AWS サービス状態

### 診断コマンド

#### `diagnose` - 包括的診断

```bash
nimbus diagnose <SUBCOMMAND>
```

**サブコマンド:**

##### `full` - 完全診断

```bash
nimbus diagnose full [OPTIONS] --instance-id <INSTANCE_ID>
```

**オプション:**

- `--instance-id, -i <ID>` - EC2 インスタンス ID
- `--local-port <PORT>` - ローカルポート
- `--remote-port <PORT>` - リモートポート
- `--profile, -p <PROFILE>` - AWS プロファイル
- `--region <REGION>` - AWS リージョン
- `--parallel` - 並列実行 (デフォルト: true)
- `--timeout <SECONDS>` - タイムアウト (デフォルト: 30)

##### `preventive` - 予防的チェック

```bash
nimbus diagnose preventive [OPTIONS] --instance-id <INSTANCE_ID>
```

**機能:**

- 接続前の事前チェック
- 問題の早期発見
- 接続成功率の予測
- 推奨事項の提示

##### `aws-config` - AWS 設定検証

```bash
nimbus diagnose aws-config [OPTIONS] --instance-id <INSTANCE_ID>
```

**検証項目:**

- AWS 認証情報
- IAM 権限
- VPC 設定
- セキュリティグループ
- SSM エージェント状態

##### `interactive` - インタラクティブ診断

```bash
nimbus diagnose interactive [OPTIONS] --instance-id <INSTANCE_ID>
```

**機能:**

- リアルタイム UI
- 進捗表示
- 色分け表示
- 自動更新

##### `precheck` - 接続前チェック（diagnose サブコマンド）

```bash
nimbus diagnose precheck [OPTIONS] --instance-id <INSTANCE_ID>
```

**オプション:**

- `--instance-id, -i <ID>` - EC2 インスタンス ID
- `--local-port <PORT>` - ローカルポート
- `--profile, -p <PROFILE>` - AWS プロファイル
- `--region <REGION>` - AWS リージョン

##### `item` - 個別診断項目の実行

```bash
nimbus diagnose item [OPTIONS] --item <NAME> --instance-id <INSTANCE_ID>
```

**オプション:**

- `--item, -t <NAME>` - 診断項目名
- `--instance-id, -i <ID>` - EC2 インスタンス ID
- `--local-port <PORT>` - ローカルポート
- `--remote-port <PORT>` - リモートポート
- `--profile, -p <PROFILE>` - AWS プロファイル
- `--region <REGION>` - AWS リージョン

##### `list` - 利用可能な診断項目の一覧

```bash
nimbus diagnose list
```

##### `aws-config-integrated` - 統合 AWS 設定検証

クロスバリデーションとキャッシュ機能付きの AWS 設定検証を実行します。

```bash
nimbus diagnose aws-config-integrated [OPTIONS] --instance-id <INSTANCE_ID>
```

**オプション:**

- `--instance-id, -i <ID>` - EC2 インスタンス ID
- `--profile, -p <PROFILE>` - AWS プロファイル
- `--region <REGION>` - AWS リージョン
- `--include-credentials` - 認証情報検証を含む (デフォルト: true)
- `--include-iam` - IAM 権限検証を含む (デフォルト: true)
- `--include-vpc` - VPC 設定検証を含む (デフォルト: true)
- `--include-security-groups` - セキュリティグループ検証を含む (デフォルト: true)
- `--minimum-score <SCORE>` - 最低コンプライアンススコア (デフォルト: 75.0)
- `--clear-cache` - 検証前にキャッシュをクリア (デフォルト: false)

##### `settings` - 診断設定の管理

```bash
nimbus diagnose settings <SUBCOMMAND>
```

**サブコマンド:**

- `show` - 現在の診断設定を表示
- `enable <CHECK_NAME>` - 診断チェックを有効化
- `disable <CHECK_NAME>` - 診断チェックを無効化
- `auto-fix --enable [--safe-only]` - 自動修復モードを設定
- `parallel <true|false>` - 並列実行モードを設定
- `timeout <SECONDS>` - デフォルトタイムアウトを設定
- `format <text|json|yaml>` - レポート形式を設定
- `reset` - デフォルト設定にリセット

#### `precheck` - 接続前チェック

```bash
nimbus precheck [OPTIONS] --instance-id <INSTANCE_ID>
```

**出力形式:**

- `text` - 人間が読みやすい形式
- `json` - 機械処理用
- `yaml` - 構造化データ

#### `fix` - 自動修復

```bash
nimbus fix [OPTIONS] --instance-id <INSTANCE_ID>
```

**オプション:**

- `--auto-fix` - 確認なしで自動修復
- `--safe-only` - 安全な修復のみ
- `--dry-run` - 実行せずに表示のみ

### 設定管理コマンド

#### `config` - 設定管理

```bash
nimbus config <SUBCOMMAND>
```

**サブコマンド:**

##### `validate` - 設定検証

```bash
nimbus config validate
```

##### `show` - 設定表示

```bash
nimbus config show
```

##### `generate` - 設定ファイル生成

```bash
nimbus config generate [OPTIONS]
```

**オプション:**

- `--output, -o <FILE>` - 出力ファイル (デフォルト: config.json)
- `--format, -f <FORMAT>` - 形式 (json, toml)

##### `env-help` - 環境変数ヘルプ

```bash
nimbus config env-help
```

##### `test` - 設定テスト

```bash
nimbus config test
```

### VS Code 統合コマンド

#### `vscode` - VS Code 統合

```bash
nimbus vscode <SUBCOMMAND>
```

**サブコマンド:**

##### `status` - 統合状態確認

```bash
nimbus vscode status
```

##### `test` - 統合テスト

```bash
nimbus vscode test [SESSION_ID]
```

##### `setup` - 統合設定

```bash
nimbus vscode setup
```

##### `cleanup` - SSH 設定クリーンアップ

```bash
nimbus vscode cleanup [SESSION_ID]
```

### データベース管理コマンド

#### `database` - データベース管理

> ⚠️ `persistence` feature flag が必要です（`cargo build --features persistence`）

```bash
nimbus database <SUBCOMMAND>
```

**サブコマンド:**

##### `init` - データベース初期化

```bash
nimbus database init
```

##### `info` - データベース情報

```bash
nimbus database info
```

##### `sessions` - 保存済みセッション一覧

```bash
nimbus database sessions
```

##### `stats` - パフォーマンス統計

```bash
nimbus database stats [SESSION_ID]
```

##### `cleanup` - 古いデータ削除

```bash
nimbus database cleanup [OPTIONS]
```

**オプション:**

- `--days, -d <DAYS>` - 保持期間 (デフォルト: 30)

##### `export` - データエクスポート

```bash
nimbus database export [OPTIONS]
```

**オプション:**

- `--output, -o <FILE>` - 出力ファイル
- `--format, -f <FORMAT>` - 形式 (json, csv)

## 設定 API

### 設定ファイル構造

```json
{
  "aws": {
    "default_region": "us-east-1",
    "default_profile": null,
    "connection_timeout": 30,
    "request_timeout": 60
  },
  "session": {
    "max_sessions_per_instance": 3,
    "health_check_interval": 5,
    "inactive_timeout": 30,
    "timeout_prediction_threshold": 300,
    "reconnection": {
      "enabled": true,
      "max_attempts": 5,
      "base_delay_ms": 1000,
      "max_delay_ms": 16000,
      "aggressive_mode": false,
      "aggressive_attempts": 10,
      "aggressive_interval_ms": 500
    }
  },
  "performance": {
    "monitoring_enabled": true,
    "metrics_interval": 10,
    "latency_threshold_ms": 200,
    "optimization_enabled": true
  },
  "resources": {
    "max_memory_mb": 10.0,
    "max_cpu_percent": 0.5,
    "low_power_mode": true,
    "monitoring_interval": 5
  },
  "ui": {
    "rich_ui": true,
    "update_interval_ms": 1000,
    "show_progress": true,
    "notifications": true
  },
  "logging": {
    "level": "info",
    "file_logging": true,
    "log_file": null,
    "json_format": false
  },
  "vscode": {
    "auto_launch_enabled": false,
    "auto_update_ssh_config": true,
    "ssh_config_path": null,
    "vscode_path": null,
    "notifications_enabled": true
  }
}
```

### 環境変数オーバーライド

すべての設定項目は環境変数で上書き可能です：

```bash
# AWS 設定
export NIMBUS_AWS_REGION=us-west-2
export NIMBUS_AWS_PROFILE=production
export NIMBUS_CONNECTION_TIMEOUT=45
export NIMBUS_REQUEST_TIMEOUT=90

# セッション管理
export NIMBUS_MAX_SESSIONS=5
export NIMBUS_HEALTH_CHECK_INTERVAL=3
export NIMBUS_INACTIVE_TIMEOUT=60

# 再接続ポリシー
export NIMBUS_RECONNECTION_ENABLED=true
export NIMBUS_MAX_RECONNECTION_ATTEMPTS=10
export NIMBUS_RECONNECTION_BASE_DELAY_MS=2000
export NIMBUS_RECONNECTION_MAX_DELAY_MS=30000
export NIMBUS_AGGRESSIVE_RECONNECTION=true
export NIMBUS_AGGRESSIVE_ATTEMPTS=15
export NIMBUS_AGGRESSIVE_INTERVAL_MS=250

# パフォーマンス監視
export NIMBUS_PERFORMANCE_MONITORING=true
export NIMBUS_LATENCY_THRESHOLD_MS=150
export NIMBUS_OPTIMIZATION_ENABLED=true

# リソース制限
export NIMBUS_MAX_MEMORY_MB=8
export NIMBUS_MAX_CPU_PERCENT=0.3
export NIMBUS_LOW_POWER_MODE=true

# UI 設定
export NIMBUS_RICH_UI=false
export NIMBUS_UI_UPDATE_INTERVAL_MS=500
export NIMBUS_NOTIFICATIONS=false

# ログ設定
export NIMBUS_LOG_LEVEL=debug
export NIMBUS_FILE_LOGGING=true
export NIMBUS_JSON_LOGGING=true

# VS Code 統合
export NIMBUS_VSCODE_AUTO_LAUNCH=true
export NIMBUS_VSCODE_SSH_CONFIG_UPDATE=true
export NIMBUS_VSCODE_PATH=/usr/local/bin/code
```

## セッション管理 API

### セッション状態

```rust
pub enum SessionStatus {
    Connecting,    // 接続中
    Active,        // アクティブ
    Inactive,      // 非アクティブ
    Reconnecting,  // 再接続中
    Terminated,    // 終了済み
}
```

### セッション優先度

```rust
pub enum SessionPriority {
    Low,       // 低優先度
    Normal,    // 通常優先度
    High,      // 高優先度
    Critical,  // 重要優先度
}
```

### セッション設定

```rust
pub struct SessionConfig {
    pub instance_id: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub aws_profile: Option<String>,
    pub region: String,
    pub priority: SessionPriority,
    pub tags: HashMap<String, String>,
}
```

### 再接続ポリシー

```rust
pub struct ReconnectionPolicy {
    pub enabled: bool,
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub aggressive_mode: bool,
    pub aggressive_attempts: u32,
    pub aggressive_interval: Duration,
}
```

**プリセット:**

- `ReconnectionPolicy::new()` - デフォルト (5回試行、指数バックオフ)
- `ReconnectionPolicy::aggressive()` - アグレッシブ (10回試行、500ms間隔)
- `ReconnectionPolicy::conservative()` - 保守的 (3回試行、長い間隔)
- `ReconnectionPolicy::disabled()` - 無効

## 診断 API

### 診断結果

```rust
pub struct DiagnosticResult {
    pub item_name: String,
    pub status: DiagnosticStatus,
    pub message: String,
    pub details: Option<String>,
    pub execution_time_ms: u64,
    pub recommendations: Vec<String>,
}

pub enum DiagnosticStatus {
    Pass,     // 成功
    Warning,  // 警告
    Fail,     // 失敗
    Skip,     // スキップ
}
```

### 予防的チェック結果

```rust
pub struct PreventiveCheckResult {
    pub overall_status: PreventiveCheckStatus,
    pub connection_likelihood: ConnectionLikelihood,
    pub should_abort_connection: bool,
    pub critical_issues: Vec<DiagnosticResult>,
    pub warnings: Vec<DiagnosticResult>,
    pub recommendations: Vec<String>,
    pub execution_time_ms: u64,
}

pub enum ConnectionLikelihood {
    VeryHigh,   // 95-100%
    High,       // 80-94%
    Medium,     // 60-79%
    Low,        // 30-59%
    VeryLow,    // 0-29%
}
```

### AWS 設定検証結果

```rust
pub struct AwsConfigValidationResult {
    pub overall_score: f64,
    pub compliance_level: ComplianceLevel,
    pub credential_validation: ValidationResult,
    pub iam_validation: ValidationResult,
    pub vpc_validation: ValidationResult,
    pub security_group_validation: ValidationResult,
    pub recommendations: Vec<String>,
    pub execution_time_ms: u64,
}

pub enum ComplianceLevel {
    Excellent,  // 90-100%
    Good,       // 75-89%
    Fair,       // 60-74%
    Poor,       // 0-59%
}
```

## パフォーマンス監視 API

### リソース使用量

```rust
pub struct ResourceUsage {
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub process_count: u32,
    pub active_sessions: u32,
    pub timestamp: SystemTime,
}
```

### パフォーマンスメトリクス

```rust
pub struct PerformanceMetrics {
    pub session_id: String,
    pub connection_time: f64,    // ミリ秒
    pub latency: f64,            // ミリ秒
    pub throughput: f64,         // MB/s
    pub cpu_usage: f64,          // %
    pub memory_usage: f64,       // MB
    pub timestamp: SystemTime,
}
```

### 効率性メトリクス

```rust
pub struct EfficiencyMetrics {
    pub memory_efficiency_percent: f64,
    pub cpu_efficiency_percent: f64,
    pub low_power_mode_active: bool,
    pub uptime_seconds: u64,
    pub optimization_count: u32,
}
```

## VS Code 統合 API

### 統合状態

```rust
pub struct VsCodeIntegrationStatus {
    pub vscode_available: bool,
    pub vscode_path: Option<PathBuf>,
    pub ssh_config_writable: bool,
    pub ssh_config_path: PathBuf,
    pub auto_launch_enabled: bool,
    pub notifications_enabled: bool,
}
```

### 統合結果

```rust
pub struct VsCodeIntegrationResult {
    pub success: bool,
    pub ssh_config_updated: bool,
    pub vscode_launched: bool,
    pub connection_info: Option<SshConnectionInfo>,
    pub error_message: Option<String>,
}

pub struct SshConnectionInfo {
    pub ssh_host: String,
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub proxy_command: String,
}
```

## データベース API

### データベース情報

```rust
pub struct DatabaseInfo {
    pub db_path: PathBuf,
    pub schema_version: u32,
    pub session_count: u64,
    pub metrics_count: u64,
    pub file_size_bytes: u64,
}
```

### パフォーマンス統計

```rust
pub struct PerformanceStatistics {
    pub session_id: String,
    pub total_measurements: u64,
    pub avg_connection_time_ms: f64,
    pub min_connection_time_ms: f64,
    pub max_connection_time_ms: f64,
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub avg_throughput_mbps: f64,
    pub max_throughput_mbps: f64,
    pub avg_cpu_usage_percent: f64,
    pub max_cpu_usage_percent: f64,
    pub avg_memory_usage_mb: f64,
    pub max_memory_usage_mb: f64,
}
```

## エラーハンドリング

### エラー型

```rust
pub enum NimbusError {
    // AWS 関連エラー
    Aws(AwsError),
    
    // セッション関連エラー
    Session(SessionError),
    
    // 接続関連エラー
    Connection(ConnectionError),
    
    // 設定関連エラー
    Configuration(ConfigurationError),
    
    // システム関連エラー
    System(String),
}

pub enum AwsError {
    AuthenticationFailed { message: String },
    PermissionDenied { action: String, resource: String },
    ServiceUnavailable { service: String, region: String },
    RateLimitExceeded { retry_after: Option<Duration> },
    InvalidRegion { region: String },
    InvalidProfile { profile: String },
}

pub enum SessionError {
    CreationFailed { reason: String },
    NotFound { session_id: String },
    AlreadyExists { session_id: String },
    InvalidState { current_state: String, expected_state: String },
    ResourceLimitExceeded { resource: String, limit: String },
}

pub enum ConnectionError {
    Timeout { duration: Duration },
    NetworkUnreachable { target: String },
    PortInUse { port: u16 },
    PreventiveCheckFailed { reason: String, issues: Vec<String> },
    SsmSessionFailed { reason: String },
}

pub enum ConfigurationError {
    FileNotFound { path: PathBuf },
    InvalidFormat { reason: String },
    ValidationFailed { field: String, reason: String },
    EnvironmentVariableInvalid { name: String, value: String },
}
```

### エラー回復

```rust
pub trait ErrorRecovery {
    fn is_recoverable(&self) -> bool;
    fn recovery_suggestions(&self) -> Vec<String>;
    fn retry_delay(&self) -> Option<Duration>;
}
```

### 終了コード

| コード | 意味 | 説明 |
|--------|------|------|
| 0 | 成功 | 正常終了 |
| 1 | 一般エラー | 予期しないエラー |
| 2 | 設定エラー | 設定ファイルまたは環境変数の問題 |
| 3 | AWS エラー | AWS API または認証の問題 |
| 4 | 接続エラー | ネットワークまたは SSM 接続の問題 |
| 5 | セッションエラー | セッション管理の問題 |
| 6 | リソースエラー | システムリソースの問題 |
| 7 | 権限エラー | ファイルまたはシステム権限の問題 |

## 使用例

### 基本的な使用パターン

```bash
# 1. 設定確認
nimbus config validate

# 2. 予防的チェック
nimbus diagnose preventive -i i-1234567890abcdef0

# 3. 接続
nimbus connect -i i-1234567890abcdef0 -l 8080 -r 80

# 4. 状態監視
nimbus tui

# 5. セッション終了
nimbus terminate session-abc123
```

### 高度な使用パターン

```bash
# 包括的診断とレポート出力
nimbus diagnose full -i i-1234567890abcdef0 --timeout 60 > diagnostic-report.txt

# 自動修復付き接続
nimbus fix -i i-1234567890abcdef0 --auto-fix --safe-only
nimbus connect -i i-1234567890abcdef0

# パフォーマンス監視とデータエクスポート
nimbus metrics
nimbus database export -o performance-data.json -f json

# VS Code 統合セットアップ
nimbus vscode setup
nimbus connect -i i-1234567890abcdef0 --priority high
```

## 参考資料

- [設定ガイド](CONFIGURATION.md)
- [データモデル仕様](DATA_MODELS.md)
- [使用例とチュートリアル](TUTORIALS.md)
- [トラブルシューティングガイド](TROUBLESHOOTING.md)
- [パフォーマンス最適化ガイド](PERFORMANCE_OPTIMIZATION.md)
