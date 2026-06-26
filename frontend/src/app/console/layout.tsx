"use client";

import { useEffect } from "react";
import { usePathname, useRouter } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { useTheme } from "@/lib/theme";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Icon } from "@/components/Icon";
import { clsx } from "@/lib/clsx";

const NAV = [
  { href: "/console", label: "Dashboard", icon: "chart" },
  { href: "/console/properties", label: "Properties", icon: "building" },
  { href: "/console/llcs", label: "LLCs", icon: "shield" },
  { href: "/console/applications", label: "Applications", icon: "user" },
  { href: "/console/tokens", label: "API tokens", icon: "key" },
];

export default function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const { user, loading, logout } = useAuth();
  const { brand } = useTheme();
  const router = useRouter();
  const pathname = usePathname();

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
        <aside className="hidden w-56 shrink-0 border-r border-line p-3 sm:block">
          <nav className="space-y-1">
            {NAV.map((item) => {
              const active =
                pathname === item.href ||
                (item.href !== "/console" && pathname.startsWith(item.href));
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={clsx(
                    "flex items-center gap-2.5 rounded-xl px-3 py-2.5 text-sm font-semibold transition",
                    active
                      ? "bg-accent-soft text-accent-2"
                      : "text-ink-2 hover:bg-surface-2"
                  )}
                >
                  <Icon name={item.icon} size={17} />
                  {item.label}
                </Link>
              );
            })}
          </nav>
          {user.is_platform_staff && (
            <Link
              href="/console/platform"
              className={clsx(
                "mt-3 flex items-center gap-2.5 rounded-xl border border-info-soft px-3 py-2.5 text-sm font-semibold text-info",
                pathname.startsWith("/console/platform") && "bg-info-soft"
              )}
            >
              <Icon name="globe" size={17} />
              Platform admin
            </Link>
          )}
        </aside>

        <main className="min-w-0 flex-1 p-6">{children}</main>
      </div>
    </div>
  );
}
