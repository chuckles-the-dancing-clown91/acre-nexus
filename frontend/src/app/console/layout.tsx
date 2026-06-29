"use client";

import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import Link from "next/link";
import {
  Check,
  ChevronsUpDown,
  Loader2,
  LogOut,
  Menu,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Sun,
  X,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { useTheme } from "@/lib/theme";
import { ModulesProvider, useModules } from "@/lib/modules";
import { MODULES } from "@/modules/registry";
import { navIcon } from "@/components/console/nav-icons";
import { CommandPalette } from "@/components/console/command-palette";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/lib/store";
import { activeMembership } from "@/lib/workspaces";
import { humanizeKey } from "@/lib/iam";

export default function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <ModulesProvider>
      <ConsoleShell>{children}</ConsoleShell>
    </ModulesProvider>
  );
}

function ConsoleShell({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();
  const { setBrandTenant } = useTheme();
  const router = useRouter();
  const collapsed = useUiStore((s) => s.sidebarCollapsed);
  const [mobileOpen, setMobileOpen] = useState(false);
  const pathname = usePathname();

  useEffect(() => {
    if (!loading && !user) router.replace("/login");
  }, [loading, user, router]);

  // Apply the active workspace's white-label brand (Acre default at the HQ /
  // platform workspace). Re-runs when the user switches workspace.
  useEffect(() => {
    if (!user) return;
    const slug = user.active_tenant_id
      ? (user.workspaces?.find((w) => w.tenant_id === user.active_tenant_id)
          ?.slug ?? null)
      : null;
    setBrandTenant(slug);
  }, [user, setBrandTenant]);

  // Close the mobile drawer on navigation.
  useEffect(() => {
    setMobileOpen(false);
  }, [pathname]);

  if (loading || !user) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-bg">
        <Loader2 className="h-6 w-6 animate-spin text-ink-3" />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-bg">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[200] focus:rounded-lg focus:bg-accent focus:px-4 focus:py-2 focus:text-sm focus:font-semibold focus:text-on-accent"
      >
        Skip to content
      </a>
      <CommandPalette />
      {/* Desktop sidebar */}
      <aside
        className={cn(
          "fixed inset-y-0 left-0 z-30 hidden border-r border-line bg-surface lg:flex lg:flex-col",
          collapsed ? "w-[72px]" : "w-64"
        )}
      >
        <Sidebar collapsed={collapsed} />
      </aside>

      {/* Mobile drawer */}
      {mobileOpen && (
        <div className="lg:hidden">
          <div
            className="fixed inset-0 z-40 bg-ink/30 backdrop-blur-sm"
            onClick={() => setMobileOpen(false)}
            aria-hidden
          />
          <aside className="fixed inset-y-0 left-0 z-50 flex w-72 flex-col border-r border-line bg-surface">
            <button
              onClick={() => setMobileOpen(false)}
              aria-label="Close menu"
              className="absolute right-3 top-3 flex h-8 w-8 items-center justify-center rounded-lg text-ink-2 hover:bg-surface-2"
            >
              <X className="h-4 w-4" />
            </button>
            <Sidebar collapsed={false} />
          </aside>
        </div>
      )}

      {/* Main column */}
      <div className={cn(collapsed ? "lg:pl-[72px]" : "lg:pl-64")}>
        {/* Mobile top bar */}
        <div className="sticky top-0 z-20 flex h-14 items-center gap-3 border-b border-line bg-surface/80 px-4 backdrop-blur lg:hidden">
          <button
            onClick={() => setMobileOpen(true)}
            aria-label="Open menu"
            className="flex h-9 w-9 items-center justify-center rounded-lg border border-line text-ink-2 hover:bg-surface-2"
          >
            <Menu className="h-4 w-4" />
          </button>
          <BrandMark />
        </div>

        <main id="main-content" className="mx-auto max-w-[1400px] p-5 sm:p-7">
          <div className="acre-fade">{children}</div>
        </main>
      </div>
    </div>
  );
}

