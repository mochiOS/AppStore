# サービス境界

## Accounts

人間を表すAccount、GitHub OAuth、外部Identity、Session、Account状態、認証監査を
管理します。AppStoreはGitHub OAuth tokenやAccountsのSession DBを扱いません。

## DeveloperCA

Developer、Developer Member、追加作成申請、審査、Developer Certificate、trust
store、失効情報を管理します。CertificateはAccount IDではなくDeveloper IDへ結び付きます。

## AppStore

アプリ、Bundle ID、Package、Release、公開カタログ、Release審査を管理します。
Package本体とAppStore固有metadataはAccountsやDeveloperCAへ移しません。

## 現在の移行状態

旧OAuth、Developer作成、審査、CSR、CA発行・失効コードはAppStoreから削除済みです。
`developers`表はAppStoreデータが参照する外部Developer IDの投影として残しています。
`public_keys`とPHP session guardは既存Package署名・管理画面を維持する暫定実装です。

Accounts／DeveloperCAとのAppStore統合はまだ未実装です。統合時には次を行います。

1. Accounts Sessionを安全なbackend間フローでintrospectionする
2. DeveloperCAでmembershipとroleを確認する
3. Package署名をDeveloper Certificate、scope、Capability、失効状態で検証する
4. AppStore内の暫定公開鍵registryとPHP sessionを削除する

統合が完了するまで、Developer向けAPIを認証なしで公開したり、信頼できない
`X-Developer-ID`ヘッダーを認証として採用してはいけません。

