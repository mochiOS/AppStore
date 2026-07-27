import type { Metadata } from "next";
import { AppList } from "@/components/app-card";
import { listApps } from "@/lib/catalog";

export const metadata: Metadata = { title: "アプリ" };

export default async function AppsPage({ searchParams }: { searchParams: Promise<{ category?: string }> }) {
  const category = (await searchParams).category?.trim();
  const apps = await listApps({ kind: "app", category });
  return <><header className="page-heading"><h1>{category || "アプリ"}</h1></header><AppList apps={apps}/></>;
}
