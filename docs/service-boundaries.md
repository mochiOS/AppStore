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

## 現在の統合状態

AppStore APIはRust版Cloudflare Workerへ移行済みです。OAuth、Account、Developer、審査、Certificate発行・失効の正本は保持しません。

Developer向けAPIはAccountsのBearer session tokenと`X-Developer-ID`を受け取り、DeveloperCAのService Bindingへ照会します。`X-Developer-ID`だけでは認証しません。Release作成時もDeveloperCAへCertificateを照会し、activeかつ対象Developerに属することを確認します。

AppStoreが保持する`public_keys`はPackage形式との後方互換用metadataであり、DeveloperやCertificateの正本ではありません。新しい署名フローでは`certificate_id`を必須とします。
