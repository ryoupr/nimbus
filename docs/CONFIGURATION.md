# EC2 Connect 設定ガイド

## 設定ファイル

EC2 Connect は JSON と TOML の両形式をサポートしています：

- **JSON**: `config.json`（デフォルト）
- **TOML**: `config.toml`

### デフォルトの設定ファイル場所

- **Linux/macOS**: `~/.config/ec2-connect/config.json`
- **Windows**: `%APPDATA%\ec2-connect\config.json`

### サンプルファイル

サンプル設定ファイルをコピーしてカスタマイズしてください：

```bash
# JSON形式
cp config.json.example ~/.config/ec2-connect/config.json

# TOML形式
cp config.toml.example ~/.config/ec2-connect/config.toml
```

## 設定ファイルリファレンス

### aws - AWS接続設定

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `default_profile` | string/null | `null` | デフォルトで使用するAWSプロファイル名 |
| `default_region` | string | `"us-east-1"` | デフォルトのAWSリージョン |
| `connection_timeout` | number | `30` | AWS接続タイムアウト（秒） |
| `request_timeout` | number | `60` | AWSリクエストタイムアウト（秒） |

### session - セッション管理設定

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `max_sessions_per_instance` | number | `3` | インスタンスあたりの最大同時セッション数 |
| `health_check_interval` | number | `5` | ヘルスチェック間隔（秒） |
| `timeout_prediction_threshold` | number | `300` | タイムアウト予測の閾値（秒） |
| `inactive_timeout` | number | `30` | 非アクティブセッションのタイムアウト（秒） |

### session.reconnection - 再接続ポリシー

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `enabled` | boolean | `true` | 自動再接続を有効化 |
| `max_attempts` | number | `5` | 最大再接続試行回数 |
| `base_delay_ms` | number | `1000` | 再接続の基本遅延（ミリ秒） |
| `max_delay_ms` | number | `16000` | 再接続の最大遅延（ミリ秒） |
| `aggressive_mode` | boolean | `false` | アグレッシブ再接続モード |
| `aggressive_attempts` | number | `10` | アグレッシブモードでの試行回数 |
| `aggressive_interval_ms` | number | `500` | アグレッシブモードでの試行間隔（ミリ秒） |

### performance - パフォーマンス監視設定

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `monitoring_enabled` | boolean | `true` | パフォーマンス監視を有効化 |
| `metrics_interval` | number | `10` | メトリクス収集間隔（秒） |
| `latency_threshold_ms` | number | `200` | レイテンシ警告の閾値（ミリ秒） |
| `optimization_enabled` | boolean | `true` | 自動最適化を有効化 |

### resources - リソース制限設定

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `max_memory_mb` | number | `10` | 最大メモリ使用量（MB） |
| `max_cpu_percent` | number | `0.5` | 最大CPU使用率（%） |
| `low_power_mode` | boolean | `true` | 省電力モードを有効化 |
| `monitoring_interval` | number | `5` | リソース監視間隔（秒） |

### ui - ユーザーインターフェース設定

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `rich_ui` | boolean | `true` | リッチターミナルUIを有効化 |
| `update_interval_ms` | number | `1000` | UI更新間隔（ミリ秒） |
| `show_progress` | boolean | `true` | 進捗表示を有効化 |
| `notifications` | boolean | `true` | 通知を有効化 |

### logging - ログ設定

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `level` | string | `"info"` | ログレベル（trace/debug/info/warn/error） |
| `file_logging` | boolean | `true` | ファイルへのログ出力を有効化 |
| `log_file` | string/null | `null` | ログファイルパス（nullの場合はデフォルトパス） |
| `json_format` | boolean | `false` | JSON形式でログを出力 |

### vscode - VS Code統合設定

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `vscode_path` | string/null | `null` | VS Code実行ファイルのパス |
| `ssh_config_path` | string/null | `null` | SSH設定ファイルのパス |
| `auto_launch_enabled` | boolean | `true` | 接続時にVS Codeを自動起動 |
| `notifications_enabled` | boolean | `true` | VS Code関連の通知を有効化 |
| `launch_delay_seconds` | number | `2` | VS Code起動までの遅延（秒） |
| `auto_update_ssh_config` | boolean | `true` | SSH設定を自動更新 |
| `ssh_user` | string/null | `null` | SSH接続のデフォルトユーザー名 |
| `ssh_identity_file` | string/null | `null` | SSH秘密鍵のパス |
| `ssh_identities_only` | boolean | `false` | 指定した鍵のみを使用 |

### 設定ファイル例

```json
{
  "aws": {
    "default_profile": null,
    "default_region": "ap-northeast-1",
    "connection_timeout": 30,
    "request_timeout": 60
  },
  "session": {
    "max_sessions_per_instance": 3,
    "health_check_interval": 5,
    "timeout_prediction_threshold": 300,
    "inactive_timeout": 30,
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
    "max_memory_mb": 10,
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
    "vscode_path": null,
    "ssh_config_path": null,
    "auto_launch_enabled": true,
    "notifications_enabled": true,
    "launch_delay_seconds": 2,
    "auto_update_ssh_config": true,
    "ssh_user": null,
    "ssh_identity_file": null,
    "ssh_identities_only": false
  }
}
```

