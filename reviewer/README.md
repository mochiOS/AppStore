# MPKG Reviewer

GitHub Releases上の`.mpkg`を一時ファイルへ取得し、展開・実行せずに検証してAppStore管理APIへ報告するネイティブRustツールです。

## 実行

```powershell
$env:APPSTORE_REVIEWER_TOKEN='<AppStore API REVIEWER_TOKEN>'
cargo run --release --manifest-path reviewer/Cargo.toml -- <release_id>
```

ローカルAPI:

```powershell
cargo run --manifest-path reviewer/Cargo.toml -- <release_id> --api http://127.0.0.1:8787
```

ReviewerはRelease登録時にDeveloper CAが検証して固定したCertificate identityをAppStore APIから取得します。MPKG内MCERについて次を照合します。

- Certificate IDに対応するserial
- Subject public keyとSubject Key ID
- Certificate Developer ID（AppStore Releaseと同じ32桁UUIDv7本体）
- Issuer public keyとIssuer Key ID
- MCER署名、有効期間、package signing usage、Package ID完全一致scope
- 全`[[binary]].requires`がallowed Capability内
- `manifest.sig`
- payload size／SHA-256、未知payload拒否
- MPKG v1 header、無圧縮ustar、entry type、重複・path traversal・未知signature拒否

Issuer公開鍵はDeveloper CAがOffline Root署名Trust Snapshotまたはlegacy Root経路で検証した値です。Reviewerはその公開鍵でMCER署名を再検証し、AppStore APIはreport受理時と公開承認時にDeveloper CA statusを再照会します。未登録、不一致、失効、期限切れCertificateは拒否されます。

Package IDは`org.mochios.*`へ限定せず、共有`mochios-certificate` validatorで2 segment以上の小文字reverse-domain形式を検証します。

成功後、Releaseは`validation_status=valid`、`review_status=submitted`になります。審査担当者は[Console](https://console.mochios.org/#reviews)で最終承認または却下します。

Reviewer専用tokenはConsoleの`ADMIN_TOKEN`と分離します。検証reportはRelease ID、GitHub Asset ID、Asset SHA-256、Package digest、Reviewer version、検証時刻へ拘束され、別Releaseへ再利用できません。失敗時も固定error codeと短いsummaryだけを保存し、MPKG本体やraw payloadは保存しません。

## 検証

```powershell
cargo fmt --manifest-path reviewer/Cargo.toml -- --check
cargo test --manifest-path reviewer/Cargo.toml
cargo clippy --manifest-path reviewer/Cargo.toml --all-targets -- -D warnings
```
