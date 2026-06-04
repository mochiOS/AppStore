"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  AppStoreIcon,
  NewReleasesIcon,
  Settings04Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const navigation = [
  {
    href: "/apps",
    label: "Apps",
    icon: AppStoreIcon,
  },
  {
    href: "/releases",
    label: "Releases",
    icon: NewReleasesIcon,
  },
  {
    href: "/settings",
    label: "Settings",
    icon: Settings04Icon,
  },
] as const;

function isActivePath(pathname: string, href: string) {
  return pathname === href || pathname.startsWith(`${href}/`);
}

export function DashboardShell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const current = navigation.find((item) => isActivePath(pathname, item.href)) ?? navigation[0];

  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top,_rgba(223,230,255,0.9),_transparent_34%),linear-gradient(180deg,_#fcfbf8_0%,_#f4f1ea_100%)] text-zinc-950">
      <div className="mx-auto flex min-h-screen w-full max-w-[1600px] flex-col lg:flex-row">
        <aside className="border-b border-zinc-200/80 bg-white/70 px-5 py-5 backdrop-blur-xl lg:w-[260px] lg:border-b-0 lg:border-r">
          <div className="flex items-center gap-3">
            <div className="flex size-11 items-center justify-center rounded-2xl border border-zinc-200 bg-zinc-950 text-white shadow-sm">
              <HugeiconsIcon icon={AppStoreIcon} size={22} color="currentColor" strokeWidth={1.8} />
            </div>
            <div>
              <p className="text-[11px] font-medium uppercase tracking-[0.32em] text-zinc-500">
                mochiOS
              </p>
              <h1 className="text-lg font-semibold tracking-tight">DeveloperCenter</h1>
            </div>
          </div>

          <nav className="mt-6 flex gap-2 overflow-x-auto pb-1 lg:flex-col lg:overflow-visible lg:pb-0">
            {navigation.map((item) => {
              const active = isActivePath(pathname, item.href);

              return (
                <Button
                  key={item.href}
                  asChild
                  variant={active ? "default" : "ghost"}
                  className={cn(
                    "h-auto min-w-[11rem] justify-start rounded-3xl border px-4 py-3 text-left lg:min-w-0",
                    active
                      ? "border-zinc-950 bg-zinc-950 text-white hover:bg-zinc-900"
                      : "border-transparent bg-transparent text-zinc-600 hover:border-zinc-200 hover:bg-white"
                  )}
                >
                  <Link href={item.href}>
                    <div className="flex size-9 items-center justify-center rounded-2xl bg-white/10 text-current">
                      <HugeiconsIcon icon={item.icon} size={18} color="currentColor" strokeWidth={1.8} />
                    </div>
                    <div className="ml-3 min-w-0 text-sm font-medium">{item.label}</div>
                  </Link>
                </Button>
              );
            })}
          </nav>
        </aside>

        <div className="flex min-w-0 flex-1 flex-col">
          <header className="border-b border-zinc-200/80 bg-white/60 px-5 py-4 backdrop-blur-xl sm:px-6 lg:px-8">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
              <div>
                <p className="text-[11px] font-medium uppercase tracking-[0.32em] text-zinc-500">
                  {current.label}
                </p>
                <h2 className="mt-1 text-2xl font-semibold tracking-tight text-zinc-950">mochiOS DeveloperCenter</h2>
              </div>

              <div className="flex flex-wrap items-center gap-3">
                <div className="rounded-full border border-zinc-200 bg-white px-4 py-2 text-sm text-zinc-600 shadow-sm">
                  Internal review
                </div>
              </div>
            </div>
          </header>

          <main className="flex-1 px-5 py-6 sm:px-6 lg:px-8">{children}</main>
        </div>
      </div>
    </div>
  );
}
