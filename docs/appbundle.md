# mochiOS MPKG v1

正本はmochiOSリポジトリの`docs/mpkg.md`、`docs/packages.md`、`docs/certificates.md`です。

## コンテナ

`.mpkg`は32-byte headerの直後に非圧縮ustar streamを連結した形式です。gzipではありません。

```text
offset  size  内容
0       4     "MPKG"
4       2     major = 1 (u16 LE)
6       2     minor = 0 (u16 LE)
8       2     header size = 32 (u16 LE)
10      1     compression = 0
11      1     flags = 0
12      8     tar stream length (u64 LE)
20      12    reserved = all zero
```

## 必須entry

```text
manifest.toml
signatures/manifest.sig
signatures/developer.cert
payload/root/...
# または payload/bundle/...
```

`manifest.sig`はraw 64-byte Ed25519署名、`developer.cert`はraw MCER v1です。`signatures/chain/`、未知のsignature entry、絶対path、`..`、重複path、link、device、FIFOは拒否します。

## Manifest

Manifestは`format = 1`、`[package]`、`[[file]]`、`[[binary]]`で構成します。各`[[file]]`はpayload fileのID、path、size、SHA-256、modeを宣言し、各`[[binary]]`は対象file IDと`requires` Capabilityを宣言します。未宣言payload、存在しないfile ID、hash／size不一致は拒否します。

署名メッセージは次のとおりです。

```text
"mochios-mpkg-manifest-v1\0" || SHA256(manifest.toml raw bytes)
```

ReviewerはMCER v1を組み込みRoot公開鍵で検証し、Package ID scopeと全`binary.requires`が証明書の許可範囲内であることを確認します。さらに登録時のCertificate serialとsubject公開鍵が、MPKG内の証明書と一致することを確認します。

AppStoreはMPKG本体を保存せず、固定GitHub Release asset URL、外側のSHA-256、Certificate情報、審査状態だけを保持します。`releases/latest/download/...`は使いません。
