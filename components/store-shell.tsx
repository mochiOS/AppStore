"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";
import { MaterialIcon } from "@/components/material-icon";

const navigation = [
  { href: "/", label: "見つける", icon: "today" },
  { href: "/apps", label: "アプリ", icon: "apps" },
  { href: "/games", label: "ゲーム", icon: "sports_esports" },
  { href: "/categories", label: "カテゴリ", icon: "grid_view" },
  { href: "/search", label: "検索", icon: "search" },
];

export function StoreShell({ children }: { children: ReactNode }) {
  const pathname = usePathname();

  return (
    <div className="app-shell">
      <header className="site-header">
        <div className="site-header__inner">
          <Link className="brand" href="/">
            <span>mochiOS</span>
            <span className="brand__product">App Store</span>
          </Link>
          <form className="header-search" action="/search" role="search">
            <MaterialIcon>search</MaterialIcon>
            <input name="q" aria-label="アプリを検索" placeholder="検索" />
          </form>
        </div>
      </header>

      <div className="workspace">
        <aside className="sidebar">
          <nav aria-label="App Store">
            {navigation.map((item) => {
              const current = item.href === "/" ? pathname === "/" : pathname.startsWith(item.href);
              return (
                <Link href={item.href} aria-current={current ? "page" : undefined} key={item.href}>
                  <MaterialIcon>{item.icon}</MaterialIcon>
                  <span>{item.label}</span>
                </Link>
              );
            })}
          </nav>
        </aside>
        <main className="main">{children}</main>
      </div>
      <footer className="site-footer">
        <div className="site-footer__inner">
          <span>Copyright © 2026 mochiOS team</span>
          <nav aria-label="フッター">
            <a href="https://policy.mochios.org/terms/">利用規約</a>
            <a href="https://policy.mochios.org/privacy/">プライバシー</a>
            <a href="https://status.mochios.org/">サービス状態</a>
          </nav>
        </div>
      </footer>
    </div>
  );
}
