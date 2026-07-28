# MPKG reviewer

GitHub Releases上の`.mpkg`を一時ファイルへ取得し、安全性と署名を検証してAppStore管理APIへ報告するネイティブRustツールです。パッケージは展開・インストール・実行しません。

## 実行

管理tokenは引数へ渡さず、環境変数から読み込みます。

```powershell
$env:APPSTORE_ADMIN_TOKEN='<ADMIN_TOKEN>'
$env:APPSTORE_ADMIN_ACCOUNT_ID='<監査ログ用Account UUID>'
$env:MOCHIOS_ROOT_PUBLIC_KEYS_HEX='<Root公開鍵hex。複数の場合はカンマ区切り>'
cargo run --release --manifest-path reviewer/Cargo.toml -- <release_id>
```

ローカルAPIに対して実行する場合:

```powershell
cargo run --manifest-path reviewer/Cargo.toml -- <release_id> --api http://127.0.0.1:8787
```

成功するとReleaseは`validation_status=valid`、`review_status=submitted`になります。その後、審査担当者が[mochiOS Console](https://console.mochios.org/#reviews)でmetadata、hash、署名情報、アプリ内容を確認し、承認または却下します。`ADMIN_TOKEN`をブラウザーへ渡したり、管理APIをブラウザーから直接呼び出したりしません。

ReviewerはMPKG v1 header、非圧縮ustar、manifestの全payload、raw MCER v1、Root直署名、Certificate serial／subject公開鍵、manifest署名、Package ID scope、Capabilityを検証します。

## 検証

```powershell
cargo fmt --manifest-path reviewer/Cargo.toml -- --check
cargo clippy --manifest-path reviewer/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path reviewer/Cargo.toml
```
