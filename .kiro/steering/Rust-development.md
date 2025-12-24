---
inclusion: always
---

1. ワークフローとタスク計画 (Workflow & Task Planning)

Kiro等の自律型エージェントとしてタスクリストを生成・実行する際は、以下の "Check First, Build Last" 戦略を遵守してください。

基本ルール

開発・修正フェーズ: コードの構文・型チェックには必ず cargo check を使用してください。この段階での cargo build は禁止です。

ロジック検証: 全体のテストではなく、対象の単体テストのみを cargo test <test_name> で実行してください。

最終フェーズ: タスクの最後の手順としてのみ cargo build (または cargo run) を行い、バイナリ生成を確認してください。

2. コーディング指針 (Code Optimization Guidelines)

コンパイル時間を悪化させる設計を避け、以下のパターンを優先してください。

A. モノモーフィゼーションの抑制 (Avoid Monomorphization)

ジェネリクス (<T: Trait>) はコンパイル時間を増大させます。パフォーマンスがクリティカルなホットパス以外では、動的ディスパッチ (&dyn Trait / Box<dyn Trait>) を優先してください。

B. Inner Function パターン

パブリックAPIでジェネリクスが必要な場合、実装の詳細を非ジェネリックな内部関数に委譲してください。

// 推奨パターン
pub fn heavy_logic<T: AsRef<Path>>(path: T) {
    heavy_logic_inner(path.as_ref())
}
// ここにロジックを集約（コード生成は1回のみ）
fn heavy_logic_inner(path: &Path) { ... }


C. 依存関係の最小化

不要なクレートの追加を避けてください。

外部クレートを追加する際は、可能な限り default-features = false を指定し、必要な機能のみを features で有効化してください。

手続き型マクロ（serde, sqlx, tokio::main 等）の使用は必要最小限に留めてください。

D. 型推論の補助

複雑なイテレータチェーンやコンビネータには、明示的な型注釈を与えてコンパイラの推論コストを下げてください。

3. 環境設定チェック (Configuration Setup)

プロジェクトの設定が最適化されているか確認し、未設定の場合は以下を提案してください。

リンカの高速化 (.cargo/config.toml)

OSに応じて mold (Linux) または lld (macOS/Windows) を使用する設定。

# Linux example
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold", "-Zshare-generics=y"]

# macOS example
[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]


開発プロファイルの最適化 (Cargo.toml)

依存関係の最適化を行い、自身のコードのビルドは最速にする設定。

[profile.dev]
opt-level = 0
debug = 0  # デバッグ情報を減らしてリンク時間を短縮
strip = "debuginfo"

# 依存関係は最適化して実行速度を確保（変更頻度が低いため）
[profile.dev.package."*"]
opt-level = 3
