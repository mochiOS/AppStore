import type { Metadata } from "next";
import type { ReactNode } from "react";
import { DashboardShell } from "@/components/dashboard-shell";
import { cn } from "@/lib/utils";
import "./globals.css";

export const metadata: Metadata = {
  title: "mochiOS DeveloperCenter",
  description: "mochiOS AppStore developer dashboard",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html
      lang="ja"
      className={cn("h-full", "antialiased", "font-sans")}
    >
      <body className="min-h-screen bg-background text-foreground font-sans">
        <DashboardShell>{children}</DashboardShell>
      </body>
    </html>
  );
}
