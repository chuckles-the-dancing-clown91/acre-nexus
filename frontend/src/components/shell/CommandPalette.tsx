"use client";

// ⌘K: jump to any page you can reach, search the workspace (properties,
// tenants, entities, tickets, LLCs; results are permission-gated server-side),
// or run a quick action.

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Command } from "cmdk";
import { useQuery } from "@tanstack/react-query";
import { CornerDownLeft, LogOut, Radar, Search } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useUiStore } from "@/lib/store";
import { Icon } from "@/components/ui/icon";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/menu";
import { Kbd } from "@/components/ui/misc";
import { useTheme } from "@/theme/ThemeProvider";
import { useNav } from "./nav";
import { useHasTenantScope } from "./tenant-scope";

const KIND_LABEL: Record<string, string> = {
  property: "Properties",
  lease: "Tenants",
  entity: "Entities",
  ticket: "Maintenance",
  llc: "Legal entities",
};
const KIND_ICON: Record<string, string> = {
  property: "building",
  lease: "users",
  entity: "briefcase",
  ticket: "wrench",
  llc: "landmark",
};

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

const itemClass =
  "flex cursor-default items-center gap-3 rounded-xl px-3 py-2.5 text-[13px] text-fg-2 select-none data-[selected=true]:bg-fill-2 data-[selected=true]:text-fg [&_svg]:size-4 [&_svg]:shrink-0 [&_svg]:text-fg-3 data-[selected=true]:[&_svg]:text-accent";
const groupClass =
  "[&_[cmdk-group-heading]]:eyebrow [&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:pt-3 [&_[cmdk-group-heading]]:pb-1.5";

export function CommandPalette() {
  const open = useUiStore((s) => s.paletteOpen);
  const setOpen = useUiStore((s) => s.setPaletteOpen);
  const router = useRouter();
  const { logout } = useAuth();
  const { hud, setHud } = useTheme();
  const { dashboard, groups } = useNav();
  const scoped = useHasTenantScope();
  const [query, setQuery] = useState("");
  const term = useDebounced(query.trim(), 200);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen(!useUiStore.getState().paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setOpen]);

  const { data: results, isFetching } = useQuery({
    queryKey: ["search", term],
    queryFn: () => api.search(term),
    enabled: open && scoped && term.length >= 2,
    staleTime: 30_000,
  });

  function go(href: string) {
    setOpen(false);
    setQuery("");
    router.push(href);
  }

  const hits = term.length >= 2 ? (results?.hits ?? []) : [];
  const hitGroups = Object.keys(KIND_LABEL)
    .map((kind) => ({ kind, items: hits.filter((h) => h.kind === kind) }))
    .filter((g) => g.items.length > 0);

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (!o) setQuery("");
      }}
    >
      <DialogContent
        hideClose
        aria-describedby={undefined}
        className="top-[14%] max-w-[620px] -translate-y-0 overflow-hidden p-0"
      >
        <DialogTitle className="sr-only">Search and commands</DialogTitle>
        <Command loop className="flex max-h-[min(640px,72vh)] flex-col">
          <div className="flex items-center gap-3 border-b border-line px-4">
            <Search className="size-4 shrink-0 text-fg-3" />
            <Command.Input
              value={query}
              onValueChange={setQuery}
              placeholder="Search properties, tenants, tickets… or jump to a page"
              className="h-14 flex-1 bg-transparent text-[15px] text-fg outline-none placeholder:text-fg-4"
            />
            {isFetching && (
              <span className="size-1.5 animate-pulse rounded-full bg-accent" />
            )}
            <Kbd>esc</Kbd>
          </div>

          <Command.List className="flex-1 overflow-y-auto p-2">
            <Command.Empty className="px-3 py-10 text-center text-[13px] text-fg-3">
              {term.length >= 2 && isFetching
                ? "Searching…"
                : "Nothing matches that."}
            </Command.Empty>

            {hitGroups.map((g) => (
              <Command.Group
                key={g.kind}
                heading={KIND_LABEL[g.kind]}
                className={groupClass}
              >
                {g.items.map((hit) => (
                  <Command.Item
                    key={`${hit.kind}-${hit.id}`}
                    value={`${hit.kind}-${hit.id}-${hit.title}`}
                    keywords={[term]}
                    onSelect={() => go(hit.href)}
                    className={itemClass}
                  >
                    <Icon name={KIND_ICON[hit.kind] ?? "building"} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-fg">
                        {hit.title}
                      </span>
                      <span className="block truncate text-xs text-fg-3">
                        {hit.subtitle}
                      </span>
                    </span>
                    <CornerDownLeft className="opacity-0 [[data-selected=true]_&]:opacity-100" />
                  </Command.Item>
                ))}
              </Command.Group>
            ))}

            <Command.Group heading="Go to" className={groupClass}>
              {[
                { label: "Overview", item: dashboard },
                ...groups.flatMap((g) =>
                  g.items.map((item) => ({ label: g.label, item }))
                ),
              ].map(({ label, item }) => (
                <Command.Item
                  key={item.href}
                  value={`go ${item.label} ${label}`}
                  onSelect={() => go(item.href)}
                  className={itemClass}
                >
                  <Icon name={item.icon} />
                  <span className="flex-1 truncate text-fg">{item.label}</span>
                  <span className="text-xs text-fg-4">{label}</span>
                </Command.Item>
              ))}
            </Command.Group>

            <Command.Group heading="Actions" className={groupClass}>
              <Command.Item
                value="toggle hud mode telemetry"
                onSelect={() => setHud(!hud)}
                className={itemClass}
              >
                <Radar />
                <span className="flex-1 text-fg">
                  {hud ? "Turn off HUD mode" : "Turn on HUD mode"}
                </span>
              </Command.Item>
              <Command.Item
                value="sign out log out"
                onSelect={() => {
                  logout();
                  go("/login");
                }}
                className={itemClass}
              >
                <LogOut />
                <span className="flex-1 text-fg">Sign out</span>
              </Command.Item>
            </Command.Group>
          </Command.List>

          <div className="flex items-center gap-4 border-t border-line px-4 py-2.5 text-[11px] text-fg-3">
            <span className="flex items-center gap-1.5">
              <Kbd>↑</Kbd>
              <Kbd>↓</Kbd>
              to move
            </span>
            <span className="flex items-center gap-1.5">
              <Kbd>↵</Kbd>
              to open
            </span>
          </div>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
