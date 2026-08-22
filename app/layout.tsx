import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { StoreShell } from "@/components/store-shell";
import "@fontsource/material-symbols-outlined/400.css";
import "./globals.css";

export const metadata: Metadata = {
  title: { default: "mochiOS App Store", template: "%s — mochiOS App Store" },
  description: "mochiOSで利用できるアプリを探して、配布情報を確認できます。",
  metadataBase: new URL("https://store.mochios.org"),
  robots: { index: true, follow: true },
};

export const viewport: Viewport = { width: "device-width", initialScale: 1, themeColor: "#f7f7f7" };

export default function RootLayout({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="ja">
      <body><StoreShell>{children}</StoreShell></body>
    </html>
  );
}
