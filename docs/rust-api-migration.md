# Rust APIへの移行

旧PHP APIの責務は`api/`の`workers-rs` Workerへ移行済みです。Package保存は廃止し、GitHub Releasesを配布元に変更しました。

| 旧構成 | 現在の構成 |
|---|---|
| PHP Router | `workers-rs` Router |
| PDO + SQLite | D1 + prepared statement |
| AppStore内の認証 | Accounts Bearer token |
| AppStore内のDeveloper／証明書 | DeveloperCA Service Binding |
| R2へのPackage upload | GitHub Release assetの固定metadata |
| R2からの配布 | mochiOSクライアントからGitHubへ直接接続 |
| PHP Package審査 | ネイティブRust MPKG reviewer |

旧R2 Releaseはmigration時に`invalid`、`rejected`、`revoked`へ変更され、公開されません。GitHub Releaseとして再登録し、再審査してください。

AppStoreはGitHub OAuth tokenを保持しません。登録時のリポジトリ権限とasset確認はAccountsの内部APIへ委譲し、Accountsが暗号化保管したOAuth grantを必要なときだけ復号します。

審査ツールは`.mpkg`を一時ファイルへ取得しますが、AppStore WorkerやD1には保存しません。検証後にD1へ保存するのはSHA-256、manifest hash、署名、Certificate ID、審査・公開状態だけです。
