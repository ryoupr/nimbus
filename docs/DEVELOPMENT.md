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
curl -sL https://github.com/your-org/ec2-connect/releases/download/v3.1.0/ec2-connect-darwin-x86_64.tar.gz.sha256
curl -sL https://github.com/your-org/ec2-connect/releases/download/v3.1.0/ec2-connect-darwin-arm64.tar.gz.sha256
```

`Formula/ec2-connect.rb` の以下を更新:
- `version "3.1.0"`
- 各アーキテクチャの `sha256`

### 4. Homebrew Tap リポジトリへ反映

Tap用リポジトリ（`your-org/homebrew-tap`）に Formula をコピー:

```bash
cp Formula/ec2-connect.rb ../homebrew-tap/Formula/
cd ../homebrew-tap
git add -A && git commit -m "ec2-connect 3.1.0" && git push
```

## 初回セットアップ

### Homebrew Tap リポジトリ作成

1. GitHub で `homebrew-tap` リポジトリを作成
2. `Formula/` ディレクトリを作成
3. `ec2-connect.rb` を配置

```
homebrew-tap/
└── Formula/
    └── ec2-connect.rb
```

ユーザーは以下でインストール可能に:
```bash
brew tap your-org/tap
brew install ec2-connect
```

### GitHub リポジトリ設定

`your-org` を実際の GitHub organization/username に置き換え:

- `.github/workflows/release.yml`
- `install.sh`
- `install.ps1`
- `Formula/ec2-connect.rb`
- `README.md`

## ローカル開発

```bash
# ビルド
cargo build

# リリースビルド
cargo build --release

# テスト
cargo test

# 実行
cargo run -- --help
```

## クロスコンパイル（手動）

```bash
# Mac ARM → Mac Intel
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Linux向け（Mac上）
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu
```
