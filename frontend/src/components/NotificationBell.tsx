"use client";

// Console-header bell: unread in-app notification count, refreshed on an
// interval and when the tab regains focus. Links to the notifications page.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import { Icon } from "@/components/Icon";

const POLL_MS = 60_000;

export function NotificationBell() {
  const [unread, setUnread] = useState(0);

  const refresh = useCallback(() => {
    api
      .unreadCount()
      .then((r) => setUnread(r.unread))
      .catch(() => {
        // Silently keep the last count (offline / no tenant context).
      });
  }, []);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, POLL_MS);
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [refresh]);

  return (
    <Link
      href="/console/notifications"
      title="Notifications"
      className="relative flex h-9 w-9 items-center justify-center rounded-xl border border-line bg-surface-2 text-ink-2 hover:text-ink"
    >
      <Icon name="bell" size={17} />
      {unread > 0 && (
        <span
          className="absolute -right-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-full px-1 text-[10px] font-bold text-white"
          style={{ background: "var(--accent)" }}
        >
          {unread > 99 ? "99+" : unread}
        </span>
      )}
    </Link>
  );
}
