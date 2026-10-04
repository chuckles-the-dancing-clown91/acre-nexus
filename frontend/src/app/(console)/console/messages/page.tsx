"use client";

// Resident messages: every resident and manager thread from the portal,
// filterable by status, with the conversation beside the list (one after the
// other on a phone). Reading needs `message:read`; replying and closing need
// `message:manage`.

import { Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ChevronLeft,
  Inbox,
  Lock,
  MessagesSquare,
  RotateCcw,
  Search,
  Send,
} from "lucide-react";
import { toast } from "sonner";
import { api, type MessageThread } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { Input, fieldClass } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

type StatusFilter = "all" | "open" | "closed";
const STATUS_TABS = [
  ["all", "All"],
  ["open", "Open"],
  ["closed", "Closed"],
] as const;

function when(iso: string) {
  const d = new Date(iso);
  const today = new Date().toDateString() === d.toDateString();
  return today
    ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
    : d.toLocaleDateString([], { month: "short", day: "numeric" });
}

function stamp(iso: string) {
  return new Date(iso).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export default function MessagesPage() {
  return (
    <Suspense fallback={<Skeleton className="h-96 rounded-2xl" />}>
      <Messages />
    </Suspense>
  );
}

function Messages() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const read = can("message:read");
  const manage = can("message:manage");
  const params = useSearchParams();
  const router = useRouter();
  const openId = params.get("t");
  const [status, setStatus] = useState<StatusFilter>("all");
  const [q, setQ] = useState("");

  const threads = useQuery({
    queryKey: ["messages", "threads", status],
    queryFn: () => api.messageThreads(status === "all" ? undefined : status),
    enabled: scoped && read,
  });
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (threads.data ?? []).filter(
      (t) =>
        !needle ||
        t.subject.toLowerCase().includes(needle) ||
        (t.resident_name ?? "").toLowerCase().includes(needle) ||
        (t.property_address ?? "").toLowerCase().includes(needle)
    );
  }, [threads.data, q]);
  const awaiting = (threads.data ?? []).filter(
    (t) => t.status === "open" && t.last_sender_kind === "resident"
  ).length;

  const open = (id: string | null) =>
    router.push(id ? `/console/messages?t=${id}` : "/console/messages", {
      scroll: false,
    });

  if (!read) {
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Messaging" title="Messages" />
        <Panel>
          <EmptyState
            icon={<MessagesSquare />}
            title="You don't have access to resident messages"
            description="Ask an admin for the message:read permission."
          />
        </Panel>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className={cn(openId && "hidden lg:block")}>
        <PageHeader
          eyebrow="Messaging"
          title="Messages"
          description={
            awaiting > 0
              ? `Resident conversations across the portfolio. ${awaiting} waiting on a reply.`
              : "Resident conversations across the portfolio."
          }
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-[380px_minmax(0,1fr)]">
        <Panel
          className={cn(
            "flex flex-col overflow-hidden lg:h-[calc(100dvh-13rem)] lg:min-h-[520px]",
            openId && "hidden lg:flex"
          )}
        >
          <div className="space-y-2 border-b border-line p-3">
            <Tabs tabs={STATUS_TABS} value={status} onChange={setStatus} />
            <div className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Search subjects, residents, addresses"
                aria-label="Search messages"
                className="h-10 pl-9"
              />
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {threads.isLoading && (
              <div className="space-y-2 p-3">
                {Array.from({ length: 6 }, (_, i) => (
                  <Skeleton key={i} className="h-14" />
                ))}
              </div>
            )}
            {threads.error && (
              <p className="p-4 text-[13px] text-bad">
                Couldn&apos;t load messages: {threads.error.message}
              </p>
            )}
            {threads.data && rows.length === 0 && (
              <EmptyState
                icon={<Inbox />}
                title={
                  q.trim()
                    ? "Nothing matches that search"
                    : status === "all"
                      ? "No resident messages"
                      : `No ${status} messages`
                }
              />
            )}
            <ul className="divide-y divide-line">
              {rows.map((t) => (
                <li key={t.id}>
                  <ThreadRow
                    t={t}
                    active={openId === t.id}
                    onOpen={() => open(t.id)}
                  />
                </li>
              ))}
            </ul>
          </div>
        </Panel>

        <div className={cn("min-w-0", !openId && "hidden lg:block")}>
          {openId ? (
            <ThreadView
              key={openId}
              id={openId}
              manage={manage}
              onBack={() => open(null)}
            />
          ) : (
            <Panel className="flex h-[calc(100dvh-13rem)] min-h-[520px] items-center justify-center">
              <EmptyState
                icon={<MessagesSquare />}
                title="Pick a conversation"
                description="Residents write from the portal; you answer here."
              />
            </Panel>
          )}
        </div>
      </div>
    </div>
  );
}

