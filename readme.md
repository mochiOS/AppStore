# mochiOS App Store

`store.mochios.org`で公開するmochiOS向けApp Storeです。フロントエンドはNext.js + OpenNext、APIはRust + `workers-rs`で実装し、どちらもCloudflare WorkersへWranglerで公開します。

## 現在の公開状態

ストアフロントはメンテナンスモードです。`store.mochios.org`配下のすべての画面をメンテナンス案内へ内部的に書き換え、既存のカタログAPIを呼び出しません。検索エンジンにはインデックスしないよう応答します。

既存のカタログAPIは削除していません。Developer Consoleと審査APIの整備を先行し、利用者向けストアフロントの実装再開まではメンテナンス表示を維持します。

## 現在実装している管理基盤

- App、Build、Submission、Review、公開状態を分離したD1モデル
- GitHub Releases上の固定`.mpkg` assetを使うBuild登録と直接ダウンロード
- 審査待ちReleaseを排他的に取得する自動MPKG Reviewer Queue
- DraftからRejectedまでのSubmissionワークフロー
- Store Listing、Capability、通信先、Privacy、課金、Content、動的コード、AI、テスト情報の申告
- 512×512 PNG／JPEGアイコンの実体検査と、3枚以上のスクリーンショット検査
- Available／Developer Unpublished／Removedを審査状態から分離した公開管理
- 回数制限のないAppeal、append-only Review／公開履歴
- 1 Appにつき1つのcurrent Developer Certificateと、失効後の安全な置換
- Developer／運営者向け通知、Account単位の未読管理

利用者向けストアフロントは現在データを読み込まず、モックも表示しません。

## API設定

APIは`api/`にあります。D1にはRelease metadata、SHA-256、署名、審査状態だけを保存し、`.mpkg`本体はGitHub Releasesから配布します。Developer認証とCertificate確認にはAccounts／DeveloperCAのService Bindingを使用します。

```powershell
npm run api:check
npm run api:test
npm run api:migrate:local
npm run api:dev
```

別ターミナルでフロントエンドを起動すると、メンテナンス画面を確認できます。

```powershell
$env:APPSTORE_API_BASE_URL='http://127.0.0.1:8787/v1'
npm run dev
```

主な公開API:

```text
GET /apps
GET /apps/{bundle_id}
GET /apps/{bundle_id}/status
POST /apps/{bundle_id}/acquisitions
GET /apps/{bundle_id}/download
GET /search?q={query}
GET /storefront
```

アプリアイコンはAPIレスポンスの`icon`で指定します。フロントエンド側で絵文字や生成画像へ置き換えません。
利用者向けストアフロントを再開するまでは、これらのAPIを画面から呼び出しません。

## ローカル開発

```powershell
npm install
npm run dev
```

Cloudflare Workersと同じ実行環境で確認する場合:

```powershell
npm run preview
```

## 検証

```powershell
npm run lint
npx tsc --noEmit
npm run build
npx opennextjs-cloudflare build
npx wrangler deploy --dry-run
npm run reviewer:check
npm run reviewer:test
```

本番Reviewer RunnerはGitHub Actionsが1時間ごとに最大1件処理します。AppStoreリポジトリのRepository Secretに、AppStore APIの`REVIEWER_TOKEN`と同じ値を設定してください。

```text
APPSTORE_REVIEWER_TOKEN
```

手動実行はGitHubの`Actions` → `MPKG Reviewer` → `Run workflow`から行えます。ローカルで常駐させる場合は次を実行します。

```powershell
$env:APPSTORE_REVIEWER_TOKEN='<AppStore API REVIEWER_TOKEN>'
cargo run --release --manifest-path reviewer/Cargo.toml -- --queue
```

MPKG形式と署名対象は[docs/appbundle.md](docs/appbundle.md)、審査ツールは[reviewer/README.md](reviewer/README.md)を参照してください。

## 公開

```powershell
npx wrangler secret put ADMIN_TOKEN --config api/wrangler.jsonc
npx wrangler secret put REVIEWER_TOKEN --config api/wrangler.jsonc
npx wrangler secret put APPSTORE_SERVICE_TOKEN --config api/wrangler.jsonc
npm run api:migrate:remote
npm run api:deploy
npm run deploy
```

Custom Domainはフロントエンドが`store.mochios.org`、APIが`api.store.mochios.org`です。詳細は[api/README.md](api/README.md)を参照してください。