function Sidebar({ collapsed }: { collapsed: boolean }) {
  const { user, logout, can } = useAuth();
  const router = useRouter();
  const pathname = usePathname();
  const { isEnabled } = useModules();
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);

  if (!user) return null;

  const moduleNav = MODULES.filter((m) => isEnabled(m.key)).flatMap((m) =>
    m.nav
      .filter((item) => !item.permission || can(item.permission))
      .map((item) => ({ ...item, preview: m.preview }))
  );

  const admin = [
    can("member:read") && { href: "/console/members", label: "Members", icon: "user" },
    can("tenant:manage") && { href: "/console/modules", label: "Modules", icon: "modules" },
  ].filter(Boolean) as { href: string; label: string; icon: string }[];

  const platform = user.is_platform_staff
    ? ([
        { href: "/console/platform", label: "Overview", icon: "platform", exact: true },
        can("user:read") && { href: "/console/platform/users", label: "Users", icon: "user" },
        can("role:read") && { href: "/console/platform/roles", label: "Roles", icon: "roles" },
        can("audit:read") && { href: "/console/platform/audit", label: "Audit", icon: "audit" },
      ].filter(Boolean) as { href: string; label: string; icon: string; exact?: boolean }[])
    : [];

  return (
    <div className="flex h-full flex-col">
      {/* Brand + collapse */}
      <div className="flex h-14 items-center gap-2 border-b border-line px-3">
        <BrandMark collapsed={collapsed} />
        <button
          onClick={toggleSidebar}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          className="ml-auto hidden h-8 w-8 items-center justify-center rounded-lg text-ink-3 hover:bg-surface-2 hover:text-ink lg:flex"
        >
          {collapsed ? (
            <PanelLeftOpen className="h-4 w-4" />
          ) : (
            <PanelLeftClose className="h-4 w-4" />
          )}
        </button>
      </div>

      {/* Workspace switcher */}
      <div className="p-2">
        <WorkspaceSwitcher collapsed={collapsed} />
      </div>

      {/* Command palette trigger */}
      {!collapsed && (
        <div className="px-2 pb-1">
          <button
            onClick={() => window.dispatchEvent(new Event("acre:open-command"))}
            className="flex w-full items-center gap-2 rounded-lg border border-line bg-surface-2/40 px-2.5 py-1.5 text-left text-[13px] text-ink-3 transition hover:bg-surface-2 hover:text-ink-2"
          >
            <Search className="h-3.5 w-3.5" />
            <span className="flex-1">Search…</span>
            <kbd className="rounded border border-line bg-surface px-1 font-mono text-[10px]">
              ⌘K
            </kbd>
          </button>
        </div>
      )}

      {/* Nav */}
      <nav className="flex-1 space-y-5 overflow-y-auto px-2 pb-4">
        <NavSection label="Operations" collapsed={collapsed}>
          <NavItem
            href="/console"
            label="Dashboard"
            icon="chart"
            pathname={pathname}
            exact
            collapsed={collapsed}
          />
          {moduleNav.map((item) => (
            <NavItem
              key={item.href}
              href={item.href}
              label={item.label}
              icon={item.icon}
              pathname={pathname}
              badge={item.preview ? "Preview" : undefined}
              collapsed={collapsed}
            />
          ))}
        </NavSection>

        {admin.length > 0 && (
          <NavSection label="Administration" collapsed={collapsed}>
            {admin.map((item) => (
              <NavItem
                key={item.href}
                href={item.href}
                label={item.label}
                icon={item.icon}
                pathname={pathname}
                collapsed={collapsed}
              />
            ))}
          </NavSection>
        )}

        {platform.length > 0 && (
          <NavSection label="Acre Platform" collapsed={collapsed} accent>
            {platform.map((item) => (
              <NavItem
                key={item.href}
                href={item.href}
                label={item.label}
                icon={item.icon}
                pathname={pathname}
                exact={item.exact}
                collapsed={collapsed}
              />
            ))}
          </NavSection>
        )}
      </nav>

      {/* User footer */}
      <div className="border-t border-line p-2">
        <UserFooter
          collapsed={collapsed}
          onLogout={() => {
            logout();
            router.push("/login");
          }}
        />
      </div>
    </div>
  );
}

