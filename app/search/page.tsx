import type { Metadata } from "next";
import { AppList } from "@/components/app-card";
import { MaterialIcon } from "@/components/material-icon";
import { searchApps } from "@/lib/catalog";

export const metadata: Metadata = { title: "検索" };

export default async function SearchPage({ searchParams }: { searchParams: Promise<{ q?: string }> }) {
  const query = (await searchParams).q?.trim() ?? "";
  const apps = query ? await searchApps(query) : [];

  return (
    <>
      <header className="page-heading"><h1>検索</h1></header>
      <form className="search-form" action="/search">
        <MaterialIcon>search</MaterialIcon>
        <input name="q" defaultValue={query} aria-label="アプリを検索" placeholder="アプリ名、Developer、Bundle ID" autoFocus />
        <button type="submit">検索</button>
      </form>
      {query ? (
        <section className="section">
          <div className="section-title"><h2>検索結果</h2><span>{apps.length}件</span></div>
          <AppList apps={apps} />
        </section>
      ) : null}
    </>
  );
}
