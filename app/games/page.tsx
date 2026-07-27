import type { Metadata } from "next";
import { AppList } from "@/components/app-card";
import { listApps } from "@/lib/catalog";

export const metadata: Metadata = { title: "ゲーム" };
export const dynamic = "force-dynamic";

export default async function GamesPage() {
  const apps = await listApps({ kind: "game" });
  return <><header className="page-heading"><h1>ゲーム</h1></header><AppList apps={apps}/></>;
}
