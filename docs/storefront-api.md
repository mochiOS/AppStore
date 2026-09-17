# ストアフロントAPI

App Storeの「見つける」画面は`GET /storefront`の応答から構成します。フロントエンドに特集文言、架空アプリ、代替画像は持たせません。

```json
{
  "featured": [
    {
      "id": "feature-id",
      "eyebrow": "任意の短いラベル",
      "title": "特集タイトル",
      "description": "任意の説明",
      "artwork": "https://cdn.example.com/feature.webp",
      "app": { "bundle_id": "org.example.app", "name": "..." }
    }
  ],
  "sections": [
    {
      "id": "section-id",
      "title": "セクション名",
      "subtitle": "任意の説明",
      "layout": "row",
      "apps": []
    }
  ],
  "categories": [
    {
      "slug": "utilities",
      "name": "ユーティリティ",
      "artwork": "https://cdn.example.com/category.webp"
    }
  ]
}
```

`sections[].layout`は次のいずれかです。

- `row`: 横スクロールのアプリ棚
- `chart`: 順位付きランキング
- `grid`: 折り返しグリッド

各`app`は最低限`bundle_id`、`name`、`version`、Developer表示名の`developer`、公開識別子の`developer_id`、`description`、`icon`を返します。必要に応じて以下も返せます。

`GET /apps/{package_id}/releases`は固定repository、tag、Asset ID、asset名、size、Asset SHA-256、Package digest、Reviewerが署名済みmanifestから検証した`architecture`／`abi`、download URLを返します。`?architecture=x86_64&abi=mochios-1`による完全一致filterも利用できます。クライアントはこのmetadataを使って候補を絞り、mochiOSがインストール時にABI compatibilityを最終判断します。OS marketing versionによる比較は行いません。内部Account ID、審査担当者ID、監査情報は公開しません。

```json
{
  "bundle_id": "org.example.app",
  "releases": [{
    "version": "1.2.0",
    "architecture": "x86_64",
    "abi": "mochios-1",
    "github_asset_id": 123456,
    "sha256": "...",
    "package_digest": "...",
    "download_url": "https://github.com/example/app/releases/download/v1.2.0/app.mpkg"
  }]
}
```

```json
{
  "subtitle": "短い説明",
  "category": "カテゴリ名",
  "kind": "app",
  "rating": 4.8,
  "rating_count": 120,
  "age_rating": "4+",
  "screenshots": ["https://cdn.example.com/screenshot.webp"],
  "download_url": "/downloads/org.example.app/latest"
}
```

Availableな無料AppのdownloadにmochiOS IDは不要です。ログイン済みで`POST /v1/apps/{package_id}/acquisitions`を呼んだ場合だけ取得履歴を保存します。Developer Unpublished／Removed後の再ダウンロードは従来どおり取得履歴を要求し、セキュリティ停止中は常に拒否します。

`kind`は`app`または`game`です。画像URLは絶対URLか、`APPSTORE_API_BASE_URL`を基準に解決できる相対URLを指定します。特集画像がない特集、空のセクション、存在しない任意項目は画面に表示されません。
