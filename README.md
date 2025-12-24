# EC2 Connect v3.0 (Rust)

高性能 EC2 SSM 接続管理ツール - 自動セッション管理機能付き

## 概要

EC2 Connect v3.0 は、Rust で完全に書き直された高性能な EC2 インスタンス接続管理ツールです。自動セッション維持、高速再接続、リソース使用量最適化などの機能を提供します。

## 主な機能

- **自動セッション維持**: セッションを自動的に監視し、切断を予防
- **高速再接続**: 5 秒以内の切断検出と自動再接続
- **セッション管理最適化**: 複数セッションの効率的な管理
- **パフォーマンス監視**: 接続速度とレイテンシの継続的な監視
- **リソース使用量最適化**: メモリ 10MB 以下、CPU 0.5%以下の軽量動作
- **リッチターミナル UI**: ratatui による美しいターミナルインターフェース

## パフォーマンス目標

- **メモリ使用量**: 10MB 以下
- **CPU 使用率**: 0.5%以下（通常動作時）
- **接続時間**: 150ms 以下
- **切断検出**: 5 秒以内

## インストール

### 前提条件

- Rust 1.70 以上
- AWS CLI
- AWS Session Manager Plugin

### ビルド

```bash
cd tools/ec2-connect-rust
cargo build --release
```

### インストール

```bash
cargo install --path .
```

## 使用方法

### 基本コマンド

#### 接続管理

```bash
# EC2 インスタンスに接続
ec2-connect connect --instance-id i-1234567890abcdef0 --local-port 8080 --remote-port 80

# セッション一覧表示
ec2-connect list

# セッション状態確認
ec2-connect status [SESSION_ID]

# セッション終了
ec2-connect terminate SESSION_ID
```

#### ユーザーインターフェース

```bash
# ターミナル UI 起動
ec2-connect tui

# マルチセッション管理 UI
ec2-connect multi-session
```

#### 監視・メトリクス

```bash
# パフォーマンスメトリクス表示
ec2-connect metrics

# リソース使用状況確認
ec2-connect resources

# ヘルスチェック実行
ec2-connect health [SESSION_ID] [--comprehensive]
```

### 診断・トラブルシューティング

#### 包括的診断

```bash
# 完全診断実行
ec2-connect diagnose full --instance-id i-1234567890abcdef0 \
  --local-port 8080 --remote-port 80 \
  --profile my-profile --region us-east-1 \
  --parallel --timeout 30

# 事前チェック
ec2-connect diagnose precheck --instance-id i-1234567890abcdef0 \
  --local-port 8080 --profile my-profile

# 予防的チェック
ec2-connect diagnose preventive --instance-id i-1234567890abcdef0 \
  --local-port 8080 --remote-port 22 \
  --abort-on-critical --timeout 30

# 特定項目の診断
ec2-connect diagnose item --item instance_state --instance-id i-1234567890abcdef0

# 利用可能な診断項目一覧
ec2-connect diagnose list

# AWS 設定検証
ec2-connect diagnose aws-config --instance-id i-1234567890abcdef0 \
  --include-credentials --include-iam --include-vpc \
  --minimum-score 75.0

# 統合 AWS 設定検証（キャッシュ機能付き）
ec2-connect diagnose aws-config-integrated --instance-id i-1234567890abcdef0 \
  --clear-cache

# リアルタイム診断 UI
ec2-connect diagnose interactive --instance-id i-1234567890abcdef0 \
  --parallel --no-color --refresh-interval 100
```

#### 事前チェック・自動修復

```bash
# 接続前チェック
ec2-connect precheck --instance-id i-1234567890abcdef0 \
  --local-port 8080 --timeout 15 \
  --format json --output precheck-results.json

# 自動修復実行
ec2-connect fix --instance-id i-1234567890abcdef0 \
  --auto-fix --safe-only --timeout 60

# ドライラン（実行せずに確認）
ec2-connect fix --instance-id i-1234567890abcdef0 \
  --dry-run --format yaml --output fix-plan.yaml
```

### 設定管理

#### 設定ファイル操作