function ThreadRow({
  t,
  active,
  onOpen,
}: {
  t: MessageThread;
  active: boolean;
  onOpen: () => void;
}) {
  const waiting = t.status === "open" && t.last_sender_kind === "resident";
  return (
    <button
      type="button"
      onClick={onOpen}
      aria-current={active ? "true" : undefined}
      className={cn(
        "block w-full px-4 py-3 text-left transition hover:bg-fill-2",
        active && "bg-fill-2"
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <span
          className={cn(
            "truncate text-[14px] text-fg",
            waiting ? "font-semibold" : "font-medium"
          )}
        >
          {t.subject}
        </span>
        <span className="figure shrink-0 text-xs text-fg-3">
          {when(t.last_message_at)}
        </span>
      </div>
      <div className="truncate text-xs text-fg-3">
        {t.resident_name ?? "Resident"}
        {t.property_address ? ` · ${t.property_address}` : ""}
      </div>
      <div className="mt-1 flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-[13px] text-fg-2">
          {t.last_preview ?? ""}
        </span>
        {waiting && <Badge tone="accent">awaiting reply</Badge>}
        {t.status === "closed" && <Badge>closed</Badge>}
      </div>
    </button>
  );
}

function ThreadView({
  id,
  manage,
  onBack,
}: {
  id: string;
  manage: boolean;
  onBack: () => void;
}) {
  const qc = useQueryClient();
  const detail = useQuery({
    queryKey: ["messages", "thread", id],
    queryFn: () => api.messageThread(id),
  });
  const [reply, setReply] = useState("");
  const [busy, setBusy] = useState(false);
  const bottom = useRef<HTMLLIElement>(null);
  const count = detail.data?.messages.length ?? 0;

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [count]);

  async function run(fn: () => Promise<unknown>): Promise<boolean> {
    setBusy(true);
    try {
      await fn();
      await qc.invalidateQueries({ queryKey: ["messages"] });
      return true;
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function send() {
    const body = reply.trim();
    if (!body) return;
    if (await run(() => api.replyMessageThread(id, body))) setReply("");
  }

  if (detail.isLoading) {
    return <Skeleton className="h-[calc(100dvh-13rem)] min-h-[520px]" />;
  }
  if (detail.error || !detail.data) {
    return (
      <Panel className="p-6">
        <Button size="sm" variant="ghost" onClick={onBack} className="mb-3">
          <ChevronLeft />
          All messages
        </Button>
        <p className="text-[13px] text-bad">
          Couldn&apos;t open this conversation
          {detail.error ? `: ${detail.error.message}` : "."}
        </p>
      </Panel>
    );
  }
  const d = detail.data;

  return (
    <Panel className="flex h-[calc(100dvh-9rem)] min-h-[480px] flex-col lg:h-[calc(100dvh-13rem)] lg:min-h-[520px]">
      <div className="flex items-center gap-3 border-b border-line px-3 py-3 sm:px-5">
        <Button
          size="icon"
          variant="ghost"
          onClick={onBack}
          aria-label="Back to all messages"
          className="lg:hidden"
        >
          <ChevronLeft />
        </Button>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="truncate text-[16px] font-semibold text-fg">
              {d.subject}
            </span>
            <Badge tone={d.status === "open" ? "info" : "neutral"}>
              {d.status}
            </Badge>
          </div>
          <div className="truncate text-xs text-fg-3">
            {d.resident_name ?? "Resident"}
            {d.property_address ? ` · ${d.property_address}` : ""} ·{" "}
            {d.message_count} message{d.message_count === 1 ? "" : "s"}
          </div>
        </div>
        {manage && (
          <Button
            size="sm"
            variant="secondary"
            disabled={busy}
            onClick={() =>
              run(() =>
                api.updateMessageThread(
                  id,
                  d.status === "open" ? "closed" : "open"
                )
              )
            }
          >
            {d.status === "open" ? <Lock /> : <RotateCcw />}
            {d.status === "open" ? "Close" : "Reopen"}
          </Button>
        )}
      </div>

      <ul className="min-h-0 flex-1 space-y-3 overflow-y-auto px-3 py-4 sm:px-5">
        {d.messages.map((m) => {
          const staff = m.sender_kind === "staff";
          return (
            <li
              key={m.id}
              className={cn("flex", staff ? "justify-end" : "justify-start")}
            >
              <div className="max-w-[85%] sm:max-w-[75%]">
                <div
                  className={cn(
                    "rounded-2xl px-3.5 py-2 text-[14px] whitespace-pre-wrap",
                    staff
                      ? "rounded-br-md bg-accent text-accent-fg"
                      : "rounded-bl-md bg-fill-2 text-fg"
                  )}
                >
                  {m.body}
                </div>
                <div
                  className={cn(
                    "mt-0.5 text-[11px] text-fg-4",
                    staff && "text-right"
                  )}
                >
                  {m.sender_name} · {stamp(m.created_at)}
                </div>
              </div>
            </li>
          );
        })}
        <li ref={bottom} aria-hidden />
      </ul>

      {manage && (
        <form
          className="flex items-end gap-2 border-t border-line p-3 sm:p-4"
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
        >
          <textarea
            aria-label="Reply to the resident"
            className={cn(
              fieldClass,
              "max-h-40 min-h-[44px] flex-1 resize-y text-[14px]"
            )}
            rows={2}
            placeholder="Reply to the resident"
            value={reply}
            onChange={(e) => setReply(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                e.currentTarget.form?.requestSubmit();
              }
            }}
          />
          <Button type="submit" disabled={busy || !reply.trim()}>
            <Send />
            <span className="hidden sm:inline">Send</span>
          </Button>
        </form>
      )}
    </Panel>
  );
}
