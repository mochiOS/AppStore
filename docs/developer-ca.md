# mochiOS Developer CA

## 概要

1. GitHub OAuthでログインした開発者を`developer_id`へ正規化する
2. 開発者審査の状態をサーバー側で管理する
3. 審査済み開発者からCSRを受け取る
4. サーバーが保持するCA秘密鍵でコード署名用証明書を発行する
5. 証明書の失効状態を管理する

mochiOS側は以下を行う想定である。

1. `/v1/ca/root`でルートCA証明書を取得する
2. リリースmetadataに含まれる証明書と署名を検証する
3. 失効済み証明書や停止済み開発者を拒否する

## 信頼モデル

- GitHub OAuthはログイン手段であり、署名の信頼根ではない
- サーバーCAが「この公開鍵はこの開発者に発行した」と署名する
- mochiOSはサーバーCAをtrust anchorとして扱う

## 追加テーブル

### `developer_verifications`

開発者の審査状態を保持する。

- `pending`
- `verified`
- `rejected`

### `certificate_signing_requests`

開発者が提出したCSRを保持する。

- CSR本文
- CSRから抽出した公開鍵
- 公開鍵fingerprint
- subjectDN
- 審査状態

### `developer_certificates`

サーバーCAが発行した証明書を保持する。

- `certificate_pem`
- `serial_number`
- `public_key_fingerprint`
- `issued_at`
- `expires_at`
- `status`
- `revoked_at`

### `revocations`

既存の失効テーブルを利用し、証明書失効時に`target_type = certificate`を記録する。

## 設定

`config/app.php`に以下の共通設定を置く。

```php
'admin_api_token' => '',
'ca_cert_path' => '',
'ca_key_path' => '',
'ca_key_passphrase' => '',
'ca_certificate_days' => 365,
```

本番では実値を設定する。秘密値をリポジトリへコミットしてはいけない。

テストやローカル起動では以下の環境変数でも上書きできる。

- `APPSTORE_ADMIN_API_TOKEN`
- `APPSTORE_CA_CERT_PATH`
- `APPSTORE_CA_KEY_PATH`
- `APPSTORE_CA_KEY_PASSPHRASE`
- `APPSTORE_CA_CERTIFICATE_DAYS`

## API

### 開発者向け

#### `GET /v1/developer-verifications/me`

自分の審査状態を返す。

#### `POST /v1/developer-verifications/request`

審査申請を作成する。

```json
{
  "note": "please verify me"
}
```

#### `GET /v1/certificate-requests`

自分の CSR 一覧を返す。

#### `POST /v1/certificate-requests`

審査済み開発者のみ CSR を提出できる。

```json
{
  "csr_pem": "-----BEGIN CERTIFICATE REQUEST-----\n..."
}
```

#### `GET /v1/certificates`

自分に発行された証明書一覧を返す。

#### `GET /v1/certificates/{certificate_id}`

自分に発行された証明書詳細を返す。

#### `GET /v1/ca/root`

公開用のルート CA 証明書を返す。

## 管理 API

管理 API は `X-Admin-Token` ヘッダが必要。

#### `POST /v1/admin/developers/{developer_id}/verification`

審査状態を更新する。

```json
{
  "verification_status": "verified",
  "note": "approved"
}
```

#### `POST /v1/admin/certificate-requests/{csr_id}/issue`

CSR を CA で署名し、開発者証明書を発行する。

#### `POST /v1/admin/certificate-requests/{csr_id}/reject`

CSR を却下する。

```json
{
  "reason": "missing verification"
}
```

#### `POST /v1/admin/certificates/{certificate_id}/revoke`

証明書を失効させる。

```json
{
  "reason": "compromised"
}
```

## 証明書発行フロー

1. 開発者が GitHub OAuth でログインする
2. 開発者が `POST /v1/developer-verifications/request` を実行する
3. 管理者が `POST /v1/admin/developers/{developer_id}/verification` で `verified` にする
4. 開発者がローカルで鍵ペアを生成する
5. 開発者が CSR を `POST /v1/certificate-requests` で提出する
6. 管理者が `POST /v1/admin/certificate-requests/{csr_id}/issue` を実行する
7. サーバーが CA 秘密鍵で CSR を署名する
8. 開発者は発行済み証明書を使って release manifest に署名する

## 失効フロー

1. 管理者が `POST /v1/admin/certificates/{certificate_id}/revoke` を実行する
2. `developer_certificates.status` を `revoked` に更新する
3. `revocations` に履歴を残す
4. mochiOS は失効済み証明書を拒否する

## 今回の責務に含めていないもの

- mochiOS 側の証明書チェーン検証
- release manifest の canonicalization
- package 署名の実施
- CRL/OCSP のようなオンライン失効配布

これらは OS 側または release API 拡張で扱う。