function BrandMark({ collapsed = false }: { collapsed?: boolean }) {
  const { brand } = useTheme();
  return (
    <Link href="/console" className="flex items-center gap-2.5 overflow-hidden">
      <span
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg font-display text-base font-bold text-on-accent"
        style={{ background: "var(--accent)" }}
      >
        {brand.company_name.charAt(0)}
      </span>
      {!collapsed && (
        <span className="truncate font-display text-[15px] font-bold tracking-tight text-ink">
          {brand.company_name}
        </span>
      )}
    </Link>
  );
}

function WorkspaceSwitcher({ collapsed }: { collapsed: boolean }) {
  const { user, switchWorkspace } = useAuth();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  if (!user) return null;

  const workspaces = user.workspaces ?? [];
  const current =
    workspaces.find((w) =>
      user.active_tenant_id == null
        ? w.kind === "platform"
        : w.tenant_id === user.active_tenant_id
    ) ?? workspaces[0];

  const membership = activeMembership(user);
  const persona = membership
    ? humanizeKey(membership.profile_type)
    : user.is_platform_staff
      ? "Platform staff"
      : "Member";

  async function pick(tenantId: string | null) {
    setOpen(false);
    if (
      (tenantId == null && user!.active_tenant_id == null) ||
      tenantId === user!.active_tenant_id
    )
      return;
    setBusy(true);
    try {
      await switchWorkspace(tenantId);
    } finally {
      setBusy(false);
    }
  }

  if (collapsed) {
    return (
      <div
        className="mx-auto flex h-9 w-9 items-center justify-center rounded-lg border border-line bg-surface-2 font-display text-sm font-bold text-ink"
        title={current?.name}
      >
        {(current?.name ?? "?").charAt(0)}
      </div>
    );
  }

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        disabled={busy}
        className="flex w-full items-center gap-2.5 rounded-lg border border-line bg-surface-2/60 px-2.5 py-2 text-left transition hover:bg-surface-2"
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-accent-soft font-display text-xs font-bold text-accent-2">
          {(current?.name ?? "?").charAt(0)}
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[13px] font-semibold text-ink">
            {current?.name ?? "Workspace"}
          </span>
          <span className="block truncate text-[11px] text-ink-3">{persona}</span>
        </span>
        {busy ? (
          <Loader2 className="h-4 w-4 shrink-0 animate-spin text-ink-3" />
        ) : (
          <ChevronsUpDown className="h-4 w-4 shrink-0 text-ink-3" />
        )}
      </button>

      {open && (
        <>
          <div
            className="fixed inset-0 z-40"
            onClick={() => setOpen(false)}
            aria-hidden
          />
          <div className="absolute left-0 right-0 top-[calc(100%+4px)] z-50 overflow-hidden rounded-lg border border-line bg-raised p-1 shadow-acre-lg">
            <div className="px-2 py-1.5 text-[11px] font-semibold uppercase tracking-wide text-ink-3">
              Switch workspace
            </div>
            {user.is_platform_staff && (
              <WorkspaceRow
                name="Acre HQ"
                sub="Platform"
                active={user.active_tenant_id == null}
                onClick={() => pick(null)}
              />
            )}
            {workspaces
              .filter((w) => w.kind === "tenant")
              .map((w) => (
                <WorkspaceRow
                  key={w.tenant_id}
                  name={w.name}
                  sub={w.slug ?? "Tenant"}
                  active={w.tenant_id === user.active_tenant_id}
                  onClick={() => pick(w.tenant_id ?? null)}
                />
              ))}
          </div>
        </>
      )}
    </div>
  );
}

