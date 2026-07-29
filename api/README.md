# App Store API

Cloudflare Workers上で動作するRust／`workers-rs`版APIです。D1、Accounts Service Binding、Developer CA Service Bindingを使用し、アプリ本体は保存しません。

## Release登録

```http
POST /v1/developer/apps/{package_id}/releases
Authorization: Bearer <Accounts tokenまたはConsole delegation token>
X-Developer-ID: <32桁小文字UUIDv7 Developer ID>
Content-Type: application/json

{
  "version": "1.2.0",
  "repository": "example/texteditor",
  "release_tag": "v1.2.0",
  "asset": "texteditor-1.2.0-x86_64.mpkg",
  "certificate_id": "<Developer Certificate ID>",
  "minimum_mochios_version": "0.1.0",
  "changelog": "変更内容"
}
```

DeveloperCAがtokenからAccountとactive・verified Developer Memberを確定し、roleが`owner`／`admin`／`developer`の場合だけ許可します。AccountsはそのAccountの保存済みGitHub grantで`push`／`maintain`／`admin`権限、公開済み固定tag、完全一致`.mpkg` assetを確認します。`viewer`、request内Account ID、`latest`は拒否します。

Developer CAのstatusが有効で内部Developer IDと一致するときだけ、serial、Subject／Issuer key identity、Certificate Developer ID、発行経路をReleaseへ固定します。Reviewer reportはこれらすべてと一致しなければ受理しません。report受理時と公開承認時にもstatusを再確認します。

Package IDは`org.mochios.*`へ限定しません。`com.example.paint`、`io.github.user.tool`、`dev.tas0.volume`のような2 segment以上の小文字reverse-domain形式を共有Certificate crateで検証します。

## 検証状態

```text
登録: validation=pending, review=pending, publish=draft
Reviewer成功: validation=valid, review=submitted, publish=draft
人手承認: validation=valid, review=approved, publish=published
Certificate／Asset不整合: validation=invalid, review=rejected, publish=revoked
```

公開クライアントはAppStore APIから固定SHA-256とdownload URLを取得し、GitHub Releasesから直接MPKGを取得します。

## ローカル確認

```powershell
cd api
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
npx wrangler d1 migrations apply mochios-app-store --local
npx wrangler dev
```

## 本番

```powershell
npx wrangler secret put APPSTORE_SERVICE_TOKEN
npx wrangler secret put ADMIN_TOKEN
npx wrangler secret put REVIEWER_TOKEN
npx wrangler d1 migrations apply mochios-app-store --remote
npx wrangler deploy
```

`APPSTORE_SERVICE_TOKEN`はAccounts内部API用、`ADMIN_TOKEN`はConsole審査BFF用、`REVIEWER_TOKEN`はMPKG Reviewer専用です。Developer秘密鍵、Offline Root秘密鍵、Online Intermediate秘密鍵はAppStoreへ設定しません。
公開済みパッケージは管理APIから一時停止／再開できます。停止中はストア一覧、詳細、Release一覧、ダウンロードから除外され、新しいReleaseも登録できません。Releaseの失効とは別の可逆なインシデント対応です。

6時間ごとのscheduled integrity checkは公開ReleaseのDeveloperCA statusとGitHub repository／Release／Asset identityを再確認します。不整合や到達不能はfail closedで新規downloadを停止します。管理APIから即時整合性確認と再検証要求も行えます。
