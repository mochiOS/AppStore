# mochiOS アプリパッケージの仕様

## 概要

mochiOSではアプリケーションを`.pkg`形式で配布する。

`.pkg`はgzip圧縮されたtarアーカイブ（tar.gz）であり、インストール時にシステムによって展開される。

インストール後のアプリケーションは`.app`ディレクトリとして`/applications`配下に配置される。

## パッケージ形式

#### 拡張子
.pkg

#### 圧縮形式
tar.gz

#### パッケージ構造

パッケージ内にはアプリケーションに必要なファイルを格納する。

例:

```txt
Binder.pkg
├─ about.toml
├─ entry.elf
├─ assets/
│  └─ icon.png
└─ ...
```

パッケージのルートには最低限以下のファイルが存在しなければならない。

```txt
about.toml
{entry}.elf
```

## インストール後の構造

インストール後は以下のような構造となる。

```txt
/applications/
└─ Binder.app/
   ├─ about.toml
   ├─ entry.elf
   ├─ assets/
   │  └─ icon.png
   └─ ...
```

`.app` ディレクトリはアプリケーション単位で管理される。

## about.toml

`about.toml` はアプリケーションのメタデータを定義するファイルである。

## 必須項目

| キー | 型 | 説明 |
|--------|--------|--------|
| name | string | アプリ名 |
| bundle_id | string | アプリを識別する一意なID |
| version | string | バージョン |
| developer | string | 開発者名 |
| entry | string | 起動するELFファイル |
| description | string | アプリ説明 |
| icon | string | アイコン画像 |

## 任意項目

| キー | 型 | 説明 |
|--------|--------|--------|
| resources | array<string> | リソース一覧 |

## 記述例

```toml
name = "Binder"
bundle_id = "com.mochi.binder"
version = "0.1.0"
developer = "tas0dev"
entry = "entry.elf"
description = "Binder is a Finder-like file manager app"
icon = "assets/icon.png"

resources = [
    "assets/icon.png"
]
```

## bundle_id

`bundle_id`はシステム内で一意でなければならない。

推奨形式:

```txt
com.company.application
```

例:

```txt
com.mochi.binder
com.mochi.settings
dev.taso.editor
org.example.game
```

## アプリ起動

アプリケーション起動時の処理は以下の通り。

1. about.toml を読み込む
2. entryを取得する
3. ELFをロードする
4. プロセスを生成する
5. 実行開始

## インストール処理

パッケージインストール時は以下を行う。

1. .pkg を展開
2. about.tomlを読み込む
3. 必須項目を検証
4. entry.elfの存在確認
5. entry.elfを署名確認
6. /applications/<name>.appを作成
7. ファイルをコピー
8. アプリ一覧を更新

## アップデート

既に同じ `bundle_id` のアプリが存在する場合はアップデートとして扱う。

旧:
com.mochi.binder
version 0.1.0

新:
com.mochi.binder
version 0.2.0

既存のアプリを新しい内容へ置き換える。

## アンインストール

アンインストール時は対象の `.app` ディレクトリを削除し、lib/AppService/以下の対応するディレクトリを削除する

## セキュリティ

パッケージ展開時には以下を禁止する。

- ディレクトリトラバーサル
- シンボリックリンク
- ハードリンク
- デバイスファイル

## 署名検証

mochiOS はインストール時に以下を検証する想定である。

1. パッケージの `sha256` を再計算する
2. release metadata に含まれる署名を検証する
3. 発行元証明書がストアの CA によって署名されているか確認する
4. `bundle_id` が証明書の所有範囲に含まれるか確認する
5. 証明書や開発者が失効済みでないか確認する

オフライン時は、事前に同期した root CA / certificate / revocation 情報のみを使って既知の release を検証する。

## ストア連携

ストアは`about.toml`を読み取り、アプリ情報として利用する。

ストアは release metadata に `package_hash`、`signature`、`certificate_id` などの検証材料を含める。

## 予定

将来的に以下の機能を追加可能とする。

- パッケージ署名
- SHA-256検証
- 依存関係管理
- 権限管理
- 自動アップデート
- 多言語対応
- スクリーンショット
- ストア評価機能
