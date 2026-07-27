import type { Metadata } from "next";
import Link from "next/link";
import { getStorefront, resolveAssetUrl } from "@/lib/catalog";

export const metadata: Metadata = { title: "カテゴリ" };
export const dynamic = "force-dynamic";

export default async function CategoriesPage() {
  const { categories } = await getStorefront();
  return (
    <><header className="page-heading"><h1>カテゴリ</h1></header>
      {categories.length ? <div className="category-grid">{categories.map((category) => {
        const artwork = resolveAssetUrl(category.artwork);
        return <Link className="category-card" href={`/apps?category=${encodeURIComponent(category.slug)}`} key={category.slug}>{artwork ? <img src={artwork} alt=""/> : null}<strong>{category.name}</strong></Link>;
      })}</div> : <div className="empty"><p>カテゴリはありません。</p></div>}
    </>
  );
}
