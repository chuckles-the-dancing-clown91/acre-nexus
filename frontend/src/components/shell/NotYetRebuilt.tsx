"use client";

// Any console route that hasn't been rebuilt in the new design yet shows this
// (the catch-all route, plus fixed paths a dynamic route would otherwise take).

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ArrowLeft, Search } from "lucide-react";
import { useUiStore } from "@/lib/store";
import { findNavItem, useNav } from "@/components/shell/nav";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Panel } from "@/components/ui/panel";

export function NotYetRebuilt() {
  const pathname = usePathname();
  const { groups } = useNav();
  const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);
  const match = findNavItem(groups, pathname);
  const label = match?.item.label ?? "This page";

  return (
    <div className="flex min-h-[70vh] items-center justify-center">
      <Panel className="w-full max-w-lg overflow-hidden p-0">
        <div className="relative px-8 pt-10 pb-8 text-center">
          <div className="pointer-events-none absolute inset-x-0 top-0 h-32 bg-[radial-gradient(60%_100%_at_50%_0%,color-mix(in_oklab,var(--accent)_18%,transparent),transparent)]" />
          <div className="relative mx-auto mb-5 flex size-14 items-center justify-center rounded-2xl border border-accent/30 bg-accent/10 text-accent">
            <Icon name={match?.item.icon ?? "blocks"} className="size-6" />
          </div>
          <div className="eyebrow mb-2">{match?.group?.label ?? "Console"}</div>
          <h1 className="text-2xl font-semibold text-fg">
            {label} is being rebuilt
          </h1>
          <p className="mx-auto mt-2 max-w-sm text-[13px] text-fg-2">
            This screen hasn&apos;t moved to the new console yet. Your data is
            untouched, and the new version is on its way.
          </p>
        </div>
        <div className="flex items-center justify-center gap-2 border-t border-line bg-fill/40 px-6 py-4">
          <Button variant="secondary" asChild>
            <Link href="/console">
              <ArrowLeft />
              Dashboard
            </Link>
          </Button>
          <Button variant="ghost" onClick={() => setPaletteOpen(true)}>
            <Search />
            Find something else
          </Button>
        </div>
      </Panel>
    </div>
  );
}
