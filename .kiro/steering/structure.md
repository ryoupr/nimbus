# プロジェクト構造

## ディレクトリ構成

```
ec2-connect/
├── src/                          # ソースコード
│   ├── main.rs                   # CLI エントリーポイント
│   ├── lib.rs                    # ライブラリエクスポート
│   ├── aws.rs                    # AWS SDK 統合（SSM, EC2, IAM, STS）
│   ├── config.rs                 # 設定管理
│   ├── session.rs                # セッション定義
│   ├── manager.rs                # セッションマネージャー
│   ├── monitor.rs                # セッション監視
│   ├── reconnect.rs              # 自動再接続
│   ├── performance.rs            # パフォーマンス監視
│   ├── health.rs                 # ヘルスチェック
│   ├── resource.rs               # リソース監視
│   ├── persistence.rs            # データベース永続化
│   ├── ui.rs                     # ターミナル UI
│   ├── multi_session.rs          # マルチセッション管理
│   ├── multi_session_ui.rs       # マルチセッション UI
│   ├── vscode.rs                 # VS Code 統合
│   ├── diagnostic.rs             # 診断マネージャー
│   ├── instance_diagnostics.rs  # インスタンス診断
│   ├── port_diagnostics.rs      # ポート診断
│   ├── ssm_agent_diagnostics.rs # SSM エージェント診断
│   ├── iam_diagnostics.rs       # IAM 権限診断
│   ├── network_diagnostics.rs   # ネットワーク診断
│   ├── auto_fix.rs              # 自動修復
│   ├── suggestion_generator.rs  # 修復提案生成
│   ├── preventive_check.rs      # 予防的チェック
│   ├── report_manager.rs        # レポート管理
│   ├── aws_config_validator.rs  # AWS 設定検証
│   ├── diagnostic_feedback.rs   # 診断フィードバック
│   ├── realtime_feedback.rs     # リアルタイムフィードバック
│   ├── error.rs                 # エラー型定義
│   ├── error_recovery.rs        # エラー回復
│   ├── logging.rs               # ログ管理
│   └── user_messages.rs         # ユーザーメッセージ
├── benches/                      # ベンチマーク
│   └── performance_benchmarks.rs
├── docs/                         # ドキュメント
│   ├── INDEX.md                  # ドキュメント索引
│   ├── API_REFERENCE.md          # API リファレンス
│   ├── TUTORIALS.md              # チュートリアル
│   ├── TROUBLESHOOTING.md        # トラブルシューティング
│   ├── PERFORMANCE_OPTIMIZATION.md # パフォーマンス最適化
│   ├── CONFIGURATION.md          # 設定ガイド
│   └── DATA_MODELS.md            # データモデル仕様
├── logs/                         # ログファイル（実行時生成）
├── performance_results/          # パフォーマンステスト結果
├── .cargo/                       # Cargo 設定
│   └── config.toml               # リンカ最適化設定
├── Cargo.toml                    # プロジェクト定義
├── Cargo.lock                    # 依存関係ロック
├── config.json.example           # 設定ファイル例（JSON）
├── config.toml.example           # 設定ファイル例（TOML）
├── run.sh                        # Unix 実行スクリプト
├── run.ps1                       # Windows 実行スクリプト
├── run_performance_tests.sh      # パフォーマンステスト（Unix）
├── run_performance_tests.ps1     # パフォーマンステスト（Windows）
└── README.md                     # プロジェクト概要
```

## モジュール構成

### コアモジュール
- `aws`: AWS サービスとの統合レイヤー
- `config`: 設定ファイル・環境変数の管理
- `session`: セッション定義と状態管理
- `manager`: セッションライフサイクル管理

### 監視・最適化モジュール
- `monitor`: セッション健全性監視
- `reconnect`: 自動再接続ロジック
- `performance`: パフォーマンスメトリクス収集
- `health`: システム・セッションヘルスチェック
- `resource`: リソース使用量監視

### 診断・修復モジュール
- `diagnostic`: 診断フレームワーク
- `instance_diagnostics`: EC2 インスタンス診断
- `port_diagnostics`: ポート可用性診断
- `ssm_agent_diagnostics`: SSM エージェント診断
- `iam_diagnostics`: IAM 権限診断
- `network_diagnostics`: ネットワーク接続診断
- `auto_fix`: 自動修復実行
- `suggestion_generator`: 修復提案生成
- `preventive_check`: 接続前予防チェック
- `aws_config_validator`: AWS 設定検証

### UI・フィードバックモジュール
- `ui`: ターミナル UI（ratatui）
- `multi_session_ui`: マルチセッション UI
- `diagnostic_feedback`: 診断フィードバックシステム
- `realtime_feedback`: リアルタイムフィードバック

### ユーティリティモジュール
- `persistence`: SQLite データベース管理
- `vscode`: VS Code 統合
- `error`: エラー型定義
- `error_recovery`: エラー回復戦略
- `logging`: 構造化ログ
- `user_messages`: ユーザー向けメッセージ生成
- `report_manager`: 診断レポート生成

## 設定ファイル配置

### ユーザー設定
- **Windows**: `%APPDATA%\ec2-connect\config.json`
- **Linux/macOS**: `~/.config/ec2-connect/config.json`

### ログファイル
- プロジェクトルート: `logs/ec2-connect.YYYY-MM-DD`

### データベース
- ユーザー設定ディレクトリ: `sessions.db`

## アーキテクチャパターン

### レイヤー構造
1. **CLI レイヤー** (`main.rs`): コマンド解析とルーティング
2. **ビジネスロジックレイヤー** (`manager`, `monitor`, `reconnect`): コア機能
3. **統合レイヤー** (`aws`, `vscode`): 外部サービス統合
4. **データレイヤー** (`persistence`, `config`): データ管理

### 非同期処理
- Tokio ランタイムを使用した非同期処理
- `async/await` による非同期 API
- `async-trait` による trait の非同期化

### エラーハンドリング
- カスタムエラー型 (`Ec2ConnectError`)
- `anyhow::Result` による伝播
- コンテキスト付きエラー (`ContextualError`)
- エラー回復戦略 (`ErrorRecoveryManager`)

### 設定管理
- 階層的設定（デフォルト → ファイル → 環境変数）
- 複数フォーマット対応（JSON, TOML, YAML）
- 実行時検証とバリデーション
