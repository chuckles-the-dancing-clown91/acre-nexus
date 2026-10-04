"use client";

// Notifications: the signed-in user's inbox, browser push on this device, and
// (with `integrations:manage`) how the workspace delivers email, SMS and chat,
// plus the message templates every channel renders from.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Bell, BellOff, BellRing, CheckCheck, Inbox } from "lucide-react";
import { toast } from "sonner";
import { api, type InboxEntry } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import {
  currentSubscription,
  disablePush,
  enablePush,
  pushSupported,
} from "@/lib/push";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { Providers } from "./providers";
import { Templates } from "./templates";

const INBOX_TABS = [
  ["all", "All"],
  ["unread", "Unread"],
] as const;

function timeAgo(iso: string): string {
  const s = Math.max(
    1,
    Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  );
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

export default function NotificationsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const manage = can("integrations:manage");

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="Notifications"
        description="Your inbox, push on this device, and how the workspace sends email, texts and chat."
      />
      <InboxPanel />
      <PushPanel />
      {manage && scoped && <Providers />}
      {manage && scoped && <Templates />}
    </div>
  );
}

function InboxPanel() {
  const qc = useQueryClient();
  const [view, setView] = useState<"all" | "unread">("all");
  const [busy, setBusy] = useState(false);
  const inbox = useQuery({
    queryKey: ["notifications", "inbox"],
    queryFn: () => api.inbox(),
    refetchInterval: 60_000,
  });
  const all = inbox.data ?? [];
  const unread = all.filter((n) => !n.read_at).length;
  const rows = view === "unread" ? all.filter((n) => !n.read_at) : all;

  async function run(fn: () => Promise<unknown>) {
    setBusy(true);
    try {
      await fn();
      // The header bell reads ["notifications", "unread"].
      await qc.invalidateQueries({ queryKey: ["notifications"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel className="overflow-hidden">
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <Inbox className="size-4" />
            Inbox
            {unread > 0 && <Badge tone="accent">{unread} unread</Badge>}
          </span>
        }
        description="New applications, work orders, signatures and texts as they happen."
        action={
          unread > 0 && (
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => run(() => api.markAllNotificationsRead())}
            >
              <CheckCheck />
              Mark all read
            </Button>
          )
        }
      />
      <div className="px-5 pt-4 pb-3">
        <Tabs tabs={INBOX_TABS} value={view} onChange={setView} />
      </div>
      {inbox.isLoading && (
        <div className="space-y-2 px-5 pb-5">
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="h-14" />
          ))}
        </div>
      )}
      {inbox.error && (
        <p className="px-5 pb-5 text-[13px] text-bad">
          Couldn&apos;t load your inbox: {inbox.error.message}
        </p>
      )}
      {inbox.data && rows.length === 0 && (
        <EmptyState
          icon={<Bell />}
          title={view === "unread" ? "All caught up" : "Nothing yet"}
          description="New applications and other workspace events show up here."
        />
      )}
      <ul className="divide-y divide-line border-t border-line">
        {rows.map((n) => (
          <InboxRow
            key={n.id}
            n={n}
            busy={busy}
            onRead={() => run(() => api.markNotificationRead(n.id))}
          />
        ))}
      </ul>
    </Panel>
  );
}

function InboxRow({
  n,
  busy,
  onRead,
}: {
  n: InboxEntry;
  busy: boolean;
  onRead: () => void;
}) {
  const unread = !n.read_at;
  return (
    <li className="flex items-start gap-3 px-5 py-3">
      <span
        aria-hidden
        className={cn(
          "mt-1.5 size-2 shrink-0 rounded-full",
          unread ? "bg-accent" : "bg-transparent"
        )}
      />
      <div className="min-w-0 flex-1">
        <div
          className={cn(
            "text-[14px]",
            unread ? "font-medium text-fg" : "text-fg-2"
          )}
        >
          {n.subject ?? n.template_key}
        </div>
        {n.body && (
          <p
            className={cn(
              "mt-0.5 text-[13px]",
              unread ? "text-fg-2" : "text-fg-3"
            )}
          >
            {n.body}
          </p>
        )}
      </div>
      <span className="figure shrink-0 text-xs text-fg-3">
        {timeAgo(n.created_at)}
      </span>
      {unread && (
        <Button size="sm" variant="ghost" disabled={busy} onClick={onRead}>
          Mark read
        </Button>
      )}
    </li>
  );
}

function PushPanel() {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  // Runs only in the browser, so the server render never guesses.
  const push = useQuery({
    queryKey: ["push", "this-device"],
    queryFn: async () => {
      const supported = pushSupported();
      const on = supported ? !!(await currentSubscription()) : false;
      return { supported, on };
    },
  });
  const on = !!push.data?.on;

  async function toggle() {
    setBusy(true);
    try {
      if (on) await disablePush();
      else await enablePush();
      qc.setQueryData(["push", "this-device"], { supported: true, on: !on });
      toast.success(on ? "Push is off on this device" : "Push is on");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change push");
    } finally {
      setBusy(false);
    }
  }

  async function test() {
    try {
      await api.testPush();
      toast.success("Test push queued. It arrives in a moment.");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send a test");
    }
  }

  return (
    <Panel>
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <BellRing className="size-4" />
            Browser push
          </span>
        }
        description="Workspace events as system notifications on this device, even with the console closed. Nothing to install."
        action={
          push.data?.supported && (
            <Badge tone={on ? "good" : "neutral"}>{on ? "On" : "Off"}</Badge>
          )
        }
      />
      <div className="px-5 pt-4 pb-5">
        {push.isLoading ? (
          <Skeleton className="h-10 w-56" />
        ) : push.data?.supported ? (
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant={on ? "secondary" : "primary"}
              loading={busy}
              onClick={toggle}
            >
              {!busy && (on ? <BellOff /> : <BellRing />)}
              {on ? "Turn off on this device" : "Turn on for this device"}
            </Button>
            {on && (
              <Button variant="ghost" onClick={test}>
                Send a test push
              </Button>
            )}
          </div>
        ) : (
          <p className="text-[13px] text-fg-3">
            This browser doesn&apos;t support web push.
          </p>
        )}
      </div>
    </Panel>
  );
}
