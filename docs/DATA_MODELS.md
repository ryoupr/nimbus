# データモデル仕様書

## 概要

Nimbus v3.0 のコアデータモデルとその使用方法を説明します。すべてのデータモデルは Serde を使用してシリアライゼーション/デシリアライゼーションに対応しています。

## コア構造体

### Session

SSM 接続セッションを表現する主要な構造体です。

```rust
pub struct Session {
    pub id: String,                      // UUID v4形式のセッションID
    pub instance_id: String,             // EC2インスタンスID
    pub local_port: u16,                 // ローカルポート番号
    pub remote_port: u16,                // リモートポート番号
    pub status: SessionStatus,           // セッション状態
    pub created_at: SystemTime,          // セッション作成時刻
    pub last_activity: SystemTime,       // 最終アクティビティ時刻
    pub process_id: Option<u32>,         // SSMプロセスID
    pub connection_count: u32,           // 接続数
    pub data_transferred: u64,           // 転送データ量（バイト）
    pub aws_profile: Option<String>,     // AWSプロファイル名
    pub region: String,                  // AWSリージョン
}
```

**主要メソッド:**

- `new()` - 新しいセッションを作成
- `is_active()` - セッションがアクティブか確認
- `is_healthy()` - セッションが健全か確認
- `update_activity()` - 最終アクティビティ時刻を更新
- `age_seconds()` - セッション作成からの経過時間（秒）
- `idle_seconds()` - 最終アクティビティからの経過時間（秒）

### SessionStatus

セッションの状態を表す列挙型です。

```rust
pub enum SessionStatus {
    Connecting,    // 接続中
    Active,        // アクティブ
    Inactive,      // 非アクティブ
    Reconnecting,  // 再接続中
    Terminated,    // 終了済み
}
```

### SessionConfig

新しいセッションを作成するための設定情報です。

```rust
pub struct SessionConfig {
    pub instance_id: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub aws_profile: Option<String>,
    pub region: String,
}
```

**使用例:**

```rust
let config = SessionConfig::new(
    "i-1234567890abcdef0".to_string(),
    8080,
    80,
    Some("default".to_string()),
    "us-east-1".to_string(),
);
```

### SessionHealth

セッションの健全性情報を表します。

```rust
pub struct SessionHealth {
    pub is_healthy: bool,
    pub last_activity: SystemTime,
    pub connection_count: u32,
    pub data_transferred: u64,
}
```

**使用例:**

```rust
let health = SessionHealth::new(&session);
if health.is_healthy {
    println!("セッションは健全です");
}
```

### SessionEvent

セッション監視イベントを表す列挙型です。

```rust
pub enum SessionEvent {
    HealthDegraded(String),           // 健全性劣化（理由）
    TimeoutPredicted(Duration),       // タイムアウト予測（残り時間）
    ActivityDetected,                 // アクティビティ検出
    ConnectionLost,                   // 接続喪失
}
```

### ResourceUsage

システムリソース使用状況を表します。

```rust
pub struct ResourceUsage {
    pub memory_mb: f64,        // メモリ使用量（MB）
    pub cpu_percent: f64,      // CPU使用率（%）
    pub active_sessions: u32,  // アクティブセッション数
}
```

**主要メソッド:**

- `new()` - 新しいリソース使用状況を作成
- `is_within_limits()` - リソース制限内か確認

**使用例:**

```rust
let usage = ResourceUsage::new(8.5, 0.3, 2);
if usage.is_within_limits(10.0, 0.5) {
    println!("リソース使用量は制限内です");
}
```

### ReconnectionPolicy

自動再接続ポリシーを定義します。

```rust
pub struct ReconnectionPolicy {
    pub enabled: bool,                    // 自動再接続有効化
    pub max_attempts: u32,                // 最大再試行回数
    pub base_delay: Duration,             // 基本遅延時間
    pub max_delay: Duration,              // 最大遅延時間
    pub aggressive_mode: bool,            // アグレッシブモード
    pub aggressive_attempts: u32,         // アグレッシブ試行回数
    pub aggressive_interval: Duration,    // アグレッシブ間隔
}
```

**プリセットメソッド:**

- `new()` - デフォルトポリシー（5 回試行、指数バックオフ）
- `aggressive()` - アグレッシブポリシー（10 回試行、500ms 間隔）
- `conservative()` - 保守的ポリシー（3 回試行、長い間隔）
- `disabled()` - 自動再接続無効

**使用例:**

