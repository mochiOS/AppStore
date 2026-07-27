# Rust APIへの移行

旧PHP APIの責務は`api/`のCloudflare Workerへ移しました。

| 旧構成 | Rust版 |
|---|---|
| PHP Router | `workers-rs` Router |
| PDO + SQLite | D1 binding + prepared statement |
| ローカルPackageStorage | R2 binding |
| PHP session | Accounts Bearer token + DeveloperCA Service Binding |
| 一括multipart upload | JSON Release作成 + R2 streaming PUT |
| PHP CLIによる審査 | 管理API + `ADMIN_TOKEN` |

Packageを一括展開する旧`PackageInspectService`は移植していません。Workersで大容量tar.gzをメモリ展開すると安全な上限を維持できないためです。Rust版ではクライアントが算出したPackage SHA-256をR2のchecksum機能で照合し、そのhashへのEd25519署名をDeveloperCAが発行したactive Certificateの公開鍵で検証します。Package内容の詳細検査を追加する場合は、R2イベントからQueue/Workflowへ渡す非同期検査Workerとして実装してください。

旧SQLiteデータは自動移行されません。D1へ取り込む場合は旧DBをJSONまたはSQLへexportし、新schemaへ変換したうえで`wrangler d1 execute --remote --file`を使用します。Packageファイルは対応する`package_key`へR2へ移してください。