## ターゲットファイル（サーバー別設定）

EC2 Connect は、サーバーごとの接続設定（インスタンスID、ポート、プロファイル/リージョン、SSHユーザー/鍵）を名前で管理する **ターゲットファイル** を読み込めます。

サポート形式：

- **JSON**（デフォルト）: `targets.json`
- **TOML**: `targets.toml`

### デフォルトのターゲットファイル場所

- **Linux/macOS**: `~/.config/ec2-connect/targets.json`
- **Windows**: `%APPDATA%\ec2-connect\targets.json`

### 設定例

リポジトリのサンプルから始めてください：

```bash
cp targets.json.example ~/.config/ec2-connect/targets.json
```

最小限のJSON構造：

```json
{
 "targets": {
  "dev": {
   "instance_id": "i-1234567890abcdef0",
   "local_port": 5555,
   "remote_port": 22,
   "profile": "default",
   "region": "ap-northeast-1",
   "ssh_user": "ubuntu",
   "ssh_identity_file": "~/.ssh/dev.pem",
   "ssh_identities_only": true
  },
  "internal-alb": {
   "instance_id": "i-bastion123456789",
   "local_port": 10443,
   "remote_port": 443,
   "remote_host": "internal-alb-xxx.ap-northeast-1.elb.amazonaws.com",
   "profile": "production",
   "region": "ap-northeast-1"
  }
 }
}
```

ターゲットごとのサポートフィールド：

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| `instance_id` | string | ✅ | EC2インスタンスID |
| `local_port` | number | - | ローカルポート番号 |
| `remote_port` | number | - | リモートポート番号 |
| `remote_host` | string | - | リモートホスト（踏み台経由で内部ALB等に接続する場合） |
| `profile` | string | - | AWSプロファイル名 |
| `region` | string | - | AWSリージョン |
| `ssh_user` | string | - | SSH接続のユーザー名 |
| `ssh_identity_file` | string | - | SSH秘密鍵のパス |
| `ssh_identities_only` | boolean | - | 指定した鍵のみを使用 |

CLIで指定した値はターゲットファイルの値より優先されます。

## 環境変数によるオーバーライド

すべての設定値は環境変数で上書きできます。以下の用途に便利です：

- CI/CD環境
- Dockerコンテナ
- 異なるデプロイ環境
- 一時的な設定変更

### AWS設定

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_AWS_PROFILE` | 使用するAWSプロファイル | `production` |
| `EC2_CONNECT_AWS_REGION` | AWSリージョン | `us-west-2` |
| `EC2_CONNECT_CONNECTION_TIMEOUT` | 接続タイムアウト（秒） | `45` |
| `EC2_CONNECT_REQUEST_TIMEOUT` | リクエストタイムアウト（秒） | `90` |

### セッション管理

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_MAX_SESSIONS` | インスタンスあたりの最大セッション数 | `5` |
| `EC2_CONNECT_HEALTH_CHECK_INTERVAL` | ヘルスチェック間隔（秒） | `3` |
| `EC2_CONNECT_INACTIVE_TIMEOUT` | 非アクティブタイムアウト（秒） | `60` |

### 再接続ポリシー

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_RECONNECTION_ENABLED` | 自動再接続を有効化 | `true` |
| `EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS` | 最大再接続試行回数 | `10` |
| `EC2_CONNECT_RECONNECTION_BASE_DELAY_MS` | 基本遅延（ミリ秒） | `2000` |
| `EC2_CONNECT_RECONNECTION_MAX_DELAY_MS` | 最大遅延（ミリ秒） | `30000` |
| `EC2_CONNECT_AGGRESSIVE_RECONNECTION` | アグレッシブモードを有効化 | `true` |
| `EC2_CONNECT_AGGRESSIVE_ATTEMPTS` | アグレッシブ試行回数 | `15` |
| `EC2_CONNECT_AGGRESSIVE_INTERVAL_MS` | アグレッシブ間隔（ミリ秒） | `250` |

### パフォーマンス監視

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_PERFORMANCE_MONITORING` | 監視を有効化 | `true` |
| `EC2_CONNECT_LATENCY_THRESHOLD_MS` | レイテンシ閾値（ミリ秒） | `150` |
| `EC2_CONNECT_OPTIMIZATION_ENABLED` | 最適化を有効化 | `true` |

### リソース制限

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_MAX_MEMORY_MB` | 最大メモリ使用量（MB） | `8` |
| `EC2_CONNECT_MAX_CPU_PERCENT` | 最大CPU使用率（%） | `0.3` |
| `EC2_CONNECT_LOW_POWER_MODE` | 省電力モードを有効化 | `true` |

### ユーザーインターフェース

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_RICH_UI` | リッチターミナルUIを有効化 | `false` |
| `EC2_CONNECT_UI_UPDATE_INTERVAL_MS` | UI更新間隔（ミリ秒） | `500` |
| `EC2_CONNECT_NOTIFICATIONS` | 通知を有効化 | `false` |

