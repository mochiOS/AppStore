# mochiOS AppStore

mochiOS向けアプリの登録、Package upload、Release審査、公開カタログ、Package配布を
担当するサービスです。現在の実装はPHPとSQLiteで動作します。

## サービス分離

認証とAccount状態は`Accounts`、Developer／Member／審査／Certificate／失効は
`DeveloperCA`へ移管しました。AppStoreから次の旧実装を削除しています。

- GitHub OAuthとOAuth Identity保存
- OAuthログイン時のDeveloper自動作成
- Developer審査
- CSR受付、CA署名、Certificate発行・失効
- Root CA秘密鍵設定とCA API

AppStoreはDeveloper IDをアプリの所有者IDとして保持しますが、Developer自体の
正本は保持しません。現在残っているPHP sessionと公開鍵registryは、AppStore固有の
管理画面・Package署名フローを維持するための暫定境界です。Accounts／DeveloperCAとの
本統合後に置換します。詳細は[サービス境界](docs/service-boundaries.md)を参照してください。

## 主な機能

- 公開アプリカタログ、検索、Package download
- Bundle ID予約とアプリ登録
- `.pkg` upload、検査、署名検証
- Release作成、提出、審査、公開
- AppStore内チームと管理者によるRelease管理

## ローカル実行

`config/app.example.php`を参考に`config/app.php`を作成し、migrationを実行します。

```sh
php src/cli/migrate.php
php -S localhost:3001 src/api/router.php
```

テスト:

```sh
php src/tests/run.php
```

Package形式は[アプリバンドル仕様](docs/appbundle.md)、APIは
[`src/api/v1/list.yaml`](src/api/v1/list.yaml)を参照してください。