```rust
// デフォルトポリシー
let policy = ReconnectionPolicy::new();

// アグレッシブモード
let aggressive = ReconnectionPolicy::aggressive();

// カスタムポリシー
let custom = ReconnectionPolicy {
    enabled: true,
    max_attempts: 7,
    base_delay: Duration::from_secs(2),
    max_delay: Duration::from_secs(30),
    aggressive_mode: false,
    aggressive_attempts: 0,
    aggressive_interval: Duration::from_secs(1),
};

// 遅延時間の計算
let delay = policy.calculate_delay(3); // 3回目の試行の遅延時間
```

### PerformanceMetrics

セッションのパフォーマンスメトリクスを記録します。

```rust
pub struct PerformanceMetrics {
    pub session_id: String,
    pub connection_time: f64,    // 接続時間（ミリ秒）
    pub latency: f64,            // レイテンシ（ミリ秒）
    pub throughput: f64,         // スループット（MB/s）
    pub cpu_usage: f64,          // CPU使用率（%）
    pub memory_usage: f64,       // メモリ使用量（MB）
    pub timestamp: SystemTime,   // タイムスタンプ
}
```

**ビルダーメソッド:**

```rust
let metrics = PerformanceMetrics::new("session-123".to_string())
    .with_connection_time(150.5)
    .with_latency(25.3)
    .with_throughput(10.5)
    .with_resource_usage(0.3, 8.2);
```

## 設定構造体

### ReconnectionConfig

設定ファイルで使用される再接続設定です。

```rust
pub struct ReconnectionConfig {
    pub enabled: bool,
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub aggressive_mode: bool,
    pub aggressive_attempts: u32,
    pub aggressive_interval_ms: u64,
}
```

**変換メソッド:**

```rust
// ReconnectionConfigからReconnectionPolicyへ変換
let policy = config.reconnection.to_policy();

// ReconnectionPolicyからReconnectionConfigへ変換
let config = ReconnectionConfig::from_policy(&policy);
```

## シリアライゼーション

すべての構造体は Serde を使用して JSON/TOML へのシリアライゼーションに対応しています。

**JSON 例:**

```rust
use serde_json;

let session = Session::new(
    "i-1234567890abcdef0".to_string(),
    8080,
    80,
    Some("default".to_string()),
    "us-east-1".to_string(),
);

// JSONへシリアライズ
let json = serde_json::to_string_pretty(&session)?;
println!("{}", json);

// JSONからデシリアライズ
let session: Session = serde_json::from_str(&json)?;
```

## データベース永続化

Session と PerformanceMetrics は SQLite データベースに永続化されます。詳細は`database.md`を参照してください。

## 要件との対応

このデータモデル実装は以下の要件を満たしています：

- **要件 6.5**: セッション状態の永続化
  - すべての構造体が Serde に対応
  - SQLite への保存・読み込みが可能
  - アプリケーション再起動後も状態を復元可能

## 使用例

### セッション作成と管理

```rust
// セッション作成
let mut session = Session::new(
    "i-1234567890abcdef0".to_string(),
    8080,
    80,
    Some("default".to_string()),
    "us-east-1".to_string(),
);

// セッション状態更新
session.status = SessionStatus::Active;
session.update_activity();
session.connection_count += 1;

// 健全性チェック
if session.is_healthy() {
    println!("セッションは健全です");
}

// アイドル時間チェック
if session.idle_seconds() > 30 {
    println!("セッションが30秒以上アイドル状態です");
}
```

### 再接続ポリシーの使用

```rust
// アグレッシブポリシーで再接続
let policy = ReconnectionPolicy::aggressive();

for attempt in 1..=policy.max_attempts {
    let delay = policy.calculate_delay(attempt);
    println!("試行 {}: {:?}後に再接続", attempt, delay);

    tokio::time::sleep(delay).await;

    // 再接続試行
    if try_reconnect().await? {
        break;
    }
}
```

### パフォーマンス測定

```rust
let start = Instant::now();

// 接続処理
connect_to_instance().await?;

let connection_time = start.elapsed().as_millis() as f64;

// メトリクス記録
let metrics = PerformanceMetrics::new(session.id.clone())
    .with_connection_time(connection_time)
    .with_latency(measure_latency().await?)
    .with_resource_usage(get_cpu_usage(), get_memory_usage());

// メトリクス保存
save_metrics(&metrics).await?;
```

## 次のステップ

- [セッション監視機能の実装](./SESSION_MONITOR.md)
- [自動再接続機能の実装](./AUTO_RECONNECTOR.md)
- [データベース永続化の実装](./DATABASE.md)