### ログ

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_LOG_LEVEL` | ログレベル | `debug` |
| `EC2_CONNECT_FILE_LOGGING` | ファイルログを有効化 | `true` |
| `EC2_CONNECT_LOG_FILE` | ログファイルパス | `/var/log/ec2-connect.log` |
| `EC2_CONNECT_JSON_LOGGING` | JSON形式を有効化 | `true` |

### VS Code / SSH

| 環境変数 | 説明 | 例 |
|---------|------|-----|
| `EC2_CONNECT_VSCODE_PATH` | VS Code実行ファイルのパス | `/opt/homebrew/bin/code` |
| `EC2_CONNECT_SSH_CONFIG_PATH` | SSH設定ファイルのパス | `~/.ssh/config` |
| `EC2_CONNECT_VSCODE_AUTO_LAUNCH` | VS Codeを自動起動（true/false） | `false` |
| `EC2_CONNECT_VSCODE_NOTIFICATIONS` | 通知を有効化（true/false） | `false` |
| `EC2_CONNECT_VSCODE_LAUNCH_DELAY` | 起動遅延（秒） | `2` |
| `EC2_CONNECT_VSCODE_AUTO_UPDATE_SSH` | SSH設定を自動更新（true/false） | `true` |
| `EC2_CONNECT_SSH_USER` | 生成エントリのSSHユーザー名 | `ubuntu` |
| `EC2_CONNECT_SSH_IDENTITY_FILE` | 生成エントリのSSH IdentityFileパス | `~/.ssh/my-key.pem` |
| `EC2_CONNECT_SSH_IDENTITIES_ONLY` | IdentitiesOnlyを有効化（true/false） | `true` |

メイン設定ファイルの `vscode` セクションでも設定できます：

```json
{
 "vscode": {
  "ssh_user": "ubuntu",
  "ssh_identity_file": "~/.ssh/my-key.pem",
  "ssh_identities_only": true
 }
}
```

## 設定例

### 開発環境

```bash
export EC2_CONNECT_LOG_LEVEL=debug
export EC2_CONNECT_MAX_MEMORY_MB=50
export EC2_CONNECT_PERFORMANCE_MONITORING=true
```

### 本番環境

```bash
export EC2_CONNECT_LOG_LEVEL=warn
export EC2_CONNECT_MAX_MEMORY_MB=10
export EC2_CONNECT_MAX_CPU_PERCENT=0.5
export EC2_CONNECT_LOW_POWER_MODE=true
export EC2_CONNECT_JSON_LOGGING=true
```

### CI/CD環境

```bash
export EC2_CONNECT_RICH_UI=false
export EC2_CONNECT_NOTIFICATIONS=false
export EC2_CONNECT_FILE_LOGGING=false
export EC2_CONNECT_RECONNECTION_ENABLED=false
```

### アグレッシブ再接続モード

```bash
export EC2_CONNECT_AGGRESSIVE_RECONNECTION=true
export EC2_CONNECT_AGGRESSIVE_ATTEMPTS=20
export EC2_CONNECT_AGGRESSIVE_INTERVAL_MS=200
export EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS=50
```

## 設定の検証

EC2 Connect は起動時にすべての設定値を検証し、無効な設定に対して詳細なエラーメッセージを表示します：

- **範囲検証**: 数値が許容範囲内であることを確認
- **型検証**: ブール値が正しい形式であることを確認
- **依存関係検証**: 関連する設定が一貫していることを確認
- **パフォーマンス警告**: パフォーマンスに影響する可能性のある設定を警告

### よくある検証エラー

1. **無効なブール値**: `true` または `false` を使用（大文字小文字を区別）
2. **範囲外の値**: 許容される最小/最大値を確認
3. **不整合な遅延**: `max_delay_ms >= base_delay_ms` であることを確認
4. **ゼロ値**: ほとんどのタイムアウトと間隔の値は 0 より大きい必要があります

## ベストプラクティス

### パフォーマンス最適化

- 最適なパフォーマンスのために `max_memory_mb` は 10 以下に設定
- 他のプロセスへの影響を避けるため `max_cpu_percent` は 0.5 以下に設定
- バッテリー駆動デバイスでは `low_power_mode = true` を使用

### 信頼性

- 本番環境では `reconnection.enabled = true` を有効化
- 過度なリトライを避けるため `max_attempts` は適切な値（5-10）に設定
- 負荷軽減のため本番環境では `aggressive_mode = false` を使用

### 監視

- トラブルシューティングのため `performance.monitoring_enabled = true` を有効化
- ネットワークに応じて適切な `latency_threshold_ms` を設定
- 構造化ログ分析のため `json_format = true` を使用

### セキュリティ

- 設定ファイルに機密情報を保存しない
- 認証情報や機密設定には環境変数を使用
- AWS認証情報とプロファイルを定期的にローテーション
