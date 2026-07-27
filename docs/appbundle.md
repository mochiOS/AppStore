# mochiOS MPKG仕様

## 概要

mochiOSアプリは`.mpkg`で配布します。`.mpkg`はgzip圧縮されたtarアーカイブです。AppStoreは本体を保存せず、固定したGitHub Release assetの場所、外側のSHA-256、Developer Certificate署名、審査状態だけを保持します。

## 必須構造

```text
ExampleApplication.mpkg
├─ about.toml
├─ entry.elf
├─ assets/
│  └─ icon.png
└─ META/
   ├─ manifest.toml
   └─ signature.toml
```

アーカイブには通常ファイルとディレクトリだけを格納できます。絶対パス、`..`、重複パス、シンボリックリンク、ハードリンク、デバイス、FIFOは拒否されます。

## about.toml

既存mochiOSアプリとの互換性のため、アプリ識別子のキーは`bundle_id`です。AppStore APIの`package_id`と同じ値でなければなりません。

```toml
name = "ExampleApplication"
bundle_id = "org.mochios.example"
version = "1.0.0"
developer = "Example Developer"
entry = "entry.elf"
description = "Example application"
icon = "assets/icon.png"
resources = ["assets/icon.png"]
```

`entry`は`.elf`で終わり、`entry`、`icon`、`resources`の各ファイルがパッケージ内に存在する必要があります。

## META/manifest.toml

manifestは署名対象となる正本です。`META/manifest.toml`と`META/signature.toml`を除く、すべての通常ファイルを重複なく列挙します。記載のないファイルや余分なファイルが1つでもあれば拒否されます。

```toml
format_version = 1
package_id = "org.mochios.example"
version = "1.0.0"
minimum_mochios_version = "0.1.0"

[[files]]
path = "about.toml"
size = 250
sha256 = "<about.toml raw bytesのSHA-256 hex>"

[[files]]
path = "entry.elf"
size = 123456
sha256 = "<entry.elf raw bytesのSHA-256 hex>"

[[files]]
path = "assets/icon.png"
size = 4096
sha256 = "<icon.png raw bytesのSHA-256 hex>"
```

## META/signature.toml

Developer秘密鍵は、`META/manifest.toml`のraw bytesから計算したSHA-256の32 bytesをEd25519で署名します。署名ファイルはパッケージ内へ含めます。

```toml
format_version = 1
algorithm = "ed25519"
certificate_id = "<Developer Certificate UUID>"
manifest_sha256 = "<META/manifest.toml raw bytesのSHA-256 hex>"
signature = "<Ed25519署名のBase64>"
```

外側の`.mpkg` SHA-256を埋め込み署名の対象にはしません。署名自身を含むファイルのhashを署名対象にすると自己参照になり、パッケージを生成できないためです。

## 2つの検証値

- `.mpkg` SHA-256: 審査時に取得したアーカイブと、インストール時に取得したアーカイブが同一であることを保証します。
- manifest署名: manifestがDeveloper Certificateに対応する秘密鍵で署名され、manifestに列挙された内容が改変されていないことを保証します。

GitHubに置かれていること自体は信頼の根拠にしません。クライアントはダウンロード後に外側のSHA-256、manifestの全ファイルhash、Developer Certificate署名、Certificateの有効期限・失効・Package ID scopeをfail closedで検証します。

## GitHub Releases

使用するURLは特定タグとasset名を指す固定URLだけです。

```text
https://github.com/example/texteditor/releases/download/v1.2.0/texteditor-1.2.0-x86_64.mpkg
```

`releases/latest/download/...`は使用しません。AppStore登録後にassetが削除・差し替えられた場合、外側のSHA-256が一致しないためインストールを拒否します。

## 審査時の上限

- 圧縮済み`.mpkg`: 128 MiB
- 展開後の合計: 512 MiB
- entry数: 10,000
- `about.toml`、manifest、signature: 各1 MiB

審査ツールはアーカイブをファイルシステムへ展開せず、ストリームとして走査します。
