# App Store API

Cloudflare Workers上で動作するRust版App Store APIです。`workers-rs`、D1、R2、DeveloperCA Service Bindingを使用します。

## 構成

- D1 `DB`: Bundle ID、アプリ、Release、鍵、チーム、監査ログ
- R2 `PACKAGES`: `.pkg`本体
- Service Binding `DEVELOPER_CA`: Developer所属とCertificateの検証
- Secret `ADMIN_TOKEN`: Release審査API専用

`ADMIN_TOKEN`は十分に長いランダム値を設定し、ソース、Wrangler設定、ログへ記録しないでください。

```powershell
npx wrangler secret put ADMIN_TOKEN --config api/wrangler.jsonc
```

## ローカル実行

```powershell
cd api
worker-build --release --no-panic-recovery
npx wrangler d1 migrations apply mochios-app-store --local
npx wrangler dev
```

Developer向けAPIは次のヘッダーを要求します。

```text
Authorization: Bearer <Accounts session token>
X-Developer-ID: <Developer UUID>
```

APIはBearer tokenを保存せず、DeveloperCAのService Bindingへ転送してDeveloperへのアクセス権と状態を確認します。

管理APIは次のヘッダーを要求します。

```text
X-Admin-Token: <ADMIN_TOKEN>
X-Admin-Account-ID: <監査ログへ記録するAccount UUID>
```

## Packageアップロード

WorkersのメモリへPackage全体を読み込まないため、アップロードは二段階です。

1. `POST /v1/developer/apps/{bundle_id}/releases`へversion、size、SHA-256、signature、certificate IDをJSONで登録
2. 応答の`package_upload_url`へ`.pkg`を`PUT`
3. `POST /v1/developer/releases/{release_id}/submit`で審査へ提出

`signature`は`package_sha256`の32 bytesに対するEd25519署名をBase64またはBase64URLで指定します。APIはDeveloperCAのCertificateに含まれる公開鍵で検証します。

`PUT`では`Content-Length`が必須です。bodyはR2へストリーミングされ、申告したSHA-256をR2が照合します。上限は128 MiBです。

## 公開

```powershell
npx wrangler d1 migrations apply mochios-app-store --remote
npx wrangler deploy
```

Custom Domainは`api.store.mochios.org`です。
