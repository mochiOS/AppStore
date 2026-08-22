export type CatalogApp = {
  bundle_id: string;
  name: string;
  version: string;
  developer: string;
  description: string;
  icon: string | null;
  subtitle?: string | null;
  category?: string | null;
  kind?: "app" | "game";
  rating?: number | null;
  rating_count?: number | null;
  age_rating?: string | null;
  screenshots?: string[];
  download_url?: string | null;
};

export type CatalogRelease = {
  release_id: string;
  version: string;
  size: number;
  sha256: string;
  package_digest: string;
  changelog?: string | null;
  download_url: string;
  github_repository: string;
  github_release_tag: string;
  asset_name: string;
  created_at: number;
};

export type CatalogAppDetail = CatalogApp & { releases: CatalogRelease[] };

export type StorefrontFeature = {
  id: string;
  eyebrow?: string | null;
  title: string;
  description?: string | null;
  artwork: string;
  app: CatalogApp;
};

export type StorefrontSection = {
  id: string;
  title: string;
  subtitle?: string | null;
  layout: "grid" | "chart" | "row";
  apps: CatalogApp[];
};

export type StorefrontCategory = {
  slug: string;
  name: string;
  artwork?: string | null;
};

export type Storefront = {
  featured: StorefrontFeature[];
  sections: StorefrontSection[];
  categories: StorefrontCategory[];
};

type AppListResponse = { apps: CatalogApp[] };
type SearchResponse = { query: string; results: CatalogApp[] };

const apiBaseUrl = process.env.APPSTORE_API_BASE_URL?.replace(/\/$/, "");

async function request<T>(path: string, allowNotFound = false): Promise<T | null> {
  if (!apiBaseUrl) throw new Error("APPSTORE_API_BASE_URL is not configured");
  try {
    const response = await fetch(`${apiBaseUrl}${path}`, {
      headers: { Accept: "application/json" },
      next: { revalidate: 60 },
    });
    if (allowNotFound && response.status === 404) return null;
    if (!response.ok) throw new Error(`App Store API returned ${response.status}`);
    return (await response.json()) as T;
  } catch (error) {
    if (error instanceof Error) throw error;
    throw new Error("App Store API request failed");
  }
}

export async function listApps(filters?: { kind?: "app" | "game"; category?: string }): Promise<CatalogApp[]> {
  const query = new URLSearchParams();
  if (filters?.kind) query.set("kind", filters.kind);
  if (filters?.category) query.set("category", filters.category);
  const suffix = query.size ? `?${query}` : "";
  const result = await request<AppListResponse>(`/apps${suffix}`);
  let apps = result?.apps ?? [];
  if (filters?.kind === "game") apps = apps.filter((app) => app.kind === "game");
  if (filters?.kind === "app") apps = apps.filter((app) => app.kind !== "game");
  if (filters?.category) apps = apps.filter((app) => !app.category || app.category === filters.category);
  return apps;
}

export async function getStorefront(): Promise<Storefront> {
  const storefront = await request<Storefront>("/storefront");
  const hasStorefrontApps = storefront && (
    storefront.featured.length > 0 ||
    storefront.sections.some((section) => section.apps.length > 0)
  );
  if (hasStorefrontApps) return storefront;
  if (storefront?.categories.length) return storefront;
  const apps = await listApps();
  const catalogApps = apps.filter((app) => app.kind !== "game");
  const games = apps.filter((app) => app.kind === "game");
  const categories = Array.from(
    new Map(
      apps
        .filter((app) => app.category)
        .map((app) => [app.category as string, { slug: app.category as string, name: app.category as string }]),
    ).values(),
  );
  return {
    featured: [],
    sections: [
      ...(catalogApps.length ? [{ id: "apps", title: "アプリ", layout: "row" as const, apps: catalogApps }] : []),
      ...(games.length ? [{ id: "games", title: "ゲーム", layout: "row" as const, apps: games }] : []),
    ],
    categories,
  };
}

export async function findApp(bundleId: string): Promise<CatalogAppDetail | null> {
  const app = await request<CatalogAppDetail>(`/apps/${encodeURIComponent(bundleId)}`, true);
  if (app) return { ...app, releases: app.releases ?? [], screenshots: app.screenshots ?? [] };
  return null;
}

export async function searchApps(query: string): Promise<CatalogApp[]> {
  if (!query.trim()) return [];
  const result = await request<SearchResponse>(`/search?q=${encodeURIComponent(query.trim())}`);
  return result?.results ?? [];
}

export function resolveAssetUrl(path: string | null | undefined): string | null {
  if (!path) return null;
  if (/^https?:\/\//i.test(path) || path.startsWith("data:")) return path;
  if (!apiBaseUrl) return null;
  return new URL(path, `${apiBaseUrl}/`).toString();
}

export function resolveDownloadUrl(path: string): string {
  if (/^https?:\/\//i.test(path)) return path;
  if (!apiBaseUrl) return path;
  return new URL(path, `${apiBaseUrl}/`).toString();
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${Math.round(bytes / 1024 / 1024)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export function formatPublishedAt(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return "—";
  return new Intl.DateTimeFormat("ja-JP", { dateStyle: "medium" }).format(new Date(timestamp * 1000));
}
