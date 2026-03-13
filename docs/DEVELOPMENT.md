# 開発者向けガイド

## リリース手順

### 1. バージョン更新

`Cargo.toml` のバージョンを更新:
```toml
version = "3.1.0"
```

### 2. タグ作成＆プッシュ

```bash
git add -A
git commit -m "Release v3.1.0"
git tag v3.1.0
git push origin main --tags
```

GitHub Actions が自動で以下を実行:
- 4プラットフォーム向けビルド（Mac Intel/ARM, Linux, Windows）
- GitHub Releases にバイナリをアップロード

### 3. Homebrew Formula 更新

リリース完了後、sha256を取得して Formula を更新:

```bash
# sha256を取得
curl -sL https://github.com/ryoupr/nimbus/releases/download/v3.1.0/nimbus-darwin-x86_64.tar.gz.sha256
curl -sL https://github.com/ryoupr/nimbus/releases/download/v3.1.0/nimbus-darwin-arm64.tar.gz.sha256
```

`Formula/nimbus.rb` の以下を更新:
- `version "3.1.0"`
- 各アーキテクチャの `sha256`

### 4. Homebrew Tap リポジトリへ反映

Tap用リポジトリ（`ryoupr/homebrew-tap`）に Formula をコピー:

```bash
cp Formula/nimbus.rb ../homebrew-tap/Formula/
cd ../homebrew-tap
git add -A && git commit -m "nimbus 3.1.0" && git push
```

## 初回セットアップ

### Homebrew Tap リポジトリ作成

1. GitHub で `homebrew-tap` リポジトリを作成
2. `Formula/` ディレクトリを作成
3. `nimbus.rb` を配置

```
homebrew-tap/
└── Formula/
    └── nimbus.rb
```

ユーザーは以下でインストール可能に:
```bash
brew tap ryoupr/tap
brew install nimbus
```

### GitHub リポジトリ設定

`ryoupr` を実際の GitHub organization/username に置き換え:

- `.github/workflows/release.yml`
- `install.sh`
- `install.ps1`
- `Formula/nimbus.rb`
- `README.md`

## ローカル開発

```bash
# ビルド
cargo build

# リリースビルド
cargo build --release

# 全 feature 有効でビルド
cargo build --features advanced

# テスト
cargo test

# 実行
cargo run -- --help
```

## プロジェクト構造

```
src/
├── main.rs                  # CLI 定義 (clap) とエントリポイント
├── commands/                # コマンドハンドラ（サブコマンドごとに分割）
│   ├── mod.rs
│   ├── connect.rs           # connect コマンド
│   ├── config.rs            # config サブコマンド
│   ├── database.rs          # database サブコマンド (persistence feature)
│   ├── diagnose.rs          # diagnose サブコマンド
│   ├── diagnostic_settings.rs # diagnose settings サブコマンド
│   ├── fix.rs               # fix コマンド
│   ├── monitoring.rs        # metrics / resources / health コマンド
│   ├── multi_session.rs     # multi-session コマンド (multi-session feature)
│   ├── tui.rs               # tui コマンド
│   └── vscode.rs            # vscode サブコマンド
├── aws.rs                   # AWS SDK ラッパー
├── config.rs                # 設定ファイル読み込み・検証
├── targets.rs               # ターゲットファイル (JSON/TOML)
├── diagnostic.rs            # 診断エンジン
├── error.rs                 # エラー型定義
├── error_recovery.rs        # エラー回復・リトライ
├── user_messages.rs         # ユーザー向けメッセージ
└── ...                      # その他モジュール
```

### Feature Flags

| Flag | 説明 |
|---|---|
| `performance-monitoring` | パフォーマンス監視 (`monitor`, `performance` モジュール) |
| `persistence` | データベース永続化 (`persistence` モジュール) |
| `multi-session` | マルチセッション管理 (`multi_session`, `multi_session_ui` モジュール) |
| `auto-reconnect` | 自動再接続 (`reconnect` モジュール) |
| `advanced` | 上記すべてを有効化 |

## クロスコンパイル（手動）

```bash
# Mac ARM → Mac Intel
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Linux向け（Mac上）
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu
```
