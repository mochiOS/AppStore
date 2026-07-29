# mochiOS App Store

`store.mochios.org`で公開するmochiOS向けApp Storeです。フロントエンドはNext.js + OpenNext、APIはRust + `workers-rs`で実装し、どちらもCloudflare WorkersへWranglerで公開します。

## 実装済み

- API駆動の「見つける」ストアフロント
- 特集、ランキング、横スクロール棚、カテゴリ
- アプリ／ゲーム一覧
- 公開カタログ検索
- 評価、スクリーンショットを含むアプリ詳細
- GitHub Releases上の固定`.mpkg` assetを使うRelease一覧と直接ダウンロード
- アプリアイコン画像の表示
- デスクトップ／モバイル対応
- Cloudflare Workers向けOpenNext設定

画面上のアプリ情報はモックデータを使用しません。`APPSTORE_API_BASE_URL`で指定した公開カタログAPIの応答だけを表示します。

## API設定

APIは`api/`にあります。D1にはRelease metadata、SHA-256、署名、審査状態だけを保存し、`.mpkg`本体はGitHub Releasesから配布します。Developer認証とCertificate確認にはAccounts／DeveloperCAのService Bindingを使用します。

```powershell
npm run api:check
npm run api:test
npm run api:migrate:local
npm run api:dev
```

別ターミナルでフロントエンドを起動します。

```powershell
$env:APPSTORE_API_BASE_URL='http://127.0.0.1:8787/v1'
npm run dev
```

APIが未設定、到達できない、または公開アプリが0件の場合は、画面確認用の`ExampleApplication`を1件だけ表示します。実データが1件でも存在すれば表示しません。

期待するAPI:

```text
GET /apps
GET /apps/{bundle_id}
GET /search?q={query}
GET /storefront
```

アプリアイコンはAPIレスポンスの`icon`で指定します。フロントエンド側で絵文字や生成画像へ置き換えません。
特集やストア棚も固定文言を置かず、`/storefront`の応答だけで構成します。詳しい応答形式は[docs/storefront-api.md](docs/storefront-api.md)を参照してください。

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
