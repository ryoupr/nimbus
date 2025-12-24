# 技術スタック

## 言語・ビルドシステム

- **言語**: Rust 1.70+
- **エディション**: 2021
- **ビルドツール**: Cargo

## 主要依存関係

### AWS SDK

- `aws-config` (1.1) - AWS 設定管理
- `aws-sdk-ssm` (1.12) - SSM サービス統合
- `aws-sdk-ec2` (1.15) - EC2 サービス統合
- `aws-sdk-iam` (1.12) - IAM 権限管理
- `aws-sdk-sts` (1.12) - 認証情報検証

### 非同期処理

- `tokio` (1.35) - 非同期ランタイム（full features）

### CLI フレームワーク

- `clap` (4.4) - CLI パーサー（derive, color features）

### シリアライゼーション

- `serde` (1.0) - シリアライゼーションフレームワーク
- `serde_json` (1.0) - JSON サポート
- `serde_yaml` (0.9) - YAML サポート
- `toml` (0.8) - TOML サポート

### ターミナル UI

- `crossterm` (0.27) - クロスプラットフォームターミナル制御
- `ratatui` (0.25) - TUI フレームワーク

### データベース

- `rusqlite` (0.30) - SQLite データベース（bundled, chrono, backup features）

### エラーハンドリング

- `anyhow` (1.0) - エラー処理
- `thiserror` (1.0) - カスタムエラー型

### ユーティリティ

- `tracing` (0.1) - 構造化ログ
- `tracing-subscriber` (0.3) - ログサブスクライバー
- `uuid` (1.6) - UUID 生成
- `chrono` (0.4) - 日時処理
- `sysinfo` (0.30) - システム情報取得

### テスト・ベンチマーク

- `proptest` (1.4) - プロパティベーステスト
- `criterion` (0.5) - ベンチマーク
- `tokio-test` (0.4) - 非同期テスト
- `tempfile` (3.8) - 一時ファイル

## ビルドプロファイル

### Release ビルド

```toml
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### Development ビルド

```toml
opt-level = 0
debug = 0  # デバッグ情報を減らしてリンク時間を短縮
strip = "debuginfo"

# 依存関係は最適化（変更頻度が低いため）
[profile.dev.package."*"]
opt-level = 3
```

## 共通コマンド

### ビルド・実行

```bash
# 開発ビルド（cargo check を優先）
cargo check

# リリースビルド
cargo build --release

# 実行
cargo run -- [ARGS]

# インストール
cargo install --path .
```

### テスト

```bash
# 単体テスト
cargo test

# 特定のテスト
cargo test <test_name>

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
RUST_LOG=debug cargo run -- [ARGS]
```

## Feature フラグ

- `default`: 基本機能のみ
- `advanced`: 全機能有効化
  - `performance-monitoring`: パフォーマンス監視
  - `persistence`: データベース永続化
  - `multi-session`: マルチセッション管理
  - `auto-reconnect`: 自動再接続

## プラットフォーム固有の依存関係

### Unix/Linux/macOS

- `nix` (0.27) - シグナル・プロセス管理

### Windows

- `winapi` (0.3) - Windows API アクセス

## リンカ最適化

`.cargo/config.toml` でリンカを高速化:

- Linux: `mold`
- macOS/Windows: `lld`
