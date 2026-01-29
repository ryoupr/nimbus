# Nimbus

高性能 EC2 SSM 接続管理ツール - 雲に乗ってEC2へ ☁️

## 概要

Nimbus は、Rust で書かれた高性能な EC2 インスタンス接続管理ツールです。

- **自動セッション維持**: セッションを自動的に監視し、切断を予防
- **高速再接続**: 5 秒以内の切断検出と自動再接続
- **セッション管理最適化**: 複数セッションの効率的な管理
- **パフォーマンス監視**: 接続速度とレイテンシの継続的な監視
- **軽量動作**: メモリ 10MB 以下、CPU 0.5%以下

## インストール

### 前提条件

- AWS CLI
- AWS Session Manager Plugin

macOS で `session-manager-plugin` が見つからない場合:

```bash
brew install --cask session-manager-plugin
```

### Mac (Homebrew)

```bash
brew tap ryoupr/tap
brew install ryoupr/tap/nimbus --formula
```

### Mac / Linux

```bash
curl -sSL https://raw.githubusercontent.com/ryoupr/nimbus/main/install.sh | bash
```

### Windows (PowerShell)

```powershell
iwr -useb https://raw.githubusercontent.com/ryoupr/nimbus/main/install.ps1 | iex
```

### ソースからビルド

```bash
# Rust 1.70以上が必要
cargo install --path .
```

## 使用方法

### 基本コマンド

```bash
# EC2 インスタンスに接続
nimbus connect --instance-id i-1234567890abcdef0 --local-port 8080 --remote-port 80

# リモートホスト経由でポートフォワード（踏み台経由で内部ALB等に接続）
nimbus connect --instance-id i-1234567890abcdef0 --local-port 10443 --remote-port 443 \
  --remote-host internal-alb-xxx.ap-northeast-1.elb.amazonaws.com

# 接続先一覧（targetsファイル）から接続
nimbus connect --target dev

# セッション一覧表示
nimbus list

# セッション状態確認
nimbus status

# セッション終了
nimbus terminate SESSION_ID
```

### 診断・トラブルシューティング

```bash
# 事前チェック
nimbus precheck --instance-id i-1234567890abcdef0 --local-port 8080

# 自動修復
nimbus fix --instance-id i-1234567890abcdef0 --auto-fix --safe-only

# 包括的診断
nimbus diagnose full --instance-id i-1234567890abcdef0 --parallel
```

### ターミナル UI

```bash
# ターミナル UI 起動
nimbus tui

# マルチセッション管理 UI
nimbus multi-session
```

## 設定

設定ファイルの場所:
- **Windows**: `%APPDATA%\nimbus\config.json`
- **Linux/macOS**: `~/.config/nimbus/config.json`

### 設定例

```json
{
  "aws": {
    "default_region": "ap-northeast-1",
    "connection_timeout": 30
  },
  "session": {
    "max_sessions_per_instance": 3,
    "health_check_interval": 5,
    "reconnection": {
      "enabled": true,
      "max_attempts": 5
    }
  }
}
```

### 環境変数

```bash
export NIMBUS_AWS_REGION=us-west-2
export NIMBUS_MAX_SESSIONS=5
export NIMBUS_LOG_LEVEL=debug
```

## ドキュメント

- **[API リファレンス](docs/API_REFERENCE.md)** - 完全なコマンドリファレンス
- **[チュートリアル](docs/TUTORIALS.md)** - 段階的な学習ガイド
- **[トラブルシューティング](docs/TROUBLESHOOTING.md)** - 問題解決ガイド
- **[設定ガイド](docs/CONFIGURATION.md)** - 詳細な設定方法
- **[パフォーマンス最適化](docs/PERFORMANCE_OPTIMIZATION.md)** - 性能チューニング

## 開発

```bash
# テスト
cargo test

# ベンチマーク
cargo bench

# デバッグログ
RUST_LOG=debug nimbus connect --instance-id i-xxx
```

## ライセンス

MIT License
