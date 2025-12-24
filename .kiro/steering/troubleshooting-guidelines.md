# トラブルシューティングガイドライン

## 概要

このドキュメントは、ツール開発・保守時のトラブルシューティングにおけるベストプラクティスを定義します。

## 基本原則

### 1. 段階的診断アプローチ

**問題の特定順序**:

1. **設定ファイルの確認** - 破損や欠損がないか
2. **依存関係の確認** - 必要なツール・ライブラリが利用可能か
3. **プロセス・ポートの確認** - 実際にサービスが動作しているか
4. **ログの詳細分析** - エラーの根本原因を特定

### 2. ログ出力の充実

**必須ログ情報**:

- **コマンド実行**: 実行されるコマンドの完全な内容
- **API 呼び出し**: リクエスト・レスポンスの詳細（機密情報は除く）
- **プロセス管理**: PID、終了コード、実行時間
- **リソース状態**: ポート、ファイル、ネットワーク接続の状態
- **エラー詳細**: スタックトレース、エラーコード、関連コンテキスト

**ログレベル設定**:

```json
{
  "logging": {
    "log_level": "DEBUG", // 開発・トラブルシューティング時
    "log_level": "INFO" // 本番運用時
  }
}
```

### 3. 外部プロセス管理のベストプラクティス

**subprocess.Popen 使用時の注意点**:

- **対話的プロセス**: 標準入出力をパイプしない
- **バックグラウンドプロセス**: `stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL`
- **Windows 対応**: `creationflags=subprocess.CREATE_NO_WINDOW`
- **プロセス監視**: PID の記録と終了状態の確認

**例**:

```python
# ❌ 対話的プロセスで標準入出力をパイプ（SSMセッションなど）
process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

# ✅ 対話的プロセスの正しい実行方法
process = subprocess.Popen(
    cmd,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
    creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
)
```

### 4. ポート・リソース検証

**必須検証項目**:

- ポートの開放状態確認
- プロセスの実行状態確認
- リソースの可用性確認

**実装例**:

```python
def _is_port_open(self, port: int, host: str = "127.0.0.1") -> bool:
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.settimeout(1.0)
            return sock.connect_ex((host, port)) == 0
    except Exception:
        return False
```

## Windows 環境特有の考慮事項

### 1. PowerShell バージョン対応

**ショートカット作成時**:

- PowerShell 7 (pwsh.exe) を優先使用
- Windows PowerShell (powershell.exe) をフォールバック
- 実行ポリシーのバイパス: `-ExecutionPolicy Bypass`

### 2. 環境変数・パス問題

**ショートカット実行時の問題**:

- UV パスの自動検出・追加
- AWS 認証情報の確認
- 作業ディレクトリの明示的設定

**解決パターン**:

```powershell
# UVパスの自動追加
$uvPaths = @(
    "$env:USERPROFILE\.local\bin",
    "$env:LOCALAPPDATA\Programs\uv\bin"
)
foreach ($uvPath in $uvPaths) {
    if (Test-Path $uvPath -PathType Container) {
        if ($env:PATH -notlike "*$uvPath*") {
            $env:PATH = "$uvPath;$env:PATH"
        }
    }
}
```

## 設定ファイル管理

### 1. 設定ファイルの検証

**必須チェック項目**:

- JSON 形式の妥当性
- 必須セクションの存在確認
- デフォルト値の適用

### 2. 設定ファイルの場所

**Windows 標準パス**:

- ユーザー設定: `%APPDATA%\{app-name}\`
- 設定例ファイル: プロジェクト内に `.example` ファイルを配置

## エラーハンドリング

### 1. 段階的エラー処理

```python
try:
    # メイン処理
    result = main_operation()
except SpecificError as e:
    logger.error(f"Specific error details: {e}", exc_info=True)
    # 具体的な対処法を提示
except ClientError as e:
    error_details = {
        "error_code": e.response.get("Error", {}).get("Code"),
        "error_message": e.response.get("Error", {}).get("Message"),
        "request_id": e.response.get("ResponseMetadata", {}).get("RequestId")
    }
    logger.error(f"AWS API error: {json.dumps(error_details, indent=2)}")
except Exception as e:
    logger.error(f"Unexpected error: {e}", exc_info=True)
```

### 2. ユーザーフレンドリーなエラーメッセージ

- 日本語でのエラー説明
- 具体的な解決手順の提示
- トラブルシューティング情報の提供

## 継続的改善

### 1. ログ分析の活用

- 頻繁に発生するエラーパターンの特定
- パフォーマンスボトルネックの発見
- ユーザー体験の改善点の抽出

### 2. ドキュメント更新

- トラブルシューティング事例の蓄積
- 解決パターンの文書化
- ベストプラクティスの共有

## チェックリスト

### 新機能開発時

- [ ] 適切なログ出力の実装
- [ ] エラーハンドリングの実装
- [ ] リソース検証機能の実装
- [ ] 設定ファイル例の更新
- [ ] トラブルシューティングガイドの更新

### トラブルシューティング時

- [ ] ログファイルの確認
- [ ] 設定ファイルの検証
- [ ] プロセス・ポート状態の確認
- [ ] 環境変数・パスの確認
- [ ] 依存関係の確認
- [ ] 解決方法の文書化
