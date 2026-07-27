# MPKG reviewer

GitHub Releases上の`.mpkg`を一時ファイルへ取得し、安全性と署名を検証してAppStore管理APIへ報告するネイティブRustツールです。パッケージは展開・インストール・実行しません。

## 実行

管理tokenは引数へ渡さず、環境変数から読み込みます。

```powershell
$env:APPSTORE_ADMIN_TOKEN='<ADMIN_TOKEN>'
$env:APPSTORE_ADMIN_ACCOUNT_ID='<監査ログ用Account UUID>'
cargo run --release --manifest-path reviewer/Cargo.toml -- <release_id>
```

ローカルAPIに対して実行する場合:

```powershell
cargo run --manifest-path reviewer/Cargo.toml -- <release_id> --api http://127.0.0.1:8787
```

成功するとReleaseは`validation_status=valid`、`review_status=submitted`になります。公開は別途、管理APIの`POST /v1/admin/releases/{release_id}/approve`で行います。

## 検証

```powershell
cargo fmt --manifest-path reviewer/Cargo.toml -- --check
cargo clippy --manifest-path reviewer/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path reviewer/Cargo.toml
```

