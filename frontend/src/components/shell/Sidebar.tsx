"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { AnimatePresence, motion } from "motion/react";
import {
  Check,
  ChevronDown,
  ChevronsUpDown,
  LogOut,
  PanelLeftClose,
  PanelLeftOpen,
  Radar,
  Search,
  ShieldCheck,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { useUiStore } from "@/lib/store";
import { activeMembership, activeWorkspace } from "@/lib/workspaces";
import { humanizeKey } from "@/lib/iam";
import { cn } from "@/lib/utils";
import { BrandLogo, VantedgeTile } from "@/components/brand";
import { Icon } from "@/components/ui/icon";
import { Kbd, Tooltip } from "@/components/ui/misc";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/menu";
import { useTheme } from "@/theme/ThemeProvider";
import { findNavItem, useNav, type NavGroup, type NavItem } from "./nav";

export function Sidebar({
  collapsed = false,
  onNavigate,
}: {
  collapsed?: boolean;
  /** Called after a link is followed (closes the mobile drawer). */
  onNavigate?: () => void;
}) {
  const pathname = usePathname();
  const { dashboard, groups } = useNav();
  const activeHref = findNavItem(groups, pathname)?.item.href ?? null;

  return (
    <div className="flex h-full flex-col">
      <div className="px-3 pt-3">
        <WorkspaceHeader collapsed={collapsed} />
      </div>
      <div className="px-3 pt-3">
        <SearchTrigger collapsed={collapsed} />
      </div>

      <nav
        aria-label="Console"
        className="mt-3 flex-1 space-y-4 overflow-y-auto px-3 pb-4"
      >
        <NavRow
          item={dashboard}
          active={activeHref === dashboard.href}
          collapsed={collapsed}
          onNavigate={onNavigate}
        />
        {groups.map((group) => (
          <Group
            key={group.key}
            group={group}
            activeHref={activeHref}
            collapsed={collapsed}
            onNavigate={onNavigate}
          />
        ))}
      </nav>

      <div className="border-t border-line p-3">
        <UserMenu collapsed={collapsed} />
      </div>
    </div>
  );
}

function Group({
  group,
  activeHref,
  collapsed,
  onNavigate,
}: {
  group: NavGroup;
  activeHref: string | null;
  collapsed: boolean;
  onNavigate?: () => void;
}) {
  const folded = useUiStore((s) => !!s.collapsedGroups[group.key]);
  const toggleGroup = useUiStore((s) => s.toggleGroup);
  const holdsActive = group.items.some((i) => i.href === activeHref);
  // A folded group still opens when it holds the page you're on.
  const open = collapsed || !folded || holdsActive;

  return (
    <div>
      {collapsed ? (
        <div className="mx-2 mb-2 h-px bg-line" />
      ) : (
        <button
          type="button"
          onClick={() => toggleGroup(group.key)}
          aria-expanded={open}
          className="eyebrow flex w-full items-center justify-between rounded-md px-2.5 pb-1.5 transition hover:text-fg-2"
        >
          {group.label}
          <ChevronDown
            className={cn(
              "size-3.5 transition-transform duration-200",
              !open && "-rotate-90"
            )}
          />
        </button>
      )}
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            className="space-y-0.5 overflow-hidden"
          >
            {group.items.map((item) => (
              <NavRow
                key={item.href}
                item={item}
                active={item.href === activeHref}
                collapsed={collapsed}
                onNavigate={onNavigate}
              />
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function NavRow({
  item,
  active,
  collapsed,
  onNavigate,
}: {
  item: NavItem;
  active: boolean;
  collapsed: boolean;
  onNavigate?: () => void;
}) {
  const link = (
    <Link
      href={item.href}
      onClick={onNavigate}
      aria-current={active ? "page" : undefined}
      className={cn(
        "group relative flex h-9 items-center gap-3 rounded-xl px-2.5 text-[13px] font-medium transition-colors",
        collapsed && "justify-center px-0",
        active ? "text-fg" : "text-fg-2 hover:bg-fill hover:text-fg"
      )}
    >
      {active && (
        <motion.span
          layoutId="nav-active"
          transition={{ type: "spring", stiffness: 520, damping: 42 }}
          className="absolute inset-0 rounded-xl border border-line-strong bg-fill-2"
        >
          <span className="absolute top-2 bottom-2 -left-px w-[3px] rounded-full bg-accent shadow-[0_0_12px_var(--accent)]" />
        </motion.span>
      )}
      <Icon
        name={item.icon}
        className={cn(
          "relative size-[18px] shrink-0 transition-colors",
          active ? "text-accent" : "text-fg-3 group-hover:text-fg-2"
        )}
      />
      {!collapsed && (
        <span className="relative min-w-0 flex-1 truncate">{item.label}</span>
      )}
      {!collapsed && item.count && (
        <span
          className={cn(
            "relative rounded-full px-1.5 py-px font-mono text-[10px] font-medium tabular-nums",
            item.count.tone === "bad"
              ? "bg-bad/15 text-bad"
              : item.count.tone === "warn"
                ? "bg-warn/15 text-warn"
                : "bg-fill-2 text-fg-2"
          )}
        >
          {item.count.value}
        </span>
      )}
      {!collapsed && item.preview && (
        <span className="relative rounded-md border border-line px-1 text-[9px] font-semibold tracking-wider text-fg-3 uppercase">
          Beta
        </span>
      )}
      {collapsed && item.count && (
        <span
          className={cn(
            "absolute top-1.5 right-2 size-1.5 rounded-full",
            item.count.tone === "bad" ? "bg-bad" : "bg-warn"
          )}
        />
      )}
    </Link>
  );
  return collapsed ? (
    <Tooltip content={item.label} side="right">
      {link}
    </Tooltip>
  ) : (
    link
  );
}

function WorkspaceHeader({ collapsed }: { collapsed: boolean }) {
  const { user, switchWorkspace } = useAuth();
  const { brand, gate } = useTheme();
  if (!user) return null;
  const workspace = activeWorkspace(user);
  const membership = activeMembership(user);
  const platform = workspace?.kind === "platform";
  const role = membership ? humanizeKey(membership.profile_type) : "";
  const persona = membership
    ? [
        role,
        // A title that just repeats the role adds nothing.
        membership.title?.toLowerCase() === role.toLowerCase()
          ? null
          : membership.title,
      ]
        .filter(Boolean)
        .join(" · ")
    : user.is_platform_staff
      ? "Platform staff"
      : "Workspace";
  const switchable = user.workspaces.length > 1;
  // Field roles see their own properties only; say so under their name.
  const reachLine =
    user.reach?.scope === "properties"
      ? ` · ${user.reach.property_ids.length} ${
          user.reach.property_ids.length === 1 ? "property" : "properties"
        }`
      : "";

  // HQ is always Vantedge. A client workspace wears the client's logo only on
  // the client's own domain; on Vantedge's hosts it is a monogram of its name.
  const logo = platform ? (
    <VantedgeTile size={36} />
  ) : gate.branded ? (
    <BrandLogo brand={brand} size={36} />
  ) : (
    <BrandLogo
      brand={{
        ...brand,
        logo_url: null,
        company_name: workspace?.name ?? brand.company_name,
      }}
      size={36}
    />
  );

  const body = (
    <span
      className={cn(
        "flex min-w-0 items-center gap-3",
        collapsed && "justify-center"
      )}
    >
      {logo}
      {!collapsed && (
        <span className="min-w-0 flex-1 text-left leading-tight">
          <span className="block truncate font-display text-[14px] font-semibold text-fg">
            {workspace?.name ?? brand.company_name}
          </span>
          <span className="block truncate text-[11px] text-fg-3">
            {persona}
            {reachLine}
          </span>
        </span>
      )}
    </span>
  );

  if (!switchable) return <div className="px-1 py-1">{body}</div>;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger className="flex w-full items-center gap-2 rounded-xl p-1 text-left transition hover:bg-fill-2">
        {body}
        {!collapsed && (
          <ChevronsUpDown className="mr-1 size-4 shrink-0 text-fg-3" />
        )}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuLabel>Workspaces</DropdownMenuLabel>
        {user.workspaces.map((ws) => (
          <DropdownMenuItem
            key={ws.tenant_id ?? "platform"}
            onSelect={() => void switchWorkspace(ws.tenant_id).catch(() => {})}
          >
            <span className="flex-1 truncate">{ws.name}</span>
            {ws.tenant_id === workspace?.tenant_id && (
              <Check className="!text-accent" />
            )}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function SearchTrigger({ collapsed }: { collapsed: boolean }) {
  const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);
  const button = (
    <button
      type="button"
      onClick={() => setPaletteOpen(true)}
      className={cn(
        "flex h-9 w-full items-center gap-2.5 rounded-xl border border-line bg-fill px-2.5 text-[13px] text-fg-3 transition hover:border-line-strong hover:text-fg-2",
        collapsed && "justify-center px-0"
      )}
    >
      <Search className="size-4 shrink-0" />
      {!collapsed && (
        <>
          <span className="flex-1 text-left">Search or jump to…</span>
          <Kbd>⌘K</Kbd>
        </>
      )}
    </button>
  );
  return collapsed ? (
    <Tooltip content="Search (⌘K)" side="right">
      {button}
    </Tooltip>
  ) : (
    button
  );
}

function initials(name: string) {
  return name
    .split(" ")
    .filter(Boolean)
    .slice(0, 2)
    .map((p) => p[0]?.toUpperCase())
    .join("");
}

function UserMenu({ collapsed }: { collapsed: boolean }) {
  const { user, logout } = useAuth();
  const { hud, setHud } = useTheme();
  const router = useRouter();
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  if (!user) return null;

  return (
    <div className={cn("flex items-center gap-1", collapsed && "flex-col")}>
      <DropdownMenu>
        <DropdownMenuTrigger
          className={cn(
            "flex min-w-0 flex-1 items-center gap-2.5 rounded-xl p-1.5 text-left transition hover:bg-fill-2",
            collapsed && "justify-center"
          )}
        >
          <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-accent to-plasma text-[11px] font-semibold text-accent-fg">
            {initials(user.name)}
          </span>
          {!collapsed && (
            <span className="min-w-0 leading-tight">
              <span className="block truncate text-[13px] font-medium text-fg">
                {user.name}
              </span>
              <span className="block truncate text-[11px] text-fg-3">
                {user.email}
              </span>
            </span>
          )}
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="start" className="w-60">
          <DropdownMenuItem
            onSelect={(e) => {
              e.preventDefault();
              setHud(!hud);
            }}
          >
            <Radar />
            <span className="flex-1">HUD mode</span>
            <span
              className={cn(
                "relative h-4 w-7 rounded-full border transition-colors",
                hud
                  ? "border-accent/50 bg-accent/30"
                  : "border-line-strong bg-fill-2"
              )}
            >
              <span
                className={cn(
                  "absolute top-0.5 size-2.5 rounded-full transition-all",
                  hud ? "left-3.5 bg-accent" : "left-0.5 bg-fg-3"
                )}
              />
            </span>
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => router.push("/console/security")}>
            <ShieldCheck />
            Security & sign-in
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            onSelect={() => {
              logout();
              router.push("/login");
            }}
          >
            <LogOut />
            Sign out
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <Tooltip
        content={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        side="right"
      >
        <button
          type="button"
          onClick={toggleSidebar}
          aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          className="hidden size-8 shrink-0 items-center justify-center rounded-lg text-fg-3 transition hover:bg-fill-2 hover:text-fg lg:flex"
        >
          {sidebarCollapsed ? (
            <PanelLeftOpen className="size-4" />
          ) : (
            <PanelLeftClose className="size-4" />
          )}
        </button>
      </Tooltip>
    </div>
  );
}
