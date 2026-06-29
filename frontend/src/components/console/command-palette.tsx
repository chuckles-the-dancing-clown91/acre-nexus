"use client";

// ⌘K / Ctrl-K command palette. Self-contained: mount <CommandPalette/> once in
// the console shell and it wires its own global hotkey. Destinations are built
// from the module registry filtered by enablement + permission, plus the
// platform routes for staff — mirroring the sidebar.

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { Command } from "cmdk";
import { Search } from "lucide-react";
import { MODULES } from "@/modules/registry";
import { navIcon } from "@/components/console/nav-icons";
import { useModules } from "@/lib/modules";
import { useAuth } from "@/lib/auth";

interface Dest {
  href: string;
  label: string;
  icon: string;
  group: string;
}

export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const router = useRouter();
  const { isEnabled } = useModules();
  const { user, can } = useAuth();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
      if (e.key === "Escape") setOpen(false);
    };
    const onOpen = () => setOpen(true);
    document.addEventListener("keydown", onKey);
    window.addEventListener("acre:open-command", onOpen);
    return () => {
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("acre:open-command", onOpen);
    };
  }, []);

  const dests = useMemo<Dest[]>(() => {
    const out: Dest[] = [
      { href: "/console", label: "Dashboard", icon: "chart", group: "Operations" },
    ];
    for (const m of MODULES) {
      if (!isEnabled(m.key)) continue;
      for (const item of m.nav) {
        if (item.permission && !can(item.permission)) continue;
        out.push({ href: item.href, label: item.label, icon: item.icon, group: "Operations" });
      }
    }
    if (can("member:read"))
      out.push({ href: "/console/members", label: "Members", icon: "user", group: "Administration" });
    if (can("tenant:manage"))
      out.push({ href: "/console/modules", label: "Modules", icon: "modules", group: "Administration" });
    if (user?.is_platform_staff) {
      out.push({ href: "/console/platform", label: "Platform overview", icon: "platform", group: "Acre Platform" });
      if (can("user:read")) out.push({ href: "/console/platform/users", label: "Users", icon: "user", group: "Acre Platform" });
      if (can("role:read")) out.push({ href: "/console/platform/roles", label: "Roles", icon: "roles", group: "Acre Platform" });
      if (can("audit:read")) out.push({ href: "/console/platform/audit", label: "Audit log", icon: "audit", group: "Acre Platform" });
    }
    return out;
  }, [isEnabled, can, user]);

  const groups = useMemo(() => {
    const g: Record<string, Dest[]> = {};
    for (const d of dests) (g[d.group] ??= []).push(d);
    return Object.entries(g);
  }, [dests]);

  function pick(href: string) {
    setOpen(false);
    router.push(href);
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-start justify-center p-4 pt-[12vh]">
      <div
        className="absolute inset-0 bg-ink/30 backdrop-blur-sm"
        onClick={() => setOpen(false)}
        aria-hidden
      />
      <Command
        label="Command palette"
        className="relative z-10 w-full max-w-lg overflow-hidden rounded-xl border border-line bg-raised shadow-acre-lg"
      >
        <div className="flex items-center gap-2.5 border-b border-line px-3.5">
          <Search className="h-4 w-4 shrink-0 text-ink-3" />
          <Command.Input
            autoFocus
            placeholder="Search pages and actions…"
            className="h-12 w-full bg-transparent text-sm text-ink outline-none placeholder:text-ink-3"
          />
          <kbd className="rounded border border-line bg-surface-2 px-1.5 py-0.5 font-mono text-[10px] text-ink-3">
            ESC
          </kbd>
        </div>
        <Command.List className="max-h-[50vh] overflow-y-auto p-2">
          <Command.Empty className="px-3 py-8 text-center text-sm text-ink-3">
            No results.
          </Command.Empty>
          {groups.map(([group, items]) => (
            <Command.Group
              key={group}
              heading={group}
              className="px-1 pb-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-semibold [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-ink-3"
            >
              {items.map((d) => {
                const Icon = navIcon(d.icon);
                return (
                  <Command.Item
                    key={d.href}
                    value={`${d.label} ${d.href}`}
                    onSelect={() => pick(d.href)}
                    className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-ink-2 outline-none data-[selected=true]:bg-surface-2 data-[selected=true]:text-ink"
                  >
                    <Icon className="h-4 w-4 text-ink-3" />
                    {d.label}
                  </Command.Item>
                );
              })}
            </Command.Group>
          ))}
        </Command.List>
      </Command>
    </div>
  );
}