```bash
# 設定検証
ec2-connect config validate

# 現在の設定表示
ec2-connect config show

# 設定ファイル生成
ec2-connect config generate --output ~/.config/ec2-connect/config.json --format json

# 環境変数ヘルプ
ec2-connect config env-help

# 設定テスト（環境変数オーバーライド含む）
ec2-connect config test
```

#### 診断設定管理

```bash
# 診断設定表示
ec2-connect diagnose settings show

# 診断チェック有効化
ec2-connect diagnose settings enable instance_state

# 診断チェック無効化
ec2-connect diagnose settings disable network_connectivity

# 自動修復設定
ec2-connect diagnose settings auto-fix --enable --safe-only

# 並列実行設定
ec2-connect diagnose settings parallel true

# タイムアウト設定
ec2-connect diagnose settings timeout 60

# レポート形式設定
ec2-connect diagnose settings format json

# 設定リセット
ec2-connect diagnose settings reset
```

### データベース管理

```bash
# データベース初期化
ec2-connect database init

# データベース情報表示
ec2-connect database info

# セッション一覧
ec2-connect database sessions

# パフォーマンス統計
ec2-connect database stats [SESSION_ID]

# 古いデータクリーンアップ
ec2-connect database cleanup --days 30

# データエクスポート
ec2-connect database export --output sessions.json --format json
```

### VS Code 統合

```bash
# VS Code 統合状態確認
ec2-connect vscode status

# VS Code 統合テスト
ec2-connect vscode test [SESSION_ID]

# VS Code 統合セットアップ
ec2-connect vscode setup

# SSH 設定クリーンアップ
ec2-connect vscode cleanup [SESSION_ID]
```

## CLI コマンドリファレンス

### 接続コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `connect` | EC2 インスタンスに接続 | `--instance-id`, `--local-port`, `--remote-port`, `--profile`, `--region`, `--priority` |
| `list` | アクティブセッション一覧 | なし |
| `terminate` | セッション終了 | `session_id` |
| `status` | セッション状態確認 | `[session_id]` |

### UI コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `tui` | ターミナル UI 起動 | なし |
| `multi-session` | マルチセッション管理 UI | なし |

### 監視コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `metrics` | パフォーマンスメトリクス表示 | なし |
| `resources` | リソース使用状況表示 | なし |
| `health` | ヘルスチェック実行 | `[session_id]`, `--comprehensive` |

### 診断コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `diagnose full` | 包括的診断実行 | `--instance-id`, `--local-port`, `--remote-port`, `--parallel`, `--timeout` |
| `diagnose precheck` | 事前チェック | `--instance-id`, `--local-port`, `--profile`, `--region` |
| `diagnose preventive` | 予防的チェック | `--instance-id`, `--abort-on-critical`, `--timeout` |
| `diagnose item` | 特定項目診断 | `--item`, `--instance-id` |
| `diagnose list` | 診断項目一覧 | なし |
| `diagnose aws-config` | AWS 設定検証 | `--instance-id`, `--include-credentials`, `--include-iam`, `--minimum-score` |
| `diagnose aws-config-integrated` | 統合 AWS 設定検証 | `--instance-id`, `--clear-cache` |
| `diagnose interactive` | リアルタイム診断 UI | `--instance-id`, `--no-color`, `--refresh-interval` |

### 修復コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `precheck` | 接続前チェック | `--instance-id`, `--timeout`, `--format`, `--output` |
| `fix` | 自動修復実行 | `--instance-id`, `--auto-fix`, `--safe-only`, `--dry-run` |

### 設定コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `config validate` | 設定検証 | なし |
| `config show` | 設定表示 | なし |
| `config generate` | 設定ファイル生成 | `--output`, `--format` |
| `config env-help` | 環境変数ヘルプ | なし |
| `config test` | 設定テスト | なし |

