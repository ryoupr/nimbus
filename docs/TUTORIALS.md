# Nimbus チュートリアル & 使用例

## 概要

Nimbus v3.0 の実践的な使用方法を学ぶためのチュートリアルと使用例集です。初心者から上級者まで、段階的に機能を習得できるように構成されています。

## 目次

- [クイックスタート](#クイックスタート)
- [基本チュートリアル](#基本チュートリアル)
- [高度な使用例](#高度な使用例)
- [実践的なワークフロー](#実践的なワークフロー)
- [トラブルシューティング実例](#トラブルシューティング実例)
- [ベストプラクティス](#ベストプラクティス)

## クイックスタート

### 1. 初回セットアップ (5分)

```bash
# 1. プロジェクトディレクトリに移動
# プロジェクトルートで実行

# 2. 設定ファイル生成
cargo run -- config generate --output ~/.config/nimbus/config.json

# 3. 設定確認
cargo run -- config validate

# 4. ヘルスチェック
cargo run -- health
```

### 2. 最初の接続 (2分)

```bash
# EC2 インスタンスに接続
cargo run -- connect --instance-id i-1234567890abcdef0

# 接続先一覧（targetsファイル）から接続（推奨: サーバーごとの設定を名前で管理）
# 例: ~/.config/nimbus/targets.json を作成してから
cargo run -- connect --target dev

# 接続状態確認
cargo run -- list

# ターミナル UI で監視
cargo run -- tui
```

### 3. 接続終了 (1分)

```bash
# セッション一覧表示
cargo run -- list

# セッション終了
cargo run -- terminate <session-id>
```

## 基本チュートリアル

### チュートリアル 1: 基本的な接続管理

#### 目標

EC2 インスタンスへの基本的な接続と管理方法を学ぶ

#### 手順

**ステップ 1: 接続前の準備**

```bash
# AWS 認証情報確認
aws sts get-caller-identity

# EC2 インスタンス確認
aws ec2 describe-instances --instance-ids i-1234567890abcdef0

# 予防的チェック実行
cargo run -- diagnose preventive --instance-id i-1234567890abcdef0
```

**ステップ 2: 接続実行**

```bash
# 基本接続 (デフォルト: localhost:8080 -> instance:80)
cargo run -- connect --instance-id i-1234567890abcdef0

# カスタムポート接続
cargo run -- connect \
  --instance-id i-1234567890abcdef0 \
  --local-port 8443 \
  --remote-port 443

# 特定のプロファイルとリージョンで接続
cargo run -- connect \
  --instance-id i-1234567890abcdef0 \
  --profile production \
  --region us-west-2

# targets ファイルから接続
cargo run -- connect --target dev

# targets ファイルのパスを明示して接続
cargo run -- connect --targets-file ~/.config/nimbus/targets.json --target dev
```

**ステップ 3: 接続状態の監視**

```bash
# セッション一覧表示
cargo run -- list

# 特定セッションの詳細状態
cargo run -- status session-abc123

# リアルタイム監視 (ターミナル UI)
cargo run -- tui
```

**ステップ 4: 接続の終了**

```bash
# 特定セッション終了
cargo run -- terminate session-abc123

# 全セッション確認
cargo run -- list
```

#### 期待される結果

- EC2 インスタンスへの安全な接続が確立される
- ローカルポートでサービスにアクセス可能になる
- セッション状態をリアルタイムで監視できる

### チュートリアル 2: 複数セッション管理

#### 目標

複数の EC2 インスタンスに同時接続し、効率的に管理する

#### 手順

**ステップ 1: 複数セッション作成**

```bash
# Web サーバー接続 (ポート 80)
cargo run -- connect \
  --instance-id i-web-server-001 \
  --local-port 8080 \
  --remote-port 80 \
  --priority high

# データベース接続 (ポート 3306)
cargo run -- connect \
  --instance-id i-database-001 \
  --local-port 3306 \
  --remote-port 3306 \
  --priority critical

# API サーバー接続 (ポート 8000)
cargo run -- connect \
  --instance-id i-api-server-001 \
  --local-port 8000 \
  --remote-port 8000 \
  --priority normal
```

**ステップ 2: マルチセッション UI で管理**

```bash
# マルチセッション管理 UI 起動
cargo run -- multi-session
```

**UI 操作:**

- `1` キー: セッション一覧タブ
- `2` キー: リソース監視タブ
- `3` キー: 警告・通知タブ
- `4` キー: 詳細情報タブ
- `↑/↓` キー: ナビゲーション
- `r` キー: 更新
- `q` キー: 終了

**ステップ 3: リソース監視**

```bash
# 現在のリソース使用状況
cargo run -- resources

# パフォーマンスメトリクス
cargo run -- metrics

# システムヘルスチェック
cargo run -- health --comprehensive
```

#### 期待される結果

- 複数セッションが同時に安定動作する
- リソース使用量が制限内に収まる
- 各セッションの優先度に応じた管理が行われる

### チュートリアル 3: 自動再接続とエラー回復

#### 目標

ネットワーク障害時の自動再接続機能を理解し、設定する

#### 手順

**ステップ 1: 再接続ポリシー設定**

```bash
# 設定ファイル編集
nano ~/.config/nimbus/config.json
```

```json
{
  "session": {
    "reconnection": {
      "enabled": true,
      "max_attempts": 10,
      "base_delay_ms": 1000,
      "max_delay_ms": 30000,
      "aggressive_mode": true,
      "aggressive_attempts": 5,
      "aggressive_interval_ms": 500
    }
  }
}
```

**ステップ 2: 環境変数での一時的設定**

```bash
# アグレッシブ再接続モード
export NIMBUS_AGGRESSIVE_RECONNECTION=true
export NIMBUS_AGGRESSIVE_ATTEMPTS=10
export NIMBUS_AGGRESSIVE_INTERVAL_MS=250

# 設定確認
cargo run -- config test
```

**ステップ 3: 接続とネットワーク障害シミュレーション**

```bash
# 接続開始
cargo run -- connect --instance-id i-1234567890abcdef0

# 別ターミナルで監視
cargo run -- tui

# ネットワーク障害シミュレーション (例)
# sudo iptables -A OUTPUT -d <aws-ssm-endpoint> -j DROP
# (実際の本番環境では実行しないでください)
```

**ステップ 4: 再接続動作の確認**

```bash
# ログ確認
tail -f logs/nimbus.$(date +%Y-%m-%d)

# セッション状態確認
cargo run -- status
```

#### 期待される結果

- ネットワーク障害時に自動再接続が実行される
- 指数バックオフまたはアグレッシブモードで再試行される
- 接続復旧後に正常な状態に戻る

### チュートリアル 4: VS Code 統合

#### 目標

VS Code との統合機能を設定し、シームレスな開発環境を構築する

#### 手順

**ステップ 1: VS Code 統合の準備**

```bash
# VS Code 統合状態確認
cargo run -- vscode status

# VS Code 統合セットアップ
cargo run -- vscode setup
```

**ステップ 2: 設定ファイル調整**

```bash
# 設定ファイル編集
nano ~/.config/nimbus/config.json
```

```json
{
  "vscode": {
    "auto_launch_enabled": true,
    "auto_update_ssh_config": true,
    "ssh_config_path": null,
    "vscode_path": "/usr/local/bin/code",
    "notifications_enabled": true
  }
}
```

**ステップ 3: 統合接続の実行**

```bash
# VS Code 統合付き接続
cargo run -- connect \
  --instance-id i-1234567890abcdef0 \
  --local-port 22 \
  --remote-port 22 \
  --priority high
```

**ステップ 4: SSH 設定確認**

```bash
# SSH 設定ファイル確認
cat ~/.ssh/config

# VS Code での接続テスト
code --remote ssh-remote+ec2-i-1234567890abcdef0 .
```

**ステップ 5: 統合テスト**

```bash
# 統合機能テスト
cargo run -- vscode test session-abc123

# クリーンアップ
cargo run -- vscode cleanup
```

#### 期待される結果

- SSH 設定が自動更新される
- VS Code が自動起動する
- リモート開発環境がシームレスに利用できる

## 高度な使用例

### 使用例 1: 本番環境での安全な運用

#### シナリオ

本番環境の複数サーバーに対して、安全性を重視した接続管理を行う

#### 実装

**1. 厳格な設定**

```json
{
  "session": {
    "max_sessions_per_instance": 1,
    "health_check_interval": 3,
    "reconnection": {
      "enabled": true,
      "max_attempts": 3,
      "aggressive_mode": false
    }
  },
  "resources": {
    "max_memory_mb": 5.0,
    "max_cpu_percent": 0.2,
    "low_power_mode": true
  },
  "logging": {
    "level": "info",
    "file_logging": true,
    "json_format": true
  }
}
```

**2. 接続前の包括的チェック**

```bash
#!/bin/bash
# production-connect.sh

INSTANCE_ID=$1
PROFILE="production"
REGION="us-east-1"

echo "🔍 Production connection to $INSTANCE_ID"

# 1. AWS 設定検証
echo "Validating AWS configuration..."
cargo run -- diagnose aws-config \
  --instance-id $INSTANCE_ID \
  --profile $PROFILE \
  --region $REGION \
  --minimum-score 90.0

if [ $? -ne 0 ]; then
  echo "❌ AWS configuration validation failed"
  exit 1
fi

# 2. 予防的チェック
echo "Running preventive checks..."
cargo run -- diagnose preventive \
  --instance-id $INSTANCE_ID \
  --profile $PROFILE \
  --region $REGION \
  --abort-on-critical true

if [ $? -ne 0 ]; then
  echo "❌ Preventive checks failed"
  exit 1
fi

# 3. 接続実行
echo "Establishing connection..."
cargo run -- connect \
  --instance-id $INSTANCE_ID \
  --profile $PROFILE \
  --region $REGION \
  --priority critical

echo "✅ Production connection established"
```

**3. 監視とアラート**

```bash
#!/bin/bash
# production-monitor.sh

while true; do
  # リソース使用量チェック
  MEMORY_USAGE=$(cargo run -- metrics | grep "Memory usage" | awk '{print $3}' | sed 's/MB//')
  
  if (( $(echo "$MEMORY_USAGE > 4.0" | bc -l) )); then
    echo "⚠️  High memory usage: ${MEMORY_USAGE}MB"
    # アラート送信 (例: Slack, メール等)
  fi
  
  # セッション健全性チェック
  cargo run -- health --comprehensive > /dev/null
  if [ $? -ne 0 ]; then
    echo "❌ Health check failed"
    # アラート送信
  fi
  
  sleep 30
done
```

### 使用例 2: 開発チーム向けの自動化

#### シナリオ

開発チームが複数の開発環境に効率的にアクセスできる自動化システム

#### 実装

**1. 環境定義ファイル**

```json
{
  "environments": {
    "dev": {
      "web": "i-dev-web-001",
      "api": "i-dev-api-001",
      "db": "i-dev-db-001"
    },
    "staging": {
      "web": "i-staging-web-001",
      "api": "i-staging-api-001",
      "db": "i-staging-db-001"
    }
  },
  "ports": {
    "web": { "local": 8080, "remote": 80 },
    "api": { "local": 8000, "remote": 8000 },
    "db": { "local": 3306, "remote": 3306 }
  }
}
```

**2. 環境接続スクリプト**

```bash
#!/bin/bash
# connect-env.sh

ENV=$1
SERVICE=$2

if [ -z "$ENV" ] || [ -z "$SERVICE" ]; then
  echo "Usage: $0 <env> <service>"
  echo "Environments: dev, staging"
  echo "Services: web, api, db"
  exit 1
fi

# 設定読み込み
INSTANCE_ID=$(jq -r ".environments.$ENV.$SERVICE" environments.json)
LOCAL_PORT=$(jq -r ".ports.$SERVICE.local" environments.json)
REMOTE_PORT=$(jq -r ".ports.$SERVICE.remote" environments.json)

if [ "$INSTANCE_ID" = "null" ]; then
  echo "❌ Invalid environment or service"
  exit 1
fi

echo "🚀 Connecting to $ENV $SERVICE ($INSTANCE_ID)"

# 既存セッション確認
EXISTING=$(cargo run -- list | grep $INSTANCE_ID | head -1)
if [ ! -z "$EXISTING" ]; then
  echo "ℹ️  Existing session found: $EXISTING"
  read -p "Use existing session? (y/n): " -n 1 -r
  echo
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "✅ Using existing session"
    exit 0
  fi
fi

# 新規接続
cargo run -- connect \
  --instance-id $INSTANCE_ID \
  --local-port $LOCAL_PORT \
  --remote-port $REMOTE_PORT \
  --priority normal

echo "✅ Connected to $ENV $SERVICE at localhost:$LOCAL_PORT"
```

**3. チーム用ダッシュボード**

```bash
#!/bin/bash
# team-dashboard.sh

echo "🏢 Development Team Dashboard"
echo "=============================="

# アクティブセッション
echo "📋 Active Sessions:"
cargo run -- list

echo ""

# リソース使用状況
echo "💾 Resource Usage:"
cargo run -- resources

echo ""

# 環境別接続状況
echo "🌍 Environment Status:"
for env in dev staging; do
  echo "  $env:"
  for service in web api db; do
    instance_id=$(jq -r ".environments.$env.$service" environments.json)
    status=$(cargo run -- health $instance_id 2>/dev/null | grep "Status:" | awk '{print $2}')
    echo "    $service ($instance_id): ${status:-Unknown}"
  done
done
```

### 使用例 3: CI/CD パイプライン統合

#### シナリオ

CI/CD パイプラインでの自動テストとデプロイメント検証

#### 実装

**1. GitHub Actions ワークフロー**

```yaml
# .github/workflows/deploy-test.yml
name: Deploy and Test

on:
  push:
    branches: [main]

jobs:
  deploy-test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    
    - name: Build Nimbus
      run: |
        # プロジェクトルートで実行
        cargo build --release
    
    - name: Configure AWS credentials
      uses: aws-actions/configure-aws-credentials@v2
      with:
        aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
        aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
        aws-region: us-east-1
    
    - name: Test connection to staging
      run: |
        # プロジェクトルートで実行
        
        # 予防的チェック
        cargo run -- diagnose preventive \
          --instance-id ${{ secrets.STAGING_INSTANCE_ID }} \
          --timeout 30 \
          --format json > preventive-check.json
        
        # 結果確認
        CONNECTION_LIKELIHOOD=$(jq -r '.connection_likelihood.percentage' preventive-check.json)
        if [ "$CONNECTION_LIKELIHOOD" -lt 80 ]; then
          echo "❌ Connection likelihood too low: $CONNECTION_LIKELIHOOD%"
          exit 1
        fi
        
        # 接続テスト
        timeout 60 cargo run -- connect \
          --instance-id ${{ secrets.STAGING_INSTANCE_ID }} \
          --local-port 8080 \
          --remote-port 80 &
        
        CONNECT_PID=$!
        sleep 10
        
        # 接続確認
        if curl -f http://localhost:8080/health; then
          echo "✅ Connection test passed"
        else
          echo "❌ Connection test failed"
          exit 1
        fi
        
        # クリーンアップ
        kill $CONNECT_PID
    
    - name: Upload test results
      uses: actions/upload-artifact@v3
      with:
        name: connection-test-results
        path: preventive-check.json
```

**2. デプロイメント検証スクリプト**

```bash
#!/bin/bash
# deployment-verification.sh

INSTANCE_ID=$1
HEALTH_ENDPOINT=$2

echo "🔍 Deployment verification for $INSTANCE_ID"

# 1. インスタンス状態確認
echo "Checking instance status..."
aws ec2 describe-instances \
  --instance-ids $INSTANCE_ID \
  --query 'Reservations[0].Instances[0].State.Name' \
  --output text

# 2. SSM 接続テスト
echo "Testing SSM connectivity..."
cargo run -- diagnose full \
  --instance-id $INSTANCE_ID \
  --timeout 30 \
  --format json > deployment-check.json

# 3. 結果解析
OVERALL_STATUS=$(jq -r '.overall_status' deployment-check.json)
if [ "$OVERALL_STATUS" != "Ready" ]; then
  echo "❌ Deployment verification failed"
  jq '.critical_issues' deployment-check.json
  exit 1
fi

# 4. アプリケーション接続テスト
echo "Testing application connectivity..."
cargo run -- connect \
  --instance-id $INSTANCE_ID \
  --local-port 8080 \
  --remote-port 80 &

CONNECT_PID=$!
sleep 15

# 5. ヘルスチェック
if curl -f $HEALTH_ENDPOINT; then
  echo "✅ Deployment verification passed"
  RESULT=0
else
  echo "❌ Application health check failed"
  RESULT=1
fi

# クリーンアップ
kill $CONNECT_PID
exit $RESULT
```

## 実践的なワークフロー

### ワークフロー 1: 日常的な開発作業

```bash
# 1. 朝の作業開始
./scripts/morning-setup.sh

# 2. 開発環境接続
cargo run -- connect --instance-id i-dev-web-001 --priority high

# 3. VS Code 起動 (自動)
# VS Code が自動的に起動し、リモート開発環境に接続

# 4. 作業中の監視
cargo run -- tui &

# 5. 夕方の作業終了
./scripts/evening-cleanup.sh
```

### ワークフロー 2: 緊急対応

```bash
# 1. 緊急アラート受信
echo "🚨 Production issue detected"

# 2. 高速診断
cargo run -- diagnose full \
  --instance-id i-prod-web-001 \
  --parallel true \
  --timeout 15

# 3. 緊急接続
cargo run -- connect \
  --instance-id i-prod-web-001 \
  --priority critical \
  --local-port 22 \
  --remote-port 22

# 4. 問題調査と修復
ssh ec2-i-prod-web-001

# 5. 修復後の検証
cargo run -- health --comprehensive
```

### ワークフロー 3: 定期メンテナンス

```bash
# 1. メンテナンス前チェック
./scripts/pre-maintenance-check.sh

# 2. 全セッション状態確認
cargo run -- database sessions

# 3. パフォーマンス統計取得
cargo run -- database stats > maintenance-report.txt

# 4. 古いデータクリーンアップ
cargo run -- database cleanup --days 7

# 5. 設定最適化
cargo run -- config validate
cargo run -- resources
```

## トラブルシューティング実例

### 実例 1: 接続が頻繁に切断される

**症状:**

```
❌ Session terminated unexpectedly
🔄 Attempting reconnection (attempt 3/5)
⚠️  High latency detected: 450ms
```

**診断手順:**

```bash
# 1. 包括的診断
cargo run -- diagnose full --instance-id i-1234567890abcdef0

# 2. ネットワーク品質チェック
cargo run -- health --comprehensive

# 3. パフォーマンス履歴確認
cargo run -- database stats session-abc123
```

**解決策:**

```bash
# 1. アグレッシブ再接続モード有効化
export NIMBUS_AGGRESSIVE_RECONNECTION=true
export NIMBUS_AGGRESSIVE_ATTEMPTS=15

# 2. ヘルスチェック間隔短縮
export NIMBUS_HEALTH_CHECK_INTERVAL=3

# 3. 接続再試行
cargo run -- connect --instance-id i-1234567890abcdef0
```

### 実例 2: メモリ使用量が制限を超過

**症状:**

```
⚠️  Resource limit violations:
    - Memory: 12.5MB > 10.0MB
🔧 Optimization needed
```

**診断手順:**

```bash
# 1. 詳細リソース分析
cargo run -- resources

# 2. セッション数確認
cargo run -- list

# 3. 最適化実行
cargo run -- resources  # 自動最適化が実行される
```

**解決策:**

```bash
# 1. 不要セッション終了
cargo run -- terminate session-old-001
cargo run -- terminate session-old-002

# 2. 省電力モード有効化
export NIMBUS_LOW_POWER_MODE=true

# 3. メモリ制限調整 (必要に応じて)
export NIMBUS_MAX_MEMORY_MB=8
```

### 実例 3: AWS 認証エラー

**症状:**

```
❌ AWS API error: AuthenticationFailed
   Error: The security token included in the request is invalid
```

**診断手順:**

```bash
# 1. AWS 設定検証
cargo run -- diagnose aws-config --instance-id i-1234567890abcdef0

# 2. 認証情報確認
aws sts get-caller-identity

# 3. プロファイル確認
aws configure list-profiles
```

**解決策:**

```bash
# 1. 認証情報更新
aws configure

# 2. 特定プロファイル使用
cargo run -- connect \
  --instance-id i-1234567890abcdef0 \
  --profile updated-profile

# 3. 一時的な認証情報使用
export AWS_ACCESS_KEY_ID=AKIA...
export AWS_SECRET_ACCESS_KEY=...
export AWS_SESSION_TOKEN=...
```

## ベストプラクティス

### 1. セキュリティ

```bash
# 認証情報の安全な管理
aws configure set profile.production.region us-east-1
aws configure set profile.production.output json

# 最小権限の原則
# IAM ポリシーで必要最小限の権限のみ付与

# セッション監査
cargo run -- database export --format json --output audit-$(date +%Y%m%d).json
```

### 2. パフォーマンス

```bash
# リソース制限の適切な設定
export NIMBUS_MAX_MEMORY_MB=8
export NIMBUS_MAX_CPU_PERCENT=0.3

# 予防的チェックの活用
cargo run -- diagnose preventive --instance-id $INSTANCE_ID

# 定期的な最適化
cargo run -- resources
```

### 3. 運用

```bash
# ログの適切な管理
export NIMBUS_LOG_LEVEL=info
export NIMBUS_FILE_LOGGING=true
export NIMBUS_JSON_LOGGING=true

# 定期的なヘルスチェック
*/5 * * * * /path/to/nimbus health --comprehensive

# データベースメンテナンス
0 2 * * 0 /path/to/nimbus database cleanup --days 30
```

### 4. チーム開発

```bash
# 共通設定の管理
git add .kiro/specs/nimbus-improvements/
git commit -m "Update Nimbus configuration"

# 環境別設定
cp config.json.dev ~/.config/nimbus/config.json  # 開発環境
cp config.json.prod ~/.config/nimbus/config.json # 本番環境

# ドキュメント更新
cargo run -- config show > team-config-$(date +%Y%m%d).md
```

## 次のステップ

1. **[API リファレンス](API_REFERENCE.md)** - 詳細な API 仕様
2. **[トラブルシューティングガイド](TROUBLESHOOTING.md)** - 問題解決方法
3. **[設定ガイド](CONFIGURATION.md)** - 詳細な設定方法
4. **[パフォーマンス最適化](PERFORMANCE_OPTIMIZATION.md)** - 最適化テクニック

## サポート

- **GitHub Issues**: バグレポートや機能要求
- **ドキュメント**: 最新の使用方法とベストプラクティス
- **コミュニティ**: 使用例の共有と質問

---

このチュートリアルが Nimbus v3.0 の効果的な活用に役立つことを願っています。質問や改善提案がありましたら、お気軽にお知らせください。