function WorkspaceRow({
  name,
  sub,
  active,
  onClick,
}: {
  name: string;
  sub: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left hover:bg-surface-2"
    >
      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-surface-2 font-display text-xs font-bold text-ink-2">
        {name.charAt(0)}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] font-medium text-ink">
          {name}
        </span>
        <span className="block truncate text-[11px] text-ink-3">{sub}</span>
      </span>
      {active && <Check className="h-4 w-4 shrink-0 text-accent-2" />}
    </button>
  );
}

function NavSection({
  label,
  collapsed,
  accent,
  children,
}: {
  label: string;
  collapsed: boolean;
  accent?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div>
      {!collapsed && (
        <div
          className={cn(
            "px-2.5 pb-1.5 text-[11px] font-semibold uppercase tracking-wide",
            accent ? "text-accent-2" : "text-ink-3"
          )}
        >
          {label}
        </div>
      )}
      <div className="space-y-0.5">{children}</div>
    </div>
  );
}

function NavItem({
  href,
  label,
  icon,
  pathname,
  exact,
  badge,
  collapsed,
}: {
  href: string;
  label: string;
  icon: string;
  pathname: string;
  exact?: boolean;
  badge?: string;
  collapsed?: boolean;
}) {
  const Icon = navIcon(icon);
  const active = exact ? pathname === href : pathname === href || pathname.startsWith(href + "/");
  return (
    <Link
      href={href}
      title={collapsed ? label : undefined}
      className={cn(
        "group flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] font-medium transition",
        collapsed && "justify-center",
        active
          ? "bg-accent-soft text-accent-2"
          : "text-ink-2 hover:bg-surface-2 hover:text-ink"
      )}
    >
      <Icon
        className={cn(
          "h-[18px] w-[18px] shrink-0",
          active ? "text-accent-2" : "text-ink-3 group-hover:text-ink-2"
        )}
      />
      {!collapsed && <span className="flex-1 truncate">{label}</span>}
      {!collapsed && badge && (
        <span className="rounded bg-surface-2 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-ink-3">
          {badge}
        </span>
      )}
    </Link>
  );
}

function UserFooter({
  collapsed,
  onLogout,
}: {
  collapsed: boolean;
  onLogout: () => void;
}) {
  const { user } = useAuth();
  const { dark, toggleDark } = useTheme();
  if (!user) return null;

  const initials = user.name
    .split(" ")
    .map((n) => n[0])
    .slice(0, 2)
    .join("");

  if (collapsed) {
    return (
      <div className="flex flex-col items-center gap-1">
        <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-accent text-xs font-bold text-on-accent">
          {initials}
        </div>
        <button
          onClick={toggleDark}
          aria-label="Toggle theme"
          className="flex h-8 w-8 items-center justify-center rounded-lg text-ink-3 hover:bg-surface-2 hover:text-ink"
        >
          {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </button>
        <button
          onClick={onLogout}
          aria-label="Sign out"
          className="flex h-8 w-8 items-center justify-center rounded-lg text-ink-3 hover:bg-surface-2 hover:text-ink"
        >
          <LogOut className="h-4 w-4" />
        </button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2.5 rounded-lg px-1.5 py-1.5">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent text-xs font-bold text-on-accent">
        {initials}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-semibold text-ink">
          {user.name}
        </div>
        <div className="truncate text-[11px] text-ink-3">{user.email}</div>
      </div>
      <button
        onClick={toggleDark}
        aria-label="Toggle theme"
        className="flex h-8 w-8 items-center justify-center rounded-lg text-ink-3 hover:bg-surface-2 hover:text-ink"
      >
        {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
      </button>
      <button
        onClick={onLogout}
        aria-label="Sign out"
        className="flex h-8 w-8 items-center justify-center rounded-lg text-ink-3 hover:bg-surface-2 hover:text-ink"
      >
        <LogOut className="h-4 w-4" />
      </button>
    </div>
  );
}