### 診断設定コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `diagnose settings show` | 診断設定表示 | なし |
| `diagnose settings enable` | 診断チェック有効化 | `check_name` |
| `diagnose settings disable` | 診断チェック無効化 | `check_name` |
| `diagnose settings auto-fix` | 自動修復設定 | `--enable`, `--safe-only` |
| `diagnose settings parallel` | 並列実行設定 | `enable` |
| `diagnose settings timeout` | タイムアウト設定 | `seconds` |
| `diagnose settings format` | レポート形式設定 | `format` |
| `diagnose settings reset` | 設定リセット | なし |

### データベースコマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `database init` | データベース初期化 | なし |
| `database info` | データベース情報 | なし |
| `database sessions` | セッション一覧 | なし |
| `database stats` | パフォーマンス統計 | `[session_id]` |
| `database cleanup` | データクリーンアップ | `--days` |
| `database export` | データエクスポート | `--output`, `--format` |

### VS Code 統合コマンド

| コマンド | 説明 | 主要オプション |
|---------|------|---------------|
| `vscode status` | 統合状態確認 | なし |
| `vscode test` | 統合テスト | `[session_id]` |
| `vscode setup` | 統合セットアップ | なし |
| `vscode cleanup` | SSH 設定クリーンアップ | `[session_id]` |

### 共通オプション

| オプション | 説明 | 適用コマンド |
|-----------|------|-------------|
| `--verbose`, `-v` | 詳細ログ出力 | 全コマンド |
| `--config`, `-c` | 設定ファイルパス | 全コマンド |
| `--help`, `-h` | ヘルプ表示 | 全コマンド |
| `--version` | バージョン表示 | 全コマンド |

### 診断項目一覧

利用可能な診断項目（`diagnose item` コマンドで使用）：

- `instance_state` - EC2 インスタンスの存在と状態確認
- `ssm_agent` - SSM エージェントのインストールと登録確認
- `iam_permissions` - IAM ロールと権限の検証
- `vpc_endpoints` - SSM 接続用 VPC エンドポイント確認
- `security_groups` - セキュリティグループルール検証
- `network_connectivity` - AWS サービスへのネットワーク接続テスト
- `local_port_availability` - ローカルポートの可用性確認

### 出力形式

多くのコマンドで以下の出力形式をサポート：

- `text` - 人間が読みやすいテキスト形式（デフォルト）
- `json` - JSON 形式
- `yaml` - YAML 形式

### 終了コード

- `0` - 成功
- `1` - 一般的なエラー
- `2` - 設定エラー
- `3` - 接続エラー
- `4` - 認証エラー
- `5` - リソース不足エラー

## 設定

設定ファイルは以下の場所に配置されます：

- **Windows**: `%APPDATA%\ec2-connect\config.json`
- **Linux/macOS**: `~/.config/ec2-connect/config.json`

### 設定例

```json
{
  "aws": {
    "default_region": "us-east-1",
    "connection_timeout": 30,
    "request_timeout": 60
  },
  "session": {
    "max_sessions_per_instance": 3,
    "health_check_interval": 5,
    "reconnection": {
      "enabled": true,
      "max_attempts": 5,
      "base_delay_ms": 1000,
      "max_delay_ms": 16000
    }
  },
  "resources": {
    "max_memory_mb": 10,
    "max_cpu_percent": 0.5,
    "low_power_mode": true
  }
}
```

### 使用例

#### 基本的な接続フロー

```bash
# 1. 事前チェック実行
ec2-connect precheck --instance-id i-1234567890abcdef0 --local-port 8080

# 2. 問題があれば自動修復
ec2-connect fix --instance-id i-1234567890abcdef0 --auto-fix --safe-only

# 3. 接続実行
ec2-connect connect --instance-id i-1234567890abcdef0 --local-port 8080 --remote-port 80

# 4. セッション状態確認
ec2-connect status

# 5. リソース監視
ec2-connect resources
```

#### 包括的診断とトラブルシューティング

```bash
# 完全診断実行
ec2-connect diagnose full --instance-id i-1234567890abcdef0 \
  --local-port 8080 --remote-port 80 --parallel --timeout 60

# AWS 設定の詳細検証
ec2-connect diagnose aws-config --instance-id i-1234567890abcdef0 \
  --include-credentials --include-iam --include-vpc --include-security-groups

# リアルタイム診断 UI
ec2-connect diagnose interactive --instance-id i-1234567890abcdef0 --parallel

# 特定の問題を診断
ec2-connect diagnose item --item ssm_agent --instance-id i-1234567890abcdef0
```

