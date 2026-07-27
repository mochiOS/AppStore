# App Store API

Cloudflare Workers上で動作するRust版App Store APIです。`workers-rs`、D1、Accounts／DeveloperCAへのService Bindingを使用します。アプリ本体は保存しません。

## 配布構成

- D1 `DB`: アプリ、GitHub Releaseの固定metadata、SHA-256、署名、審査・公開状態
- Service Binding `ACCOUNTS`: GitHub OAuth tokenを使ったリポジトリ所有権とRelease assetの確認
- Service Binding `DEVELOPER_CA`: Developer所属、Certificate状態、公開鍵の確認
- Secret `APPSTORE_SERVICE_TOKEN`: AppStoreからAccounts内部APIへの認証
- Secret `ADMIN_TOKEN`: MPKG検証・Release審査APIの認証

`.mpkg`本体、GitHub OAuth token、Developer秘密鍵はAppStoreへ保存しません。

## ローカル実行

```powershell
cd api
worker-build --release --no-panic-recovery
npx wrangler d1 migrations apply mochios-app-store --local
npx wrangler dev
```

Developer APIは次のヘッダーを要求します。

```text
Authorization: Bearer <Accounts session token>
X-Developer-ID: <Developer UUID>
```

管理APIは次のヘッダーを要求します。

```text
X-Admin-Token: <ADMIN_TOKEN>
X-Admin-Account-ID: <監査ログへ記録するAccount UUID>
```

## GitHub Releaseの登録

```http
POST /v1/developer/apps/{package_id}/releases
Authorization: Bearer <Accounts session token>
X-Developer-ID: <Developer UUID>
Content-Type: application/json

{
  "version": "1.2.0",
  "repository": "example/texteditor",
  "release_tag": "v1.2.0",
  "asset": "texteditor-1.2.0-x86_64.mpkg",
  "certificate_id": "<Developer Certificate UUID>",
  "minimum_mochios_version": "0.1.0",
  "changelog": "変更内容"
}
```

Accountsがログイン中のGitHubアカウントに`push`、`maintain`または`admin`権限があることを確認します。公開リポジトリ、公開済みRelease、完全一致するタグと`.mpkg` assetだけを受理します。`latest`や`releases/latest/download`は使用できません。

登録直後は`validation_status=pending`、`review_status=pending`、`publish_status=draft`です。Rust審査ツールがGitHubから一時ファイルへ直接取得し、形式・全ファイルhash・Developer署名を検証した後、管理APIへ結果を送信します。承認されるまで公開APIには現れません。

## 公開

初回のみ、Accountsと同じ値のService tokenを登録します。値をコマンドライン引数や設定ファイルへ書かないでください。

```powershell
npx wrangler secret put APPSTORE_SERVICE_TOKEN
npx wrangler secret put ADMIN_TOKEN
npx wrangler d1 migrations apply mochios-app-store --remote
npx wrangler deploy
```

Custom Domainは`api.store.mochios.org`です。
