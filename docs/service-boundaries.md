# サービス境界

## Accounts

Account、GitHub OAuth、Session、Account状態、GitHub repository権限を管理します。AppStoreへOAuth tokenを返さず、確認済みRelease asset metadataだけを返します。

## Developer CA

Developer、Member、Certificate、Online Intermediate、Root署名Trust Snapshot、Issuer Registry、失効状態の正本です。AppStoreはCertificateを発行・失効しません。

Release登録時、AppStore APIはDeveloper CA statusから次を固定保存します。

```text
certificate_id
serial
subject public key
subject key ID
certificate developer ID
issuer public key
issuer key ID
issuance source
Developer ID（MCERと同じ32桁UUIDv7本体）
```

## AppStore

App、Bundle ID、GitHub Release固定metadata、SHA-256、Certificate identity、審査・公開状態をD1へ保存します。`.mpkg`本体、Developer秘密鍵、GitHub OAuth tokenは保存しません。

ReviewerがMPKG内MCERと保存済みidentityを暗号学的に照合します。Reviewer専用tokenはConsole管理tokenと分離され、reportはRelease ID、Asset ID、Asset SHA-256、Package digest、Reviewer version、検証時刻へ拘束されます。AppStore APIはreport受理時、公開承認直前、公開後の定期整合性確認でDeveloper CA statusへ再照会し、不整合をfail closedで拒否または公開停止します。

DeveloperCAのDeveloper ID、MCERの`developer_id`、Releaseの`registered_by`は同じ32桁UUIDv7本体でなければなりません。Package IDは特定namespaceへ限定せず、共有validatorの規則へ従います。

既存Root直署名CertificateはDeveloper CAの`legacy_root`検証を通じて継続利用できます。新規一般発行の`online_intermediate`も同じReviewer経路を通り、特別なbypassはありません。

公開後にCertificateまたはGitHub assetの不整合を検出したReleaseは`invalid/rejected/revoked`へ遷移し、Store一覧とdownloadから外れます。再公開には現在のassetを再取得するReviewer検証とConsole再審査が必要です。既存インストール済みアプリの扱いはOS側policyへ委ねます。