#### 高度な設定とカスタマイズ

```bash
# カスタム設定ファイル生成
ec2-connect config generate --output ./my-config.json --format json

# 環境変数での設定オーバーライド
export EC2_CONNECT_AWS_REGION=us-west-2
export EC2_CONNECT_MAX_SESSIONS=5
export EC2_CONNECT_LOG_LEVEL=debug
ec2-connect connect --instance-id i-1234567890abcdef0

# 診断設定のカスタマイズ
ec2-connect diagnose settings auto-fix --enable --safe-only
ec2-connect diagnose settings parallel true
ec2-connect diagnose settings timeout 120
```

#### VS Code 統合

```bash
# VS Code 統合セットアップ
ec2-connect vscode setup

# 統合状態確認
ec2-connect vscode status

# 接続テスト
ec2-connect vscode test

# SSH 設定クリーンアップ
ec2-connect vscode cleanup
```

#### データ管理とエクスポート

```bash
# データベース初期化
ec2-connect database init

# セッション履歴確認
ec2-connect database sessions

# パフォーマンス統計
ec2-connect database stats

# データエクスポート
ec2-connect database export --output sessions-backup.json --format json

# 古いデータクリーンアップ
ec2-connect database cleanup --days 30
```

### コンポーネント

- **Session Monitor**: セッションの健全性を継続的に監視
- **Auto Reconnector**: 自動再接続機能
- **Session Manager**: 複数セッションの管理
- **Performance Monitor**: パフォーマンス監視と最適化
- **Health Checker**: システムとセッションの健全性チェック
- **Resource Monitor**: リソース使用量の監視と最適化
- **Terminal UI**: リッチターミナルインターフェース

### 技術スタック

- **AWS SDK**: aws-sdk-ssm, aws-sdk-ec2
- **非同期処理**: tokio
- **CLI**: clap
- **設定管理**: serde, toml, json
- **ログ**: tracing
- **ターミナル UI**: crossterm, ratatui
- **テスト**: proptest (プロパティベーステスト)
- **データベース**: rusqlite

## 開発

### テスト実行

```bash
# 単体テスト
cargo test

# プロパティベーステスト
cargo test --features proptest

# 統合テスト
cargo test --test '*'
```

### ベンチマーク

```bash
cargo bench
```

### ログレベル設定

```bash
RUST_LOG=debug ec2-connect connect --instance-id i-xxx
```

## ドキュメント

### 📚 完全ドキュメントセット

- **[API リファレンス](docs/API_REFERENCE.md)** - 完全な API 仕様とコマンドリファレンス
- **[チュートリアル & 使用例](docs/TUTORIALS.md)** - 段階的な学習ガイドと実践的な使用例
- **[トラブルシューティングガイド](docs/TROUBLESHOOTING.md)** - 問題解決の包括的なガイド
- **[パフォーマンス最適化](docs/PERFORMANCE_OPTIMIZATION.md)** - 性能を最大化するための最適化手法

### 🔧 設定・技術仕様

- **[設定ガイド](docs/CONFIGURATION.md)** - 詳細な設定方法と環境変数
- **[データモデル仕様](docs/DATA_MODELS.md)** - 内部データ構造と API 仕様

### 🚀 クイックリンク

