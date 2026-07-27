# mochiOS App Store

`store.mochios.org` で公開するmochiOS向けApp Storeです。フロントエンドはNext.js、Cloudflare上の実行環境はOpenNextとWorkersを使用します。

## 実装済み

- API駆動の「見つける」ストアフロント
- 特集、ランキング、横スクロール棚、カテゴリ
- アプリ／ゲーム一覧
- 公開カタログ検索
- 評価、スクリーンショットを含むアプリ詳細
- Release一覧とダウンロード
- アプリアイコン画像の表示
- デスクトップ／モバイル対応
- Cloudflare Workers向けOpenNext設定

画面上のアプリ情報はモックデータを使用しません。`APPSTORE_API_BASE_URL`で指定した公開カタログAPIの応答だけを表示します。

## API設定

開発時は公開カタログAPIの`/v1`を指定します。

```powershell
$env:APPSTORE_API_BASE_URL='http://127.0.0.1:8080/v1'
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
```

## 公開

```powershell
npm run deploy
```

Custom Domainは`store.mochios.org`です。
