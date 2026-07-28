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
internal Developer record ID
```

## AppStore

App、Bundle ID、GitHub Release固定metadata、SHA-256、Certificate identity、審査・公開状態をD1へ保存します。`.mpkg`本体、Developer秘密鍵、GitHub OAuth tokenは保存しません。

ReviewerがMPKG内MCERと保存済みidentityを暗号学的に照合します。AppStore APIはReviewer report受理時と公開承認直前にDeveloper CA statusへ再照会し、失効、Developer停止、Issuer失効、metadata変化をfail closedで拒否します。

既存Root直署名CertificateはDeveloper CAの`legacy_root`検証を通じて継続利用できます。新規一般発行の`online_intermediate`も同じReviewer経路を通り、特別なbypassはありません。
