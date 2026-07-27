import { resolveAssetUrl, type CatalogApp } from "@/lib/catalog";

export function AppIcon({ app, large = false }: { app: CatalogApp; large?: boolean }) {
  const source = resolveAssetUrl(app.icon);

  return (
    <span className={`app-icon ${large ? "app-icon--large" : ""}`} aria-hidden={!source}>
      {source ? <img src={source} alt={`${app.name}のアイコン`} /> : null}
    </span>
  );
}
