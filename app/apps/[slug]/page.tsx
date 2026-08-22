import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Rating } from "@/components/app-card";
import { AppIcon } from "@/components/app-icon";
import { MaterialIcon } from "@/components/material-icon";
import { findApp, formatBytes, formatPublishedAt, resolveAssetUrl, resolveDownloadUrl } from "@/lib/catalog";

export async function generateMetadata({ params }: { params: Promise<{ slug: string }> }): Promise<Metadata> {
  const app = await findApp((await params).slug);
  return app ? { title: app.name, description: app.description } : {};
}

export default async function AppDetailPage({ params }: { params: Promise<{ slug: string }> }) {
  const app = await findApp((await params).slug);
  if (!app) notFound();
  const screenshots = (app.screenshots ?? []).map(resolveAssetUrl).filter((value): value is string => Boolean(value));
  const primaryDownload = app.download_url || app.releases[0]?.download_url;

  return (
    <article>
      <header className="app-heading">
        <AppIcon app={app} large />
        <div>
          <h1>{app.name}</h1>
          <p>{app.subtitle || app.developer}</p>
          <div className="app-heading__actions">
            {primaryDownload ? <a className="download-button" href={resolveDownloadUrl(primaryDownload)}>入手</a> : null}
            <span>{app.bundle_id}</span>
          </div>
        </div>
      </header>

      <section className="app-facts" aria-label="アプリ情報">
          {app.rating ? <div><small>評価</small><Rating value={app.rating} count={app.rating_count}/></div> : null}
          {app.age_rating ? <div><small>年齢</small><strong>{app.age_rating}</strong></div> : null}
          {app.category ? <div><small>カテゴリ</small><strong>{app.category}</strong></div> : null}
          <div><small>開発元</small><strong>{app.developer}</strong></div>
          <div><small>バージョン</small><strong>{app.version}</strong></div>
      </section>

      {screenshots.length ? (
        <section className="app-screenshots" aria-label="スクリーンショット">
          {screenshots.map((screenshot) => <img src={screenshot} alt={`${app.name}のスクリーンショット`} key={screenshot}/>) }
        </section>
      ) : null}

      <section className="app-description">
        <h2>概要</h2>
        <p>{app.description}</p>
      </section>

      <section className="section">
        <div className="section-title"><h2>リリース</h2></div>
        {app.releases.length === 0 ? <div className="empty"><p>公開中のリリースはありません。</p></div> : (
          <div className="release-list">
            {app.releases.map((release) => (
              <article className="release" key={release.version}>
                <div className="release__copy">
                  <strong>バージョン {release.version}</strong>
                  <span>{formatBytes(release.size)} · {formatPublishedAt(release.created_at)}</span>
                  {release.changelog ? <p>{release.changelog}</p> : null}
                </div>
                <a className="download-button" href={resolveDownloadUrl(release.download_url)}>
                  <MaterialIcon>download</MaterialIcon>
                  ダウンロード
                </a>
              </article>
            ))}
          </div>
        )}
      </section>

      {app.releases[0] ? <section className="section distribution-section">
        <div className="section-title"><h2>配布情報</h2></div>
        <dl className="distribution-details">
          <div><dt>配布元</dt><dd><a href={`https://github.com/${app.releases[0].github_repository}/releases/tag/${encodeURIComponent(app.releases[0].github_release_tag)}`}>GitHub Releases</a></dd></div>
          <div><dt>ファイル</dt><dd>{app.releases[0].asset_name}</dd></div>
          <div><dt>SHA-256</dt><dd><code>{app.releases[0].sha256}</code></dd></div>
          <div><dt>Package digest</dt><dd><code>{app.releases[0].package_digest}</code></dd></div>
        </dl>
      </section> : null}
    </article>
  );
}
