# EC2 Connect トラブルシューティングガイド

## 概要

EC2 Connect v3.0 で発生する可能性のある問題と、その解決方法を体系的にまとめたガイドです。問題の種類別に整理し、段階的な診断手順と解決策を提供します。

## 目次

- [クイック診断](#クイック診断)
- [接続問題](#接続問題)
- [パフォーマンス問題](#パフォーマンス問題)
- [設定問題](#設定問題)
- [AWS 関連問題](#aws-関連問題)
- [システムリソース問題](#システムリソース問題)
- [VS Code 統合問題](#vs-code-統合問題)
- [データベース問題](#データベース問題)
- [ログ分析](#ログ分析)
- [高度なトラブルシューティング](#高度なトラブルシューティング)

## クイック診断

### 自動診断コマンド

問題が発生した場合、まず以下のコマンドで包括的な診断を実行してください：

```bash
# 包括的システム診断
cargo run -- diagnose full --instance-id <INSTANCE_ID> --timeout 60

# システムヘルスチェック
cargo run -- health --comprehensive

# 設定検証
cargo run -- config validate

# リソース状態確認
cargo run -- resources
```

### 診断結果の読み方

**正常な状態:**
```
✅ Overall Health: HEALTHY
✅ All resource limits satisfied
✅ Configuration is valid
🎯 Connection Likelihood: Very High (95%)
```

**問題がある状態:**
```
❌ Overall Health: UNHEALTHY
⚠️  Resource limit violations: Memory: 12.5MB > 10.0MB
❌ Configuration validation failed: Invalid region 'invalid-region'
🛑 Connection Likelihood: Low (35%)
```

## 接続問題

### 問題 1: 接続が確立できない

#### 症状
```
❌ Failed to create session: Connection timeout
❌ SSM session creation failed
🛑 Preventive checks failed - connection aborted
```

#### 診断手順

**ステップ 1: 基本的な確認**

```bash
# AWS 認証情報確認
aws sts get-caller-identity

# インスタンス状態確認
aws ec2 describe-instances --instance-ids <INSTANCE_ID>

# SSM エージェント状態確認
aws ssm describe-instance-information --instance-information-filter-list key=InstanceIds,valueSet=<INSTANCE_ID>
```

**ステップ 2: 予防的チェック実行**

```bash
# 詳細な予防的チェック
cargo run -- diagnose preventive \
  --instance-id <INSTANCE_ID> \
  --timeout 30 \
  --abort-on-critical false
```

**ステップ 3: AWS 設定検証**

```bash
# AWS 設定の包括的検証
cargo run -- diagnose aws-config \
  --instance-id <INSTANCE_ID> \
  --include-credentials true \
  --include-iam true \
  --include-vpc true \
  --minimum-score 75.0
```

#### 解決策

**1. SSM エージェント問題**

```bash
# インスタンスでの SSM エージェント再起動
sudo systemctl restart amazon-ssm-agent  # Amazon Linux/RHEL
sudo service amazon-ssm-agent restart    # Ubuntu/Debian

# エージェント状態確認
sudo systemctl status amazon-ssm-agent
```

**2. IAM 権限問題**

必要な IAM 権限を確認し、インスタンスプロファイルまたはユーザーに付与：

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "ssm:StartSession",
        "ssm:TerminateSession",
        "ssm:ResumeSession",
        "ssm:DescribeSessions",
        "ssm:GetConnectionStatus"
      ],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "ec2:DescribeInstances"
      ],
      "Resource": "*"
    }
  ]
}
```

**3. ネットワーク設定問題**

```bash
# セキュリティグループ確認
aws ec2 describe-security-groups --group-ids <SECURITY_GROUP_ID>

# VPC エンドポイント確認
aws ec2 describe-vpc-endpoints --filters Name=service-name,Values=com.amazonaws.<region>.ssm
```

**4. 一時的な回避策**

```bash
# 異なるリージョンで試行
cargo run -- connect \
  --instance-id <INSTANCE_ID> \
  --region us-west-2

# 異なるプロファイルで試行
cargo run -- connect \
  --instance-id <INSTANCE_ID> \
  --profile alternative-profile

# タイムアウト延長
export EC2_CONNECT_CONNECTION_TIMEOUT=60
cargo run -- connect --instance-id <INSTANCE_ID>
```

### 問題 2: 接続が頻繁に切断される

#### 症状
```
🔄 Attempting reconnection (attempt 3/5)
⚠️  Session terminated unexpectedly
⚠️  High latency detected: 450ms
```

#### 診断手順

```bash
# ネットワーク品質チェック
cargo run -- health --comprehensive

# パフォーマンス履歴確認
cargo run -- database stats <SESSION_ID>

# 接続安定性テスト
for i in {1..5}; do
  echo "Test $i:"
  cargo run -- connect --instance-id <INSTANCE_ID> &
  sleep 30
  cargo run -- health <SESSION_ID>
  cargo run -- terminate <SESSION_ID>
  sleep 10
done
```

#### 解決策

**1. 再接続ポリシー調整**

```bash
# アグレッシブ再接続モード
export EC2_CONNECT_AGGRESSIVE_RECONNECTION=true
export EC2_CONNECT_AGGRESSIVE_ATTEMPTS=20
export EC2_CONNECT_AGGRESSIVE_INTERVAL_MS=200

# 最大再試行回数増加
export EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS=15
```

**2. ヘルスチェック間隔調整**

```bash
# より頻繁なヘルスチェック
export EC2_CONNECT_HEALTH_CHECK_INTERVAL=2

# タイムアウト予測閾値調整
export EC2_CONNECT_TIMEOUT_PREDICTION_THRESHOLD=240
```

**3. ネットワーク最適化**

```bash
# レイテンシ閾値調整
export EC2_CONNECT_LATENCY_THRESHOLD_MS=300

# 最適化有効化
export EC2_CONNECT_OPTIMIZATION_ENABLED=true
```

### 問題 3: ポートフォワーディングが機能しない

#### 症状
```
✅ Session created successfully!
❌ Port 8080 is not accessible
🔍 No active connections on localhost:8080
```

#### 診断手順

```bash
# ポート使用状況確認
netstat -tlnp | grep 8080  # Linux
lsof -i :8080              # macOS

# セッション詳細確認
cargo run -- status <SESSION_ID>

# ローカル接続テスト
curl -v http://localhost:8080
telnet localhost 8080
```

#### 解決策

**1. ポート競合解決**

```bash
# 使用可能ポート確認
for port in {8080..8090}; do
  if ! lsof -i :$port > /dev/null 2>&1; then
    echo "Port $port is available"
    break
  fi
done

# 異なるポートで接続
cargo run -- connect \
  --instance-id <INSTANCE_ID> \
  --local-port 8081 \
  --remote-port 80
```

**2. ファイアウォール設定**

```bash
# macOS ファイアウォール確認
sudo pfctl -sr | grep 8080

# Linux iptables 確認
sudo iptables -L -n | grep 8080

# Windows ファイアウォール確認
netsh advfirewall firewall show rule name=all | findstr 8080
```

**3. SSM セッション設定確認**

```bash
# SSM セッション詳細確認
aws ssm describe-sessions --state Active

# セッション設定確認
aws ssm get-connection-status --target <INSTANCE_ID>
```

## パフォーマンス問題

### 問題 4: 高いメモリ使用量

#### 症状
```
⚠️  Resource limit violations:
    - Memory: 15.2MB > 10.0MB
🔧 Optimization needed
⚠️  Memory usage is approaching 85% of the 10MB limit
```

#### 診断手順

```bash
# 詳細リソース分析
cargo run -- resources

# セッション数とメモリ使用量の関係確認
cargo run -- list
cargo run -- metrics

# プロセス別メモリ使用量確認
ps aux | grep ec2-connect
top -p $(pgrep -f ec2-connect)
```

#### 解決策

**1. 不要セッション終了**

```bash
# 古いセッション確認
cargo run -- database sessions

# 非アクティブセッション終了
for session in $(cargo run -- list | grep "Inactive" | awk '{print $3}'); do
  cargo run -- terminate $session
done
```

**2. メモリ制限調整**

```bash
# 一時的な制限緩和
export EC2_CONNECT_MAX_MEMORY_MB=15

# 省電力モード有効化
export EC2_CONNECT_LOW_POWER_MODE=true

# 監視間隔延長
export EC2_CONNECT_MONITORING_INTERVAL=10
```

**3. 最適化実行**

```bash
# 自動最適化実行
cargo run -- resources

# データベースクリーンアップ
cargo run -- database cleanup --days 7

# ログファイルローテーション
find logs/ -name "*.log" -mtime +7 -delete
```

### 問題 5: 高い CPU 使用率

#### 症状
```
⚠️  Resource limit violations:
    - CPU: 1.2% > 0.5%
⚠️  High CPU usage detected
🔧 Switching to low power mode
```

#### 診断手順

```bash
# CPU 使用率詳細確認
top -p $(pgrep -f ec2-connect)
htop -p $(pgrep -f ec2-connect)

# プロファイリング実行
cargo run --release -- metrics
perf record -g cargo run -- tui
```

#### 解決策

**1. 監視頻度調整**

```bash
# 監視間隔延長
export EC2_CONNECT_HEALTH_CHECK_INTERVAL=10
export EC2_CONNECT_MONITORING_INTERVAL=15

# UI 更新間隔延長
export EC2_CONNECT_UI_UPDATE_INTERVAL_MS=2000
```

**2. 省電力モード**

```bash
# 省電力モード強制有効化
export EC2_CONNECT_LOW_POWER_MODE=true

# パフォーマンス監視無効化
export EC2_CONNECT_PERFORMANCE_MONITORING=false
```

**3. セッション数制限**

```bash
# 同時セッション数制限
export EC2_CONNECT_MAX_SESSIONS=2

# インスタンス別セッション制限
export EC2_CONNECT_MAX_SESSIONS_PER_INSTANCE=1
```

### 問題 6: 接続速度が遅い

#### 症状
```
⚠️  High latency detected: 450ms
⚠️  Connection time: 5.2s (threshold: 3.0s)
📈 Throughput: 0.5 Mbps (expected: > 1.0 Mbps)
```

#### 診断手順

```bash
# ネットワーク品質測定
ping -c 10 ssm.<region>.amazonaws.com
traceroute ssm.<region>.amazonaws.com

# 接続パフォーマンス履歴
cargo run -- database stats <SESSION_ID>

# 複数リージョンでのテスト
for region in us-east-1 us-west-2 eu-west-1; do
  echo "Testing region: $region"
  time cargo run -- connect \
    --instance-id <INSTANCE_ID> \
    --region $region &
  sleep 5
  cargo run -- terminate <SESSION_ID>
done
```

#### 解決策

**1. 最適なリージョン選択**

```bash
# 最も近いリージョンを使用
export EC2_CONNECT_AWS_REGION=us-west-2  # 西海岸の場合

# リージョン別レイテンシテスト
./scripts/region-latency-test.sh
```

**2. 接続最適化**

```bash
# 最適化有効化
export EC2_CONNECT_OPTIMIZATION_ENABLED=true

# レイテンシ閾値調整
export EC2_CONNECT_LATENCY_THRESHOLD_MS=300

# 接続タイムアウト延長
export EC2_CONNECT_CONNECTION_TIMEOUT=45
```

**3. ネットワーク設定確認**

```bash
# DNS 設定確認
nslookup ssm.<region>.amazonaws.com

# プロキシ設定確認
echo $HTTP_PROXY
echo $HTTPS_PROXY

# VPN 接続確認
ip route show
```

## 設定問題

### 問題 7: 設定ファイルエラー

#### 症状
```
❌ Configuration validation failed: Invalid format
❌ Failed to load configuration: File not found
❌ Environment variable invalid: EC2_CONNECT_MAX_MEMORY_MB='invalid'
```

#### 診断手順

```bash
# 設定ファイル存在確認
ls -la ~/.config/ec2-connect/config.json

# 設定ファイル形式確認
jq . ~/.config/ec2-connect/config.json

# 環境変数確認
env | grep EC2_CONNECT_
```

#### 解決策

**1. 設定ファイル修復**

```bash
# バックアップから復元
cp ~/.config/ec2-connect/config.json.backup ~/.config/ec2-connect/config.json

# デフォルト設定生成
cargo run -- config generate --output ~/.config/ec2-connect/config.json

# 設定検証
cargo run -- config validate
```

**2. JSON 形式エラー修正**

```bash
# JSON 形式チェック
jq . ~/.config/ec2-connect/config.json

# 一般的な JSON エラー修正
# - 末尾のカンマ削除
# - 引用符の修正
# - ブール値の修正 (true/false)
```

**3. 環境変数修正**

```bash
# 無効な環境変数削除
unset EC2_CONNECT_INVALID_VARIABLE

# 正しい形式で設定
export EC2_CONNECT_MAX_MEMORY_MB=10.0
export EC2_CONNECT_RECONNECTION_ENABLED=true

# 設定テスト
cargo run -- config test
```

### 問題 8: 権限エラー

#### 症状
```
❌ Permission denied: ~/.config/ec2-connect/config.json
❌ Failed to create log file: Permission denied
❌ SSH config not writable
```

#### 診断手順

```bash
# ファイル権限確認
ls -la ~/.config/ec2-connect/
ls -la ~/.ssh/config

# ディレクトリ権限確認
ls -ld ~/.config/ec2-connect/
ls -ld ~/.ssh/
```

#### 解決策

**1. ファイル権限修正**

```bash
# 設定ディレクトリ作成
mkdir -p ~/.config/ec2-connect/
chmod 755 ~/.config/ec2-connect/

# 設定ファイル権限修正
chmod 644 ~/.config/ec2-connect/config.json

# SSH 設定権限修正
chmod 600 ~/.ssh/config
chmod 700 ~/.ssh/
```

**2. ログディレクトリ権限**

```bash
# ログディレクトリ作成
mkdir -p logs/
chmod 755 logs/

# ログファイル権限修正
chmod 644 logs/*.log
```

## AWS 関連問題

### 問題 9: AWS 認証エラー

#### 症状
```
❌ AWS API error: AuthenticationFailed
❌ The security token included in the request is invalid
❌ Unable to locate credentials
```

#### 診断手順

```bash
# 認証情報確認
aws sts get-caller-identity
aws configure list

# プロファイル確認
aws configure list-profiles
cat ~/.aws/credentials
cat ~/.aws/config
```

#### 解決策

**1. 認証情報更新**

```bash
# 基本認証情報設定
aws configure

# プロファイル別設定
aws configure --profile production

# 一時的な認証情報
export AWS_ACCESS_KEY_ID=AKIA...
export AWS_SECRET_ACCESS_KEY=...
export AWS_SESSION_TOKEN=...
```

**2. MFA 認証**

```bash
# MFA トークン取得
aws sts get-session-token \
  --serial-number arn:aws:iam::123456789012:mfa/user \
  --token-code 123456

# 一時的な認証情報設定
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
export AWS_SESSION_TOKEN=...
```

**3. IAM ロール使用**

```bash
# ロール引き受け
aws sts assume-role \
  --role-arn arn:aws:iam::123456789012:role/EC2ConnectRole \
  --role-session-name ec2-connect-session

# 認証情報設定
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
export AWS_SESSION_TOKEN=...
```

### 問題 10: リージョン・プロファイル問題

#### 症状
```
❌ Invalid region: 'invalid-region'
❌ Profile 'nonexistent' not found
❌ No instances found in region us-east-1
```

#### 診断手順

```bash
# 利用可能リージョン確認
aws ec2 describe-regions --output table

# プロファイル一覧確認
aws configure list-profiles

# インスタンス存在確認
aws ec2 describe-instances --region <REGION>
```

#### 解決策

**1. 正しいリージョン指定**

```bash
# 正しいリージョン形式
cargo run -- connect \
  --instance-id <INSTANCE_ID> \
  --region us-east-1

# 環境変数設定
export AWS_DEFAULT_REGION=us-east-1
export EC2_CONNECT_AWS_REGION=us-east-1
```

**2. プロファイル設定**

```bash
# 新しいプロファイル作成
aws configure --profile newprofile

# プロファイル使用
cargo run -- connect \
  --instance-id <INSTANCE_ID> \
  --profile newprofile
```

## システムリソース問題

### 問題 11: ディスク容量不足

#### 症状
```
❌ Failed to write log file: No space left on device
⚠️  Disk space low: 95% used
❌ Database operation failed: Disk full
```

#### 診断手順

```bash
# ディスク使用量確認
df -h
du -sh ~/.config/ec2-connect/
du -sh logs/

# 大きなファイル検索
find . -type f -size +10M -ls
```

#### 解決策

**1. ログファイルクリーンアップ**

```bash
# 古いログファイル削除
find logs/ -name "*.log" -mtime +7 -delete

# ログローテーション設定
logrotate -f /etc/logrotate.d/ec2-connect
```

**2. データベースクリーンアップ**

```bash
# 古いデータ削除
cargo run -- database cleanup --days 7

# データベース最適化
sqlite3 ~/.config/ec2-connect/sessions.db "VACUUM;"
```

**3. 一時ファイルクリーンアップ**

```bash
# 一時ファイル削除
rm -rf /tmp/ec2-connect-*
rm -rf ~/.cache/ec2-connect/

# システム一時ファイル削除
sudo apt-get clean  # Ubuntu/Debian
sudo yum clean all  # RHEL/CentOS
```

### 問題 12: プロセス制限

#### 症状
```
❌ Failed to create process: Resource temporarily unavailable
⚠️  Process count exceeded: 1024 > 1000
❌ Too many open files
```

#### 診断手順

```bash
# プロセス数確認
ps aux | grep ec2-connect | wc -l
pgrep -c ec2-connect

# ファイルディスクリプタ確認
lsof -p $(pgrep ec2-connect) | wc -l
ulimit -n
```

#### 解決策

**1. プロセス制限調整**

```bash
# 一時的な制限緩和
ulimit -n 2048
ulimit -u 2048

# 永続的な制限変更 (/etc/security/limits.conf)
echo "* soft nofile 2048" | sudo tee -a /etc/security/limits.conf
echo "* hard nofile 4096" | sudo tee -a /etc/security/limits.conf
```

**2. 不要プロセス終了**

```bash
# 古いプロセス終了
pkill -f "ec2-connect.*terminated"

# ゾンビプロセス確認
ps aux | grep -E "defunct|<zombie>"
```

## VS Code 統合問題

### 問題 13: VS Code 自動起動失敗

#### 症状
```
❌ VS Code integration failed: VS Code not found
⚠️  VS Code integration unavailable: /usr/bin/code not executable
❌ Failed to launch VS Code: Permission denied
```

#### 診断手順

```bash
# VS Code インストール確認
which code
code --version

# VS Code 統合状態確認
cargo run -- vscode status

# SSH 設定確認
cat ~/.ssh/config | grep ec2-
```

#### 解決策

**1. VS Code インストール**

```bash
# macOS (Homebrew)
brew install --cask visual-studio-code

# Ubuntu/Debian
wget -qO- https://packages.microsoft.com/keys/microsoft.asc | gpg --dearmor > packages.microsoft.gpg
sudo install -o root -g root -m 644 packages.microsoft.gpg /etc/apt/trusted.gpg.d/
sudo sh -c 'echo "deb [arch=amd64,arm64,armhf signed-by=/etc/apt/trusted.gpg.d/packages.microsoft.gpg] https://packages.microsoft.com/repos/code stable main" > /etc/apt/sources.list.d/vscode.list'
sudo apt update
sudo apt install code
```

**2. VS Code パス設定**

```bash
# VS Code パス確認
which code

# 環境変数設定
export EC2_CONNECT_VSCODE_PATH=/usr/local/bin/code

# 設定ファイル更新
cargo run -- config show
```

**3. SSH 設定修正**

```bash
# SSH 設定ディレクトリ作成
mkdir -p ~/.ssh/
chmod 700 ~/.ssh/

# SSH 設定ファイル作成
touch ~/.ssh/config
chmod 600 ~/.ssh/config

# VS Code 統合セットアップ
cargo run -- vscode setup
```

### 問題 14: SSH 設定競合

#### 症状
```
⚠️  SSH config conflict detected
❌ Failed to update SSH config: Host already exists
⚠️  SSH Host 'ec2-i-1234567890abcdef0' already configured
```

#### 診断手順

```bash
# SSH 設定確認
cat ~/.ssh/config | grep -A 10 "Host ec2-"

# 重複エントリ確認
grep -n "Host ec2-" ~/.ssh/config
```

#### 解決策

**1. SSH 設定クリーンアップ**

```bash
# EC2 Connect 関連エントリ削除
cargo run -- vscode cleanup

# 手動クリーンアップ
sed -i '/# EC2 Connect - Start/,/# EC2 Connect - End/d' ~/.ssh/config
```

**2. SSH 設定バックアップと復元**

```bash
# バックアップ作成
cp ~/.ssh/config ~/.ssh/config.backup.$(date +%Y%m%d)

# 問題のあるエントリ削除
vim ~/.ssh/config

# VS Code 統合再設定
cargo run -- vscode setup
```

## データベース問題

### 問題 15: データベース破損

#### 症状
```
❌ Database operation failed: database disk image is malformed
❌ Failed to load sessions: SQL error
⚠️  Database integrity check failed
```

#### 診断手順

```bash
# データベース整合性チェック
sqlite3 ~/.config/ec2-connect/sessions.db "PRAGMA integrity_check;"

# データベース情報確認
cargo run -- database info

# データベースファイル確認
ls -la ~/.config/ec2-connect/sessions.db
file ~/.config/ec2-connect/sessions.db
```

#### 解決策

**1. データベース修復**

```bash
# データベースバックアップ
cp ~/.config/ec2-connect/sessions.db ~/.config/ec2-connect/sessions.db.backup

# データベース修復
sqlite3 ~/.config/ec2-connect/sessions.db ".recover" | sqlite3 ~/.config/ec2-connect/sessions_recovered.db

# 修復されたデータベースに置き換え
mv ~/.config/ec2-connect/sessions_recovered.db ~/.config/ec2-connect/sessions.db
```

**2. データベース再初期化**

```bash
# 古いデータベース削除
rm ~/.config/ec2-connect/sessions.db

# データベース再初期化
cargo run -- database init

# データベース情報確認
cargo run -- database info
```

**3. データエクスポート・インポート**

```bash
# データエクスポート (破損前のバックアップから)
cargo run -- database export --output backup-data.json --format json

# データベース再初期化後、必要に応じて手動でデータ復元
```

## ログ分析

### ログファイルの場所

```bash
# デフォルトログディレクトリ
ls -la logs/

# 日付別ログファイル
ls -la logs/ec2-connect.$(date +%Y-%m-%d)

# 設定されたログファイル
cargo run -- config show | grep log_file
```

### 重要なログパターン

**接続成功:**
```
INFO ec2_connect: Starting EC2 Connect v3.0.0
INFO ec2_connect::session: Session created successfully: session-abc123
INFO ec2_connect::monitor: Session monitoring started for session-abc123
```

**接続失敗:**
```
ERROR ec2_connect::aws: AWS API error: AuthenticationFailed
ERROR ec2_connect::session: Failed to create session: Connection timeout
WARN ec2_connect::reconnect: Reconnection attempt 3/5 failed
```

**パフォーマンス問題:**
```
WARN ec2_connect::resource: Memory usage exceeded: 12.5MB > 10.0MB
WARN ec2_connect::performance: High latency detected: 450ms
INFO ec2_connect::resource: Optimization completed: 12.5MB -> 8.2MB
```

### ログ分析コマンド

```bash
# エラーログ抽出
grep -i error logs/ec2-connect.$(date +%Y-%m-%d)

# 警告ログ抽出
grep -i warn logs/ec2-connect.$(date +%Y-%m-%d)

# 特定セッションのログ
grep "session-abc123" logs/ec2-connect.$(date +%Y-%m-%d)

# パフォーマンス関連ログ
grep -E "(latency|memory|cpu)" logs/ec2-connect.$(date +%Y-%m-%d)

# 時系列でのエラー分析
tail -f logs/ec2-connect.$(date +%Y-%m-%d) | grep -E "(ERROR|WARN)"
```

## 高度なトラブルシューティング

### デバッグモード

```bash
# デバッグログ有効化
export EC2_CONNECT_LOG_LEVEL=debug
export RUST_LOG=debug

# 詳細ログ付きで実行
cargo run -- connect --instance-id <INSTANCE_ID> --verbose

# トレースログ有効化
export RUST_LOG=trace
cargo run -- diagnose full --instance-id <INSTANCE_ID>
```

### パフォーマンス分析

```bash
# プロファイリング実行
cargo build --release
perf record --call-graph=dwarf target/release/ec2-connect connect --instance-id <INSTANCE_ID>
perf report

# メモリ使用量分析
valgrind --tool=massif target/release/ec2-connect connect --instance-id <INSTANCE_ID>
ms_print massif.out.*
```

### ネットワーク分析

```bash
# パケットキャプチャ
sudo tcpdump -i any -w ec2-connect.pcap host ssm.<region>.amazonaws.com

# SSL/TLS 接続分析
openssl s_client -connect ssm.<region>.amazonaws.com:443 -servername ssm.<region>.amazonaws.com

# DNS 解決確認
dig ssm.<region>.amazonaws.com
nslookup ssm.<region>.amazonaws.com
```

### システムコール分析

```bash
# システムコール追跡
strace -f -o ec2-connect.strace cargo run -- connect --instance-id <INSTANCE_ID>

# ファイルアクセス分析
strace -e trace=file cargo run -- connect --instance-id <INSTANCE_ID>

# ネットワークアクセス分析
strace -e trace=network cargo run -- connect --instance-id <INSTANCE_ID>
```

## 緊急時の対応

### 完全リセット手順

```bash
# 1. 全セッション終了
cargo run -- list | grep -E "session-" | awk '{print $3}' | xargs -I {} cargo run -- terminate {}

# 2. プロセス強制終了
pkill -f ec2-connect

# 3. 設定ファイル削除
rm -rf ~/.config/ec2-connect/

# 4. ログファイル削除
rm -rf logs/

# 5. SSH 設定クリーンアップ
sed -i '/# EC2 Connect/d' ~/.ssh/config

# 6. 環境変数クリア
unset $(env | grep EC2_CONNECT_ | cut -d= -f1)

# 7. 再初期化
cargo run -- config generate
cargo run -- database init
```

### バックアップからの復元

```bash
# 設定ファイル復元
cp ~/.config/ec2-connect/config.json.backup ~/.config/ec2-connect/config.json

# データベース復元
cp ~/.config/ec2-connect/sessions.db.backup ~/.config/ec2-connect/sessions.db

# SSH 設定復元
cp ~/.ssh/config.backup ~/.ssh/config

# 動作確認
cargo run -- config validate
cargo run -- database info
cargo run -- health
```

## サポートとコミュニティ

### 問題報告時の情報収集

```bash
# システム情報収集
echo "=== System Information ===" > debug-info.txt
uname -a >> debug-info.txt
cargo --version >> debug-info.txt
rustc --version >> debug-info.txt

echo "=== Configuration ===" >> debug-info.txt
cargo run -- config show >> debug-info.txt

echo "=== Health Check ===" >> debug-info.txt
cargo run -- health --comprehensive >> debug-info.txt

echo "=== Resource Usage ===" >> debug-info.txt
cargo run -- resources >> debug-info.txt

echo "=== Recent Logs ===" >> debug-info.txt
tail -50 logs/ec2-connect.$(date +%Y-%m-%d) >> debug-info.txt
```

### よくある質問 (FAQ)

**Q: 接続が遅いのですが、どうすれば改善できますか？**
A: まず `cargo run -- diagnose full` で包括的な診断を実行し、ネットワーク品質とAWS設定を確認してください。最適なリージョンの選択と、`EC2_CONNECT_OPTIMIZATION_ENABLED=true` の設定が効果的です。

**Q: メモリ使用量が制限を超えてしまいます。**
A: 不要なセッションを終了し、`EC2_CONNECT_LOW_POWER_MODE=true` を設定してください。また、`cargo run -- database cleanup` で古いデータを削除することも効果的です。

**Q: VS Code 統合が機能しません。**
A: `cargo run -- vscode status` で統合状態を確認し、VS Code のパスが正しく設定されているか確認してください。必要に応じて `EC2_CONNECT_VSCODE_PATH` 環境変数を設定してください。

**Q: 設定ファイルが見つからないエラーが出ます。**
A: `cargo run -- config generate` で新しい設定ファイルを生成してください。設定ディレクトリの権限も確認してください。

---

このトラブルシューティングガイドで解決できない問題がある場合は、GitHub Issues でバグレポートを作成するか、コミュニティフォーラムで質問してください。問題報告時は、上記の情報収集手順で得られた情報を含めていただくと、より迅速な解決が可能です。