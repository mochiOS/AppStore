# サービス境界

## Accounts

人間を表すAccount、GitHub OAuth、外部Identity、Session、Account状態、認証監査を
管理します。AppStoreはGitHub OAuth tokenやAccountsのSession DBを扱いません。

## DeveloperCA

Developer、Developer Member、追加作成申請、審査、Developer Certificate、trust
store、失効情報を管理します。CertificateはAccount IDではなくDeveloper IDへ結び付きます。

## AppStore

アプリ、Bundle ID、Release metadata、SHA-256、署名情報、公開カタログ、審査状態を管理します。`.mpkg`本体は保持せず、公開済みGitHub Release assetへmochiOSクライアントを直接案内します。

## 現在の統合状態

AppStore APIはRust版Cloudflare Workerへ移行済みです。OAuth、Account、Developer、審査、Certificate発行・失効の正本は保持しません。

Developer向けAPIはAccountsのBearer session tokenと`X-Developer-ID`を受け取り、DeveloperCAのService Bindingへ照会します。`X-Developer-ID`だけでは認証しません。Release作成時はDeveloperCAへCertificateを、Accountsへログイン中GitHubアカウントのリポジトリ権限と固定Release assetを照会します。

Accountsだけが暗号化したGitHub OAuth grantを保持します。AppStoreへtokenを返さず、確認済みmetadataだけを返します。DeveloperCAはGitHubやPackageを扱いません。

AppStoreが保持する`public_keys`は後方互換用metadataであり、DeveloperやCertificateの正本ではありません。新しい署名フローでは`certificate_id`を必須とし、審査時と公開時にDeveloperCAへ状態を再照会します。
