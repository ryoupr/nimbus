---
inclusion: always
---

# プロジェクト構造

## レイヤーアーキテクチャ

```
┌─────────────────────────────────────────────────────────┐
│  CLI Layer (main.rs)                                    │
│  - コマンド解析（clap）                                  │
│  - ルーティング                                          │
├─────────────────────────────────────────────────────────┤
│  Business Logic Layer                                   │
│  - manager.rs: セッションライフサイクル                   │
│  - monitor.rs: 健全性監視                                │
│  - reconnect.rs: 自動再接続                              │
│  - diagnostic.rs: 診断オーケストレーション                │
├─────────────────────────────────────────────────────────┤
│  Integration Layer                                      │
│  - aws.rs: AWS SDK 統合（SSM, EC2, IAM, STS）            │
│  - vscode.rs: VS Code 連携                              │
├─────────────────────────────────────────────────────────┤
│  Data Layer                                             │
│  - persistence.rs: SQLite 永続化                         │
│  - config.rs: 設定管理                                   │
└─────────────────────────────────────────────────────────┘
```

## モジュール責務

### コア（変更時は影響範囲に注意）

| モジュール | 責務 | 依存先 |
|-----------|------|--------|
| `session` | セッション状態定義 | なし |
| `manager` | セッション作成・終了 | `aws`, `session`, `config` |
| `aws` | AWS API 呼び出し | AWS SDK |
| `config` | 設定読み込み・検証 | `serde` |

### 診断系（`*_diagnostics.rs`）

すべて `diagnostic.rs` から呼び出される。新規診断追加時は `DiagnosticManager` に登録。

- `instance_diagnostics`: EC2 状態確認
- `port_diagnostics`: ポート到達性
- `ssm_agent_diagnostics`: SSM Agent 状態
- `iam_diagnostics`: 権限検証
- `network_diagnostics`: VPC/SG 設定

### 修復系

- `auto_fix`: 自動修復実行（`suggestion_generator` の提案を実行）
- `suggestion_generator`: 診断結果から修復手順を生成
- `preventive_check`: 接続前の事前チェック

### UI 系

- `ui`: ratatui ベースの TUI
- `multi_session_ui`: 複数セッション表示
- `realtime_feedback`: 進捗表示

## 新規ファイル追加時のルール

1. **診断モジュール追加**: `src/*_diagnostics.rs` として作成し、`diagnostic.rs` の `DiagnosticManager` に登録
2. **設定項目追加**: `config.rs` の `Config` 構造体に追加し、`config.*.example` を更新
3. **エラー型追加**: `error.rs` の `NimbusError` に variant を追加

## 設定ファイルパス

| 種別 | Windows | Unix |
|------|---------|------|
| 設定 | `%APPDATA%\nimbus\config.json` | `~/.config/nimbus/config.json` |
| DB | `%APPDATA%\nimbus\sessions.db` | `~/.config/nimbus/sessions.db` |
| ログ | `./logs/nimbus.YYYY-MM-DD` | `./logs/nimbus.YYYY-MM-DD` |

## アーキテクチャ原則

- **非同期優先**: すべての I/O は `async/await`（Tokio ランタイム）
- **エラー伝播**: `anyhow::Result` + `.context()` でコンテキスト付与
- **設定階層**: デフォルト → ファイル → 環境変数（後勝ち）
- **Feature フラグ**: `advanced` feature で追加機能を有効化