- **初心者**: [チュートリアル](docs/TUTORIALS.md#クイックスタート) → [基本的な使用方法](docs/TUTORIALS.md#基本チュートリアル)
- **問題解決**: [トラブルシューティング](docs/TROUBLESHOOTING.md#クイック診断) → [よくある問題](docs/TROUBLESHOOTING.md#接続問題)
- **最適化**: [パフォーマンス最適化](docs/PERFORMANCE_OPTIMIZATION.md#パフォーマンス目標) → [環境別設定](docs/PERFORMANCE_OPTIMIZATION.md#環境別最適化)
- **API 詳細**: [API リファレンス](docs/API_REFERENCE.md#cli-コマンド) → [設定 API](docs/API_REFERENCE.md#設定-api)

## トラブルシューティング

### 自動診断・修復機能

EC2 Connect v3.0 では包括的な診断・修復機能を提供しています：

#### クイック診断

```bash
# 接続前の事前チェック
ec2-connect precheck --instance-id i-1234567890abcdef0

# 自動修復実行
ec2-connect fix --instance-id i-1234567890abcdef0 --auto-fix --safe-only

# システムヘルスチェック
ec2-connect health --comprehensive
```

#### 詳細診断

```bash
# 包括的診断（推奨）
ec2-connect diagnose full --instance-id i-1234567890abcdef0 --parallel

# AWS 設定検証
ec2-connect diagnose aws-config --instance-id i-1234567890abcdef0

# リアルタイム診断 UI
ec2-connect diagnose interactive --instance-id i-1234567890abcdef0
```

### よくある問題と解決方法

詳細な解決方法は [トラブルシューティングガイド](docs/TROUBLESHOOTING.md) を参照してください。

#### 接続できない

**自動診断・修復:**
```bash
# 1. 事前チェックで問題を特定
ec2-connect precheck --instance-id i-1234567890abcdef0

# 2. 自動修復を試行
ec2-connect fix --instance-id i-1234567890abcdef0 --auto-fix

# 3. 詳細診断（必要に応じて）
ec2-connect diagnose full --instance-id i-1234567890abcdef0
```

**手動確認項目:**
1. AWS 認証情報を確認: `aws sts get-caller-identity`
2. Session Manager Plugin がインストールされているか確認
3. インスタンスが SSM 管理されているか確認: `ec2-connect diagnose item --item ssm_agent --instance-id i-xxx`
4. **詳細**: [接続問題の解決](docs/TROUBLESHOOTING.md#接続問題)

#### メモリ使用量が高い

**自動最適化:**
```bash
# リソース状況確認
ec2-connect resources

# 自動最適化実行
ec2-connect metrics
```

**手動対応:**
1. 不要なセッションを終了: `ec2-connect list` → `ec2-connect terminate SESSION_ID`
2. 低電力モードを有効化（設定ファイル）
3. 設定ファイルでリソース制限を調整
4. **詳細**: [パフォーマンス問題の解決](docs/TROUBLESHOOTING.md#パフォーマンス問題)

#### 再接続が失敗する

**診断・修復:**
```bash
# ネットワーク診断
ec2-connect diagnose item --item network_connectivity --instance-id i-xxx

# 予防的チェック
ec2-connect diagnose preventive --instance-id i-xxx --abort-on-critical
```

**手動確認:**
1. ネットワーク接続を確認
2. 再接続ポリシーの設定を確認: `ec2-connect config show`
3. ログを確認して詳細なエラーを特定
4. **詳細**: [接続問題の診断](docs/TROUBLESHOOTING.md#問題-2-接続が頻繁に切断される)

#### VS Code 統合の問題

```bash
# VS Code 統合状態確認
ec2-connect vscode status

# 統合セットアップ
ec2-connect vscode setup

# 統合テスト
ec2-connect vscode test

# SSH 設定クリーンアップ
ec2-connect vscode cleanup
```

### 診断レポートの出力

問題報告時に以下のコマンドでレポートを生成してください：

```bash
# 包括的診断レポート
ec2-connect diagnose full --instance-id i-xxx --format json --output diagnostic-report.json

# システム状態レポート
ec2-connect health --comprehensive > health-report.txt
ec2-connect resources > resource-report.txt
ec2-connect config show > config-report.txt

# データベース統計
ec2-connect database stats > database-stats.txt
```

## ライセンス

MIT License

## 貢献

プルリクエストを歓迎します。大きな変更の場合は、まず issue を開いて変更内容を議論してください。

## 変更履歴

### v3.0.0 (2024-01-01)

- Rust への完全移行
- 自動セッション管理機能の実装
- パフォーマンス最適化（メモリ 10MB 以下、CPU 0.5%以下）
- リッチターミナル UI の実装
- プロパティベーステストの導入
