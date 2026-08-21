# App Store API

Cloudflare Workers上で動作するRust／`workers-rs`版APIです。D1、Accounts Service Binding、Developer CA Service Bindingを使用し、アプリ本体は保存しません。

## App、Build、Submission

```text
App
  ├─ Build         GitHub Releases上の固定MPKGと機械検証結果
  ├─ Submission    Buildとストア情報・申告・非公開審査情報の組
  ├─ Review        管理者による変更不能な判断履歴
  └─ Availability  available / developer_unpublished / removed
```

Build登録だけではApp審査へ提出されません。MPKG Reviewerによる機械検証が`valid`になったBuildを選び、SubmissionのDraftを作成し、確認後に提出します。

## Build登録

```http
POST /v1/developer/apps/{package_id}/releases
Authorization: Bearer <Accounts tokenまたはConsole delegation token>
X-Developer-ID: <32桁小文字UUIDv7 Developer ID>
Content-Type: application/json

{
  "version": "1.2.0",
  "repository": "example/texteditor",
  "release_tag": "v1.2.0",
  "asset": "texteditor-1.2.0-x86_64.mpkg",
  "certificate_id": "<Developer Certificate ID>",
  "changelog": "変更内容"
}
```

DeveloperCAがtokenからAccountとactive・verified Developer Memberを確定し、roleが`owner`／`admin`／`developer`の場合だけ許可します。AccountsはそのAccountの保存済みGitHub grantで`push`／`maintain`／`admin`権限、公開済み固定tag、完全一致`.mpkg` assetを確認します。`viewer`、request内Account ID、`latest`は拒否します。

Developer CAのstatusが有効で内部Developer IDと一致するときだけ、serial、Subject／Issuer key identity、Certificate Developer ID、発行経路をBuildへ固定します。Reviewer reportはこれらすべてと一致しなければ受理しません。report受理時とSubmission承認時にもstatusを再確認します。

1 Appに割り当てられるcurrent Certificateは1つです。現在のCertificateがDeveloperCAで`revoked`になった場合だけ、`PATCH /v1/developer/apps/{bundle_id}/certificate`で新しいactive Certificateへ置換できます。過去BuildのCertificate記録は削除しません。

Package IDは`org.mochios.*`へ限定しません。`com.example.paint`、`io.github.user.tool`、`dev.tas0.volume`のような2 segment以上の小文字reverse-domain形式を共有Certificate crateで検証します。

## Submission審査

```text
Draft
  ↓ 開発者が提出
Submitted
  ↓ 管理者が審査開始
In Review
  ├─ Approved
  ├─ Changes Required           新しいSubmissionで再提出
  ├─ More Information Required 同じSubmissionへ回答してIn Reviewへ戻る
  └─ Rejected
```

Approved時にBuildの機械検証とDeveloper Certificateを再確認します。公開済みVersionと同じVersionは新規公開できませんが、Changes Requiredへの修正は公開前なら同じVersionで再提出できます。Developer UnpublishedまたはRemovedからの再公開は`re_review` Submissionが必要です。

ストア情報は承認済みSubmissionを公開スナップショットとして読み出します。Developer Consoleで次のDraft用情報を編集しても、承認されるまで公開中の表示は変わりません。公開クライアントは固定SHA-256とdownload URLを取得し、GitHub Releasesから直接MPKGを取得します。

## 取得履歴と再ダウンロード

mochiOS IDのactive sessionをBearer tokenとして使い、初回取得時に次を呼び出します。

```http
POST /v1/apps/{bundle_id}/acquisitions
Authorization: Bearer <mochiOS ID session token>
```

取得履歴は`app_acquisitions`へappend-onlyで保存します。AvailableなAppだけ新規取得できます。Developer UnpublishedまたはRemovedになった後は新規取得を拒否し、公開中に取得済みだった同じAccountだけ`GET /v1/apps/{bundle_id}/download`で再ダウンロードできます。Packageのセキュリティ停止中は取得済みでもダウンロードできません。

公開状態とRemoved理由は`GET /v1/apps/{bundle_id}/status`で取得できます。Developerが自ら非公開にした理由は公開しません。

## Reviewer Queue

常駐Reviewerは専用tokenで次のendpointを呼び出します。

```http
POST /v1/reviewer/releases/claim
X-Reviewer-Token: <REVIEWER_TOKEN>
```

未検証Releaseがある場合は、作成日時が最も古い1件へ10分間の一意なvalidation attempt leaseを設定して`200`で返します。Queueが空、または全対象が別Runnerの有効なlease中の場合は`204 No Content`です。取得とlease設定は単一SQLで実行されるため、複数Runner間で同じReleaseをclaimしません。

Runnerは返された`validation_attempt_id`を成功・失敗reportへ含めます。処理中にRunnerが停止した場合はlease期限後に再取得されます。APIの`REVIEWER_TOKEN`とRunnerの`APPSTORE_REVIEWER_TOKEN`は同じ値を設定します。

## 通知と履歴

Build検証、Submission判断、追加情報、Appeal、公開停止・削除、Package緊急停止の通知は、append-onlyの`audit_logs`から生成します。通知本文を別テーブルへ複製せず、既読状態だけを`notification_reads`へAccount単位で保存します。

```text
GET  /v1/developer/notifications
POST /v1/developer/notifications/{notification_id}/read
POST /v1/developer/notifications/read-all
GET  /v1/developer/apps/{bundle_id}/history

GET  /v1/admin/notifications
POST /v1/admin/notifications/{notification_id}/read
POST /v1/admin/notifications/read-all
GET  /v1/admin/releases/{release_id}/history
```

Developer向けAPIは認証中のDeveloperに属するRelease／Packageだけを返し、運営者向けAPIは`ADMIN_TOKEN`と管理Account IDを要求します。既読状態はDeveloper単位ではなくAccount単位なので、同じDeveloperの各メンバーが個別に管理できます。

`download_failed`と`reviewer_internal_error`は運用障害として分類し、開発者へ再提出を要求しません。Package、署名、Certificate、GitHub assetの不整合は開発者の対応が必要な検証結果として表示します。

## ローカル確認

```powershell
cd api
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
npx wrangler d1 migrations apply mochios-app-store --local
npx wrangler dev
```

## 本番

```powershell
npx wrangler secret put APPSTORE_SERVICE_TOKEN
npx wrangler secret put ADMIN_TOKEN
npx wrangler secret put REVIEWER_TOKEN
npx wrangler d1 migrations apply mochios-app-store --remote
npx wrangler deploy
```

`APPSTORE_SERVICE_TOKEN`はAccounts内部API用、`ADMIN_TOKEN`はConsole審査BFF用、`REVIEWER_TOKEN`はMPKG Reviewer専用です。Developer秘密鍵、Offline Root秘密鍵、Online Intermediate秘密鍵はAppStoreへ設定しません。
公開済みパッケージは管理APIから一時停止／再開できます。停止中はストア一覧、詳細、Release一覧、ダウンロードから除外され、新しいReleaseも登録できません。Releaseの失効とは別の可逆なインシデント対応です。

6時間ごとのscheduled integrity checkは公開ReleaseのDeveloperCA statusとGitHub repository／Release／Asset identityを再確認します。不整合や到達不能はfail closedで新規downloadを停止します。管理APIから即時整合性確認と再検証要求も行えます。
