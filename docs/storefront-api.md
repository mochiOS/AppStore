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

各`app`は最低限`bundle_id`、`name`、`version`、`developer`、`description`、`icon`を返します。必要に応じて以下も返せます。

```json
{
  "subtitle": "短い説明",
  "category": "カテゴリ名",
  "kind": "app",
  "price_label": "入手",
  "rating": 4.8,
  "rating_count": 120,
  "age_rating": "4+",
  "screenshots": ["https://cdn.example.com/screenshot.webp"],
  "download_url": "/downloads/org.example.app/latest"
}
```

`kind`は`app`または`game`です。画像URLは絶対URLか、`APPSTORE_API_BASE_URL`を基準に解決できる相対URLを指定します。特集画像がない特集、空のセクション、存在しない任意項目は画面に表示されません。
