---
inclusion: fileMatch
fileMatchPattern: ['**/*.rs', '**/Cargo.toml', '**/Cargo.lock']
---

# 技術スタック

Rust 1.70+ / Edition 2021 / Cargo

## 主要依存関係

| カテゴリ | クレート | 用途 |
|---------|---------|------|
| AWS | `aws-config`, `aws-sdk-ssm`, `aws-sdk-ec2`, `aws-sdk-iam`, `aws-sdk-sts` | AWS サービス統合 |
| 非同期 | `tokio` (full) | 非同期ランタイム |
| CLI | `clap` (derive, color) | コマンドライン解析 |
| シリアライズ | `serde`, `serde_json`, `serde_yaml`, `toml` | データ変換 |
| TUI | `crossterm`, `ratatui` | ターミナル UI |
| DB | `rusqlite` (bundled, chrono, backup) | SQLite 永続化 |
| エラー | `anyhow`, `thiserror` | エラーハンドリング |
| ログ | `tracing`, `tracing-subscriber` | 構造化ログ |
| テスト | `proptest`, `criterion`, `tokio-test`, `tempfile` | テスト・ベンチマーク |

## 開発ワークフロー

**Check First, Build Last** - フルビルドを最小化

```bash
cargo check              # 開発中は常にこれを使用（cargo build より 10x 高速）
cargo test <test_name>   # 特定テストのみ実行
cargo build --release    # 最終検証時のみ
```

## Feature フラグ

- `default`: 基本機能
- `advanced`: 全機能（`performance-monitoring`, `persistence`, `multi-session`, `auto-reconnect`）

## ビルドプロファイル

| プロファイル | 設定 |
|-------------|------|
| dev | `opt-level=0`, `debug=0`, 依存関係は `opt-level=3` |
| release | `opt-level=3`, `lto=true`, `codegen-units=1`, `panic=abort`, `strip=true` |

## リンカ最適化

`.cargo/config.toml` で設定済み:
- Linux: `mold`
- macOS/Windows: `lld`

## プラットフォーム固有

- Unix/Linux/macOS: `nix` (シグナル・プロセス管理)
- Windows: `winapi`
