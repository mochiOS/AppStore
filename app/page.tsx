import Link from "next/link";
import { FeatureCard, StoreSection } from "@/components/app-card";
import { getStorefront, resolveAssetUrl } from "@/lib/catalog";

export const dynamic = "force-dynamic";

export default async function StorePage() {
  const storefront = await getStorefront();
  const hasContent = storefront.featured.length > 0 || storefront.sections.some((section) => section.apps.length > 0) || storefront.categories.length > 0;

  return (
    <>
      <header className="page-heading"><h1>見つける</h1></header>

      {storefront.featured.length ? <section className="feature-grid">{storefront.featured.map((feature) => <FeatureCard feature={feature} key={feature.id}/>)}</section> : null}
      {storefront.sections.map((section) => <StoreSection section={section} key={section.id}/>) }

      {storefront.categories.length ? (
        <section className="store-section">
          <header className="store-section__heading"><div><h2>カテゴリ</h2></div><Link href="/categories">すべて表示</Link></header>
          <div className="category-rail">{storefront.categories.slice(0, 6).map((category) => {
            const artwork = resolveAssetUrl(category.artwork);
            return <Link className="category-tile" href={`/apps?category=${encodeURIComponent(category.slug)}`} key={category.slug}>{artwork ? <img src={artwork} alt=""/> : null}<strong>{category.name}</strong></Link>;
          })}</div>
        </section>
      ) : null}

      {!hasContent ? <div className="empty storefront-empty"><p>公開中のコンテンツはありません。</p></div> : null}
    </>
  );
}
