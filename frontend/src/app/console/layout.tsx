"use client";

import { useEffect } from "react";
import { usePathname, useRouter } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { useTheme } from "@/lib/theme";
import { ModulesProvider, useModules } from "@/lib/modules";
import { MODULES } from "@/modules/registry";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Icon } from "@/components/Icon";
import { clsx } from "@/lib/clsx";
import { useUiStore } from "@/lib/store";

/** Wraps the console in the module-enablement context. */
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
  const { user, loading, logout, can } = useAuth();
  const { brand } = useTheme();
  const { isEnabled } = useModules();
  const router = useRouter();
  const pathname = usePathname();
  // Global UI state (Zustand): persisted sidebar collapse.
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);

  useEffect(() => {
    if (!loading && !user) router.replace("/login");
  }, [loading, user, router]);

  if (loading || !user) {
    return (
      <div className="flex min-h-screen items-center justify-center text-ink-3">
        Loading…
      </div>
    );
  }

  // Build the sidebar from the module registry: a module's nav appears only when
  // the module is enabled for this tenant AND the user holds the permission.
  const moduleNav = MODULES.filter((m) => isEnabled(m.key)).flatMap((m) =>
    m.nav
      .filter((item) => !item.permission || can(item.permission))
      .map((item) => ({ ...item, preview: m.preview }))
  );

  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-40 flex h-[60px] items-center gap-4 border-b border-line bg-surface/80 px-5 backdrop-blur">
        <div className="flex items-center gap-2.5">
          <span
            className="flex h-[30px] w-[30px] items-center justify-center rounded-[9px] font-display text-lg font-extrabold text-on-accent"
            style={{ background: "var(--accent)" }}
          >
            {brand.company_name.charAt(0)}
          </span>
          <div className="leading-tight">
            <div className="font-display text-[15px] font-bold">
              {brand.company_name}
            </div>
            <div className="text-[11px] text-ink-3">
              {user.is_platform_staff ? "Platform staff" : "Client workspace"}
            </div>
          </div>
        </div>
        <button
          onClick={toggleSidebar}
          title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-label="Toggle sidebar"
          className="hidden h-9 w-9 items-center justify-center rounded-xl border border-line bg-surface-2 text-ink-2 hover:text-ink sm:flex"
        >
          <Icon name="wrench" size={16} />
        </button>
        <div className="ml-auto flex items-center gap-3">
          <ThemeToggle />
          <div className="flex items-center gap-2">
            <div
              className="flex h-9 w-9 items-center justify-center rounded-xl text-sm font-bold text-white"
              style={{ background: "var(--accent)" }}
            >
              {user.name
                .split(" ")
                .map((n) => n[0])
                .join("")}
            </div>
            <button
              onClick={() => {
                logout();
                router.push("/login");
              }}
              title="Sign out"
              className="flex h-9 w-9 items-center justify-center rounded-xl border border-line bg-surface-2 text-ink-2 hover:text-ink"
            >
              <Icon name="logout" size={17} />
            </button>
          </div>
        </div>
      </header>

      <div className="flex">
        <aside
          className={clsx(
            "hidden shrink-0 border-r border-line p-3 transition-all sm:block",
            sidebarCollapsed ? "w-16" : "w-56"
          )}
        >
          <nav className="space-y-1">
            <NavLink
              href="/console"
              label="Dashboard"
              icon="chart"
              pathname={pathname}
              exact
              collapsed={sidebarCollapsed}
            />
            {moduleNav.map((item) => (
              <NavLink
                key={item.href}
                href={item.href}
                label={item.label}
                icon={item.icon}
                pathname={pathname}
                badge={item.preview ? "Preview" : undefined}
                collapsed={sidebarCollapsed}
              />
            ))}
            {can("member:read") && (
              <NavLink
                href="/console/members"
                label="Members"
                icon="user"
                pathname={pathname}
                collapsed={sidebarCollapsed}
              />
            )}
          </nav>

          {can("tenant:manage") && (
            <Link
              href="/console/modules"
              className={clsx(
                "mt-3 flex items-center gap-2.5 rounded-xl px-3 py-2.5 text-sm font-semibold transition",
                pathname.startsWith("/console/modules")
                  ? "bg-accent-soft text-accent-2"
                  : "text-ink-2 hover:bg-surface-2"
              )}
            >
              <Icon name="wrench" size={17} />
              Modules
            </Link>
          )}

          {user.is_platform_staff && (
            <div className="mt-3 space-y-1">
              <Link
                href="/console/platform"
                className={clsx(
                  "flex items-center gap-2.5 rounded-xl border border-info-soft px-3 py-2.5 text-sm font-semibold text-info",
                  pathname === "/console/platform" && "bg-info-soft"
                )}
              >
                <Icon name="globe" size={17} />
                {!sidebarCollapsed && "Platform admin"}
              </Link>
              {can("user:read") && (
                <NavLink
                  href="/console/platform/users"
                  label="Users"
                  icon="user"
                  pathname={pathname}
                  collapsed={sidebarCollapsed}
                />
              )}
              {can("role:read") && (
                <NavLink
                  href="/console/platform/roles"
                  label="Roles"
                  icon="shield"
                  pathname={pathname}
                  collapsed={sidebarCollapsed}
                />
              )}
            </div>
          )}
        </aside>

        <main className="min-w-0 flex-1 p-6">{children}</main>
      </div>
    </div>
  );
}

/** A single sidebar link with active-state styling and an optional badge. */
function NavLink({
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
  const active = exact
    ? pathname === href
    : pathname === href || pathname.startsWith(href);
  return (
    <Link
      href={href}
      title={collapsed ? label : undefined}
      className={clsx(
        "flex items-center gap-2.5 rounded-xl px-3 py-2.5 text-sm font-semibold transition",
        collapsed && "justify-center",
        active
          ? "bg-accent-soft text-accent-2"
          : "text-ink-2 hover:bg-surface-2"
      )}
    >
      <Icon name={icon} size={17} />
      {!collapsed && <span className="flex-1">{label}</span>}
      {!collapsed && badge && (
        <span className="rounded-md bg-surface-2 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-ink-3">
          {badge}
        </span>
      )}
    </Link>
  );
}
