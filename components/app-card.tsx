import Link from "next/link";
import { AppIcon } from "@/components/app-icon";
import { MaterialIcon } from "@/components/material-icon";
import type { CatalogApp, StorefrontFeature, StorefrontSection } from "@/lib/catalog";
import { resolveAssetUrl } from "@/lib/catalog";

export function AppAction({ app }: { app: CatalogApp }) {
  return <span className="app-action">{app.price_label || "入手"}</span>;
}

export function AppRow({ app, rank }: { app: CatalogApp; rank?: number }) {
  return (
    <Link className="app-row" href={`/apps/${encodeURIComponent(app.bundle_id)}`}>
      {rank ? <span className="app-rank">{rank}</span> : null}
      <AppIcon app={app} />
      <span className="app-row__copy">
        <strong>{app.name}</strong>
        <small>{app.subtitle || app.developer}</small>
        {app.category ? <span>{app.category}</span> : null}
      </span>
      <AppAction app={app} />
    </Link>
  );
}

export function AppTile({ app }: { app: CatalogApp }) {
  return (
    <Link className="app-tile" href={`/apps/${encodeURIComponent(app.bundle_id)}`}>
      <AppIcon app={app} />
      <span className="app-tile__copy"><strong>{app.name}</strong><small>{app.subtitle || app.developer}</small></span>
      <AppAction app={app} />
    </Link>
  );
}

export function AppList({ apps }: { apps: CatalogApp[] }) {
  if (apps.length === 0) return <div className="empty"><p>公開中のアプリはありません。</p></div>;
  return <div className="app-list">{apps.map((app) => <AppRow app={app} key={app.bundle_id} />)}</div>;
}

export function FeatureCard({ feature }: { feature: StorefrontFeature }) {
  const artwork = resolveAssetUrl(feature.artwork);
  if (!artwork) return null;
  return (
    <Link className="feature-card" href={`/apps/${encodeURIComponent(feature.app.bundle_id)}`}>
      <img src={artwork} alt="" />
      <span className="feature-card__shade" />
      <span className="feature-card__copy">
        {feature.eyebrow ? <small>{feature.eyebrow}</small> : null}
        <strong>{feature.title}</strong>
        {feature.description ? <span>{feature.description}</span> : null}
      </span>
      <span className="feature-card__app"><AppIcon app={feature.app}/><span><strong>{feature.app.name}</strong><small>{feature.app.subtitle || feature.app.developer}</small></span><AppAction app={feature.app}/></span>
    </Link>
  );
}

export function StoreSection({ section }: { section: StorefrontSection }) {
  if (section.apps.length === 0) return null;
  return (
    <section className="store-section">
      <header className="store-section__heading"><div><h2>{section.title}</h2>{section.subtitle ? <p>{section.subtitle}</p> : null}</div></header>
      {section.layout === "chart" ? (
        <div className="chart-grid">{section.apps.map((app, index) => <AppRow app={app} rank={index + 1} key={app.bundle_id}/>)}</div>
      ) : section.layout === "row" ? (
        <div className="app-rail">{section.apps.map((app) => <AppTile app={app} key={app.bundle_id}/>)}</div>
      ) : (
        <div className="tile-grid">{section.apps.map((app) => <AppTile app={app} key={app.bundle_id}/>)}</div>
      )}
    </section>
  );
}

export function Rating({ value, count }: { value: number; count?: number | null }) {
  return <span className="rating"><MaterialIcon>star</MaterialIcon><strong>{value.toFixed(1)}</strong>{count ? <small>{count.toLocaleString("ja-JP")}件</small> : null}</span>;
}
