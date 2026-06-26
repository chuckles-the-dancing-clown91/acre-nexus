"use client";

// Header dropdown that lets a user switch between the workspaces they belong to
// (Acre HQ / platform plus any client tenants). Hidden when there is only one
// workspace — there is nothing to switch to.

import { useState } from "react";
import { Check, ChevronsUpDown } from "lucide-react";
import { useAuth } from "@/lib/auth";
import { activeWorkspace } from "@/lib/workspaces";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

/**
 * Workspace switcher for the console header. Renders nothing unless the user can
 * reach more than one workspace; selecting an entry calls `switchWorkspace`,
 * which mints a fresh token, updates the session, and refetches all data.
 */
export function WorkspaceSwitcher() {
  const { user, switchWorkspace } = useAuth();
  const [switching, setSwitching] = useState(false);

  if (!user || user.workspaces.length <= 1) return null;

  const current = activeWorkspace(user);

  async function select(tenantId: string | null) {
    if (switching) return;
    setSwitching(true);
    try {
      await switchWorkspace(tenantId);
    } catch {
      /* error toast handled in auth context */
    } finally {
      setSwitching(false);
    }
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        disabled={switching}
        aria-label="Switch workspace"
        className="flex max-w-[180px] items-center gap-1.5 rounded-xl border border-line bg-surface-2 px-2.5 py-1.5 text-xs font-semibold text-ink-2 transition hover:text-ink disabled:opacity-50"
      >
        <span className="truncate">{current?.name ?? "Select workspace"}</span>
        <ChevronsUpDown className="h-3.5 w-3.5 shrink-0" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-[14rem]">
        <DropdownMenuLabel>Workspaces</DropdownMenuLabel>
        <DropdownMenuSeparator />
        {user.workspaces.map((ws) => {
          const isActive = ws.tenant_id === current?.tenant_id;
          return (
            <DropdownMenuItem
              key={ws.tenant_id ?? "platform"}
              onSelect={(e) => {
                e.preventDefault();
                void select(ws.tenant_id);
              }}
              className="justify-between"
            >
              <span className="truncate">{ws.name}</span>
              {isActive && <Check className="h-4 w-4 text-accent-2" />}
            </DropdownMenuItem>
          );
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
