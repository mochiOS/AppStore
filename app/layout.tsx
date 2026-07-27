import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { StoreShell } from "@/components/store-shell";
import "./globals.css";

// This is the App Router root layout, so the icon font applies to every route.
/* eslint-disable @next/next/no-page-custom-font */

export const metadata: Metadata = {
  title: { default: "App Store — mochiOS", template: "%s — mochiOS App Store" },
  description: "mochiOS App Store",
  metadataBase: new URL("https://store.mochios.org"),
};

export const viewport: Viewport = { width: "device-width", initialScale: 1, themeColor: "#f7f7f7" };

export default function RootLayout({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="ja">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,400,0,0&display=swap" rel="stylesheet" />
      </head>
      <body><StoreShell>{children}</StoreShell></body>
    </html>
  );
}
