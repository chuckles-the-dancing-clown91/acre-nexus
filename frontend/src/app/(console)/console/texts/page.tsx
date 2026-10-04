"use client";

// Two-way texts: one conversation per phone number. Residents texting the
// office number, console replies, and every automatic text (receipts,
// reminders) land in the same thread. STOP always wins: a number that texted
// STOP can't be texted until it replies START. Reading needs `message:read`;
// replying, starting, assigning and closing need `message:manage`.
//
// On a phone the list and the open conversation take turns: `?t=<id>` opens
// one, Back returns to the list.

import { Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Bookmark,
  Check,
  ChevronLeft,
  Copy,
  FlaskConical,
  Image as ImageIcon,
  Inbox,
  Link2,
  MessageSquarePlus,
  MessageSquareText,
  Phone,
  PhoneMissed,
  RotateCcw,
  Send,
  Settings2,
  Trash2,
  User,
} from "lucide-react";
import { toast } from "sonner";
import {
  api,
  type Member,
  type SavedReply,
  type TextMessage,
  type TextThread,
  type TextThreadDetail,
} from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useMembers } from "@/lib/queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { Field, Input, fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

/** How often the inbox and the open conversation refresh. */
const POLL_MS = 15_000;
const MAX_LEN = 1600;

type StatusFilter = "open" | "done" | "all";
const STATUS_TABS = [
  ["open", "Open"],
  ["done", "Done"],
  ["all", "All"],
] as const;

function when(iso: string | null) {
  if (!iso) return "";
  const d = new Date(iso);
  const today = new Date().toDateString() === d.toDateString();
  return today
    ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
    : d.toLocaleDateString([], { month: "short", day: "numeric" });
}

function prettyPhone(p: string) {
  const m = p.match(/^\+1(\d{3})(\d{3})(\d{4})$/);
  return m ? `(${m[1]}) ${m[2]}-${m[3]}` : p;
}

function who(t: TextThread) {
  return t.display_name || prettyPhone(t.phone);
}

function linkWord(t: TextThread): string | null {
  if (t.lease_id) return "resident";
  if (t.lead_id) return "prospect";
  if (t.counterparty_id) return "vendor";
  return null;
}

type LinkKind = "resident" | "lead" | "vendor" | "none";

/** Say whose number this is: a resident's lease, a prospect, a vendor, or
 * nobody, and what to call them. */
function LinkDialog({
  thread,
  onClose,
  onSave,
}: {
  thread: TextThread;
  onClose: () => void;
  onSave: (body: {
    link?: { kind: LinkKind; id?: string };
    display_name?: string;
  }) => Promise<void>;
}) {
  const start: LinkKind = thread.lease_id
    ? "resident"
    : thread.lead_id
      ? "lead"
      : thread.counterparty_id
        ? "vendor"
        : "none";
  const [kind, setKind] = useState<LinkKind>(start);
  const [id, setId] = useState(
    thread.lease_id ?? thread.lead_id ?? thread.counterparty_id ?? ""
  );
  const [name, setName] = useState(thread.display_name ?? "");
  const [q, setQ] = useState("");
  const leases = useQuery({
    queryKey: ["texts", "link", "leases"],
    queryFn: () => api.leases(),
    enabled: kind === "resident",
  });
  const leads = useQuery({
    queryKey: ["texts", "link", "leads"],
    queryFn: () => api.leads(),
    enabled: kind === "lead",
  });
  const vendors = useQuery({
    queryKey: ["texts", "link", "vendors"],
    queryFn: () => api.entities(),
    enabled: kind === "vendor",
  });
  const options: { id: string; label: string; hint: string }[] =
    kind === "resident"
      ? (leases.data ?? []).map((l) => ({
          id: l.id,
          label: l.tenant_name,
          hint: [l.tenant_phone, l.status].filter(Boolean).join(" · "),
        }))
      : kind === "lead"
        ? (leads.data?.leads ?? []).map((l) => ({
            id: l.id,
            label: l.name,
            hint: [l.phone, l.status].filter(Boolean).join(" · "),
          }))
        : kind === "vendor"
          ? (vendors.data ?? []).map((v) => ({
              id: v.id,
              label: v.name,
              hint: [v.phone, v.kind].filter(Boolean).join(" · "),
            }))
          : [];
  const shown = options
    .filter((o) =>
      `${o.label} ${o.hint}`.toLowerCase().includes(q.toLowerCase())
    )
    .slice(0, 40);
  const chosen = options.find((o) => o.id === id);
  const ready = kind === "none" || !!chosen;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Whose number is {prettyPhone(thread.phone)}?
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Link it to the person so their texts sit with their record.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (!ready) return;
            const link =
              kind === start &&
              id ===
                (thread.lease_id ??
                  thread.lead_id ??
                  thread.counterparty_id ??
                  "")
                ? undefined
                : { kind, id: kind === "none" ? undefined : id };
            const display =
              name.trim() !== (thread.display_name ?? "") ||
              (link && kind === "none")
                ? name.trim()
                : undefined;
            void onSave({ link, display_name: display });
          }}
        >
          <Tabs
            tabs={[
              ["resident", "Resident"],
              ["lead", "Prospect"],
              ["vendor", "Vendor"],
              ["none", "Nobody"],
            ]}
            value={kind}
            onChange={(k) => {
              setKind(k);
              setId("");
              setQ("");
            }}
          />
          {kind !== "none" && (
            <>
              <Input
                aria-label="Search"
                placeholder="Search by name or number"
                value={q}
                onChange={(e) => setQ(e.target.value)}
              />
              <ul className="max-h-56 divide-y divide-line overflow-y-auto rounded-xl border border-line">
                {shown.length === 0 && (
                  <li className="px-3 py-4 text-center text-[13px] text-fg-3">
                    Nobody matches.
                  </li>
                )}
                {shown.map((o) => (
                  <li key={o.id}>
                    <button
                      type="button"
                      onClick={() => {
                        setId(o.id);
                        if (!name.trim() || name === chosen?.label)
                          setName(o.label);
                      }}
                      aria-pressed={o.id === id}
                      className={cn(
                        "flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-[13px]",
                        o.id === id ? "bg-accent/10" : "hover:bg-fill"
                      )}
                    >
                      <span className="truncate font-medium text-fg">
                        {o.label}
                      </span>
                      <span className="shrink-0 text-[12px] text-fg-3">
                        {o.hint}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
          <Field label="Call them">
            {(p) => (
              <Input
                {...p}
                placeholder="A name for the inbox"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            )}
          </Field>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={!ready}>
              <Link2 />
              Save
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function errMsg(e: unknown, fallback = "Something went wrong") {
  return e instanceof Error ? e.message : fallback;
}

function copy(text: string) {
  void navigator.clipboard?.writeText(text);
  toast.success("Copied");
}

export default function TextsPage() {
  return (
    <Suspense fallback={<Skeleton className="h-96 rounded-2xl" />}>
      <Texts />
    </Suspense>
  );
}

function Texts() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const read = can("message:read");
  const manage = can("message:manage");
  const ready = scoped && read;
  const params = useSearchParams();
  const router = useRouter();
  const openId = params.get("t");
  const [status, setStatus] = useState<StatusFilter>("open");
  const [mine, setMine] = useState(false);
  const [composing, setComposing] = useState(false);

  const info = useQuery({
    queryKey: ["texts", "status"],
    queryFn: api.textsStatus,
    enabled: ready,
  });
  const threads = useQuery({
    queryKey: ["texts", "threads", status, mine],
    queryFn: () => api.textThreads(status === "all" ? undefined : status, mine),
    enabled: ready,
    refetchInterval: POLL_MS,
  });
  const replies = useQuery({
    queryKey: ["texts", "replies"],
    queryFn: api.savedReplies,
    enabled: ready,
  });
  const members = useMembers({ enabled: ready });
  const active = useMemo(
    () => (members.data ?? []).filter((m) => m.status === "active"),
    [members.data]
  );
  const names = useMemo(
    () => new Map((members.data ?? []).map((m) => [m.user_id, m.name])),
    [members.data]
  );

  const open = (id: string | null) =>
    router.push(id ? `/console/texts?t=${id}` : "/console/texts", {
      scroll: false,
    });

  if (!read) {
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Messaging" title="Texts" />
        <Panel>
          <EmptyState
            icon={<MessageSquareText />}
            title="You don't have access to texts"
            description="Ask an admin for the message:read permission."
          />
        </Panel>
      </div>
    );
  }

  const testMode = !!info.data && !info.data.live;
  const list = threads.data ?? [];
  const unread = info.data?.unread_threads ?? 0;

  return (
    <div className="space-y-6">
      <div className={cn(openId && "hidden lg:block")}>
        <PageHeader
          eyebrow="Messaging"
          title="Texts"
          description={
            unread > 0
              ? `${unread} conversation${unread === 1 ? "" : "s"} with unread texts. Replies, reminders and receipts sit together.`
              : "Every text with residents in one inbox. Replies, reminders and receipts sit together."
          }
          actions={
            manage && (
              <Button onClick={() => setComposing(true)}>
                <MessageSquarePlus />
                New text
              </Button>
            )
          }
        />
      </div>

      {testMode && info.data && (
        <Panel
          className={cn(
            "border-warn/30 bg-warn/5 p-4 text-[13px] text-fg-2",
            openId && "hidden lg:block"
          )}
        >
          <div className="flex items-start gap-3">
            <FlaskConical className="mt-0.5 size-4 shrink-0 text-warn" />
            <div className="min-w-0 space-y-2">
              <p>
                <span className="font-semibold text-warn">Test mode.</span>{" "}
                Texts are logged here but not really sent. Use{" "}
                <span className="font-medium text-fg">
                  Pretend they texted back
                </span>{" "}
                in a conversation to try replies and STOP. To go live, add a
                Twilio provider in Integrations, set{" "}
                <code className="rounded bg-fill px-1 font-mono text-[12px]">
                  LIVE_PROVIDERS=sms
                </code>
                , and point the number&apos;s webhooks at these addresses.
              </p>
              <WebhookLine
                label="A message comes in"
                url={info.data.inbound_webhook_url}
              />
              <WebhookLine
                label="Status callback"
                url={info.data.status_webhook_url}
              />
              <WebhookLine
                label="A call comes in"
                url={info.data.voice_webhook_url}
              />
            </div>
          </div>
        </Panel>
      )}

      <div className="grid gap-4 lg:grid-cols-[340px_minmax(0,1fr)]">
        <Panel
          className={cn(
            "flex flex-col overflow-hidden lg:h-[calc(100dvh-14rem)] lg:min-h-[520px]",
            openId && "hidden lg:flex"
          )}
        >
          <div className="flex items-center gap-2 border-b border-line p-3">
            <Tabs
              tabs={STATUS_TABS}
              value={status}
              onChange={setStatus}
              className="flex-1"
            />
            <button
              type="button"
              aria-pressed={mine}
              onClick={() => setMine(!mine)}
              className={cn(
                "flex h-9 items-center gap-1.5 rounded-xl border px-3 text-[13px] font-medium transition",
                mine
                  ? "border-accent bg-accent/10 text-accent"
                  : "border-line text-fg-3 hover:text-fg"
              )}
            >
              <User className="size-3.5" />
              Mine
            </button>
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
                Couldn&apos;t load texts: {threads.error.message}
              </p>
            )}
            {threads.data && list.length === 0 && (
              <EmptyState
                icon={<Inbox />}
                title={
                  status === "open"
                    ? "Nothing waiting on you"
                    : "No conversations yet"
                }
                description={
                  mine ? "Only conversations assigned to you show." : undefined
                }
              />
            )}
            <ul className="divide-y divide-line">
              {list.map((t) => (
                <li key={t.id}>
                  <ThreadRow
                    t={t}
                    active={openId === t.id}
                    assignee={
                      t.assigned_user_id
                        ? names.get(t.assigned_user_id)
                        : undefined
                    }
                    onOpen={() => open(t.id)}
                  />
                </li>
              ))}
            </ul>
          </div>
        </Panel>

        <div className={cn("min-w-0", !openId && "hidden lg:block")}>
          {openId ? (
            <Conversation
              key={openId}
              id={openId}
              manage={manage}
              testMode={testMode}
              members={active}
              replies={replies.data ?? []}
              onBack={() => open(null)}
            />
          ) : (
            <Panel className="flex h-[calc(100dvh-14rem)] min-h-[520px] items-center justify-center">
              <EmptyState
                icon={<MessageSquareText />}
                title="Pick a conversation"
                description="Texts refresh on their own every few seconds."
              />
            </Panel>
          )}
        </div>
      </div>

      {composing && (
        <NewTextDialog
          onClose={() => setComposing(false)}
          onSent={(d) => {
            setComposing(false);
            open(d.thread.id);
          }}
        />
      )}
    </div>
  );
}

function WebhookLine({ label, url }: { label: string; url: string }) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-[12px] text-fg-3">{label}</span>
      <code className="min-w-0 rounded bg-fill px-1.5 py-0.5 font-mono text-[12px] break-all text-fg">
        {url}
      </code>
      <Button
        size="sm"
        variant="ghost"
        onClick={() => copy(url)}
        aria-label={`Copy ${label} address`}
      >
        <Copy />
      </Button>
    </div>
  );
}

function ThreadRow({
  t,
  active,
  assignee,
  onOpen,
}: {
  t: TextThread;
  active: boolean;
  assignee?: string;
  onOpen: () => void;
}) {
  const unread = t.unread_count > 0;
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
            unread ? "font-semibold" : "font-medium"
          )}
        >
          {who(t)}
        </span>
        <span className="figure shrink-0 text-xs text-fg-3">
          {when(t.last_message_at)}
        </span>
      </div>
      <div className="mt-0.5 flex items-center gap-2">
        <span
          className={cn(
            "min-w-0 flex-1 truncate text-[13px]",
            unread ? "text-fg-2" : "text-fg-3"
          )}
        >
          {t.last_preview ?? "No texts yet"}
        </span>
        {t.opted_out && <Badge tone="bad">STOP</Badge>}
        {t.status === "done" && <Badge>done</Badge>}
        {unread && (
          <span className="figure rounded-full bg-accent px-2 py-0.5 text-[11px] font-semibold text-accent-fg">
            {t.unread_count}
          </span>
        )}
      </div>
      {assignee && (
        <div className="mt-1 flex items-center gap-1 text-[11px] text-fg-4">
          <User className="size-3" />
          {assignee}
        </div>
      )}
    </button>
  );
}

function Conversation({
  id,
  manage,
  testMode,
  members,
  replies,
  onBack,
}: {
  id: string;
  manage: boolean;
  testMode: boolean;
  members: Member[];
  replies: SavedReply[];
  onBack: () => void;
}) {
  const qc = useQueryClient();
  const key = ["texts", "thread", id];
  const detail = useQuery({
    queryKey: key,
    queryFn: async () => {
      // Reading a conversation marks it read, so the list's counts move.
      const d = await api.textThread(id);
      void qc.invalidateQueries({ queryKey: ["texts", "threads"] });
      void qc.invalidateQueries({ queryKey: ["texts", "status"] });
      return d;
    },
    refetchInterval: POLL_MS,
  });
  const [reply, setReply] = useState("");
  const [pretend, setPretend] = useState("");
  const [linking, setLinking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [managing, setManaging] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);
  const count = detail.data?.messages.length ?? 0;

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [count]);

  async function run(
    fn: () => Promise<TextThreadDetail | TextThread>
  ): Promise<boolean> {
    setBusy(true);
    try {
      const res = await fn();
      if ("messages" in res) {
        qc.setQueryData<TextThreadDetail>(key, res);
      } else {
        qc.setQueryData<TextThreadDetail>(key, (old) =>
          old ? { ...old, thread: res } : old
        );
      }
      void qc.invalidateQueries({ queryKey: ["texts", "threads"] });
      return true;
    } catch (e) {
      toast.error(errMsg(e));
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function send() {
    const body = reply.trim();
    if (!body) return;
    if (await run(() => api.replyText(id, body))) setReply("");
  }

  if (detail.isLoading) {
    return <Skeleton className="h-[calc(100dvh-14rem)] min-h-[520px]" />;
  }
  if (detail.error || !detail.data) {
    return (
      <Panel className="p-6">
        <Button size="sm" variant="ghost" onClick={onBack} className="mb-3">
          <ChevronLeft />
          All texts
        </Button>
        <p className="text-[13px] text-bad">
          Couldn&apos;t open this conversation
          {detail.error ? `: ${detail.error.message}` : "."}
        </p>
      </Panel>
    );
  }
  const t = detail.data.thread;

  return (
    <Panel className="flex h-[calc(100dvh-9rem)] min-h-[480px] flex-col lg:h-[calc(100dvh-14rem)] lg:min-h-[520px]">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-3 py-3 sm:px-5">
        <Button
          size="icon"
          variant="ghost"
          onClick={onBack}
          aria-label="Back to all texts"
          className="lg:hidden"
        >
          <ChevronLeft />
        </Button>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="truncate text-[16px] font-semibold text-fg">
              {who(t)}
            </span>
            {t.opted_out && <Badge tone="bad">Texted STOP</Badge>}
            {t.status === "done" && <Badge>done</Badge>}
          </div>
          {(t.display_name || linkWord(t)) && (
            <div className="truncate text-xs text-fg-3">
              {t.display_name && prettyPhone(t.phone)}
              {t.display_name && linkWord(t) && " · "}
              {linkWord(t)}
            </div>
          )}
        </div>
        <Button size="icon" variant="ghost" asChild>
          <a href={`tel:${t.phone}`} aria-label={`Call ${who(t)}`}>
            <Phone />
          </a>
        </Button>
        {manage && (
          <div className="flex w-full flex-wrap items-center gap-2 sm:w-auto">
            <select
              aria-label="Assigned to"
              disabled={busy}
              value={t.assigned_user_id ?? ""}
              onChange={(e) =>
                run(() =>
                  api.updateTextThread(t.id, { assignee: e.target.value })
                )
              }
              className={cn(fieldClass, "min-w-0 flex-1 py-1.5 sm:flex-none")}
            >
              <option value="">Unassigned</option>
              {members.map((m) => (
                <option key={m.user_id} value={m.user_id}>
                  {m.name}
                </option>
              ))}
            </select>
            <label
              className="flex items-center gap-1.5 text-xs text-fg-2"
              title="They agreed to get promotional texts. STOP still wins."
            >
              <input
                type="checkbox"
                disabled={busy || t.opted_out}
                checked={t.marketing_consent}
                onChange={(e) =>
                  run(() =>
                    api.updateTextThread(t.id, {
                      marketing_consent: e.target.checked,
                    })
                  )
                }
                className="size-4 accent-[var(--accent)]"
              />
              Marketing OK
            </label>
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => setLinking(true)}
            >
              <Link2 />
              Whose number
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() =>
                run(() =>
                  api.updateTextThread(t.id, {
                    status: t.status === "open" ? "done" : "open",
                  })
                )
              }
            >
              {t.status === "open" ? <Check /> : <RotateCcw />}
              {t.status === "open" ? "Mark done" : "Reopen"}
            </Button>
          </div>
        )}
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-3 py-4 sm:px-5">
        {detail.data.messages.length === 0 && (
          <p className="py-10 text-center text-[13px] text-fg-3">
            No texts in this conversation yet.
          </p>
        )}
        {detail.data.messages.map((m) => (
          <Bubble key={m.id} m={m} />
        ))}
        <div ref={bottom} />
      </div>

      {manage && (
        <div className="space-y-2 border-t border-line p-3 sm:p-4">
          {t.opted_out ? (
            <p className="text-[13px] text-fg-3">
              This number texted STOP. You can&apos;t text it until they reply
              START.
            </p>
          ) : (
            <>
              <div className="flex flex-wrap items-center gap-1">
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button size="sm" variant="ghost">
                      <Bookmark />
                      Saved replies
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start" className="w-72">
                    {replies.length === 0 ? (
                      <DropdownMenuLabel>
                        No saved replies yet
                      </DropdownMenuLabel>
                    ) : (
                      <>
                        <DropdownMenuLabel>Insert a reply</DropdownMenuLabel>
                        {replies.map((r) => (
                          <DropdownMenuItem
                            key={r.id}
                            onSelect={() =>
                              setReply((cur) =>
                                cur.trim() ? `${cur} ${r.body}` : r.body
                              )
                            }
                          >
                            <span className="min-w-0">
                              <span className="block truncate font-medium text-fg">
                                {r.title}
                              </span>
                              <span className="block truncate text-xs text-fg-3">
                                {r.body}
                              </span>
                            </span>
                          </DropdownMenuItem>
                        ))}
                      </>
                    )}
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      disabled={!reply.trim()}
                      onSelect={() => setSaving(true)}
                    >
                      <Bookmark />
                      Save this text as a reply
                    </DropdownMenuItem>
                    {replies.length > 0 && (
                      <DropdownMenuItem onSelect={() => setManaging(true)}>
                        <Settings2 />
                        Manage saved replies
                      </DropdownMenuItem>
                    )}
                  </DropdownMenuContent>
                </DropdownMenu>
                <span className="figure ml-auto text-[11px] text-fg-4">
                  {reply.length}/{MAX_LEN}
                </span>
              </div>
              <form
                className="flex items-end gap-2"
                onSubmit={(e) => {
                  e.preventDefault();
                  void send();
                }}
              >
                <textarea
                  aria-label="Write a text"
                  className={cn(
                    fieldClass,
                    "max-h-40 min-h-[44px] flex-1 resize-y text-[14px]"
                  )}
                  rows={2}
                  placeholder="Write a text"
                  value={reply}
                  maxLength={MAX_LEN}
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
            </>
          )}
          {testMode && (
            <form
              className="flex gap-2"
              onSubmit={async (e) => {
                e.preventDefault();
                const body = pretend.trim();
                if (!body) return;
                const ok = await run(() =>
                  api
                    .simulateText(t.phone, body)
                    // Re-read so the open conversation counts as read.
                    .then(() => api.textThread(t.id))
                );
                if (ok) setPretend("");
              }}
            >
              <input
                aria-label="Pretend they texted back"
                className={cn(fieldClass, "min-w-0 flex-1 border-dashed")}
                placeholder="Pretend they texted back (try STOP or START)"
                value={pretend}
                onChange={(e) => setPretend(e.target.value)}
              />
              <Button
                type="submit"
                size="sm"
                variant="secondary"
                className="h-auto border-dashed"
                disabled={busy || !pretend.trim()}
              >
                <FlaskConical />
                Receive
              </Button>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                className="h-auto border-dashed"
                disabled={busy}
                onClick={() =>
                  void run(() =>
                    api.simulateCall(t.phone).then((r) => {
                      toast.success(
                        r.texted_back
                          ? "Missed call filed and texted back"
                          : "Missed call filed; no text this time"
                      );
                      return api.textThread(t.id);
                    })
                  )
                }
              >
                <PhoneMissed />
                Pretend they called
              </Button>
            </form>
          )}
        </div>
      )}

      {saving && (
        <SaveReplyDialog body={reply.trim()} onClose={() => setSaving(false)} />
      )}
      {linking && t && (
        <LinkDialog
          thread={t}
          onClose={() => setLinking(false)}
          onSave={async (body) => {
            const ok = await run(() => api.updateTextThread(t.id, body));
            if (ok) setLinking(false);
          }}
        />
      )}
      {managing && (
        <ManageRepliesDialog
          replies={replies}
          onClose={() => setManaging(false)}
        />
      )}
    </Panel>
  );
}

function Bubble({ m }: { m: TextMessage }) {
  if (m.status === "missed_call") {
    return (
      <div className="flex justify-center">
        <span className="inline-flex items-center gap-1.5 rounded-full border border-line bg-fill px-3 py-1 text-[12px] text-fg-3">
          <PhoneMissed className="size-3.5 text-warn" />
          Missed call · {when(m.created_at)}
        </span>
      </div>
    );
  }
  const out = m.direction === "out";
  const dim = m.status === "failed" || m.status === "blocked";
  return (
    <div className={cn("flex", out ? "justify-end" : "justify-start")}>
      <div className="max-w-[85%] sm:max-w-[75%]">
        {m.body && (
          <div
            className={cn(
              "rounded-2xl px-3.5 py-2 text-[14px] whitespace-pre-wrap",
              out
                ? "rounded-br-md bg-accent text-accent-fg"
                : "rounded-bl-md bg-fill-2 text-fg",
              dim && "opacity-60"
            )}
          >
            {m.body}
          </div>
        )}
        {m.media.length > 0 && (
          <div
            className={cn("mt-1 flex flex-wrap gap-1", out && "justify-end")}
          >
            {m.media.map((p, i) => (
              <Button
                key={p.document_id}
                size="sm"
                variant="secondary"
                onClick={() =>
                  api
                    .documentDownloadUrl(p.document_id)
                    .then((d) => window.open(d.url, "_blank", "noopener"))
                    .catch((e) => toast.error(errMsg(e, "Couldn't open it")))
                }
              >
                <ImageIcon />
                Photo {i + 1}
              </Button>
            ))}
          </div>
        )}
        <div
          className={cn("mt-0.5 text-[11px] text-fg-4", out && "text-right")}
        >
          {when(m.created_at)}
          {out &&
            ` · ${m.sent_by ?? (m.template_key ? "automatic" : "office")}`}
          {m.status === "queued" && " · sending"}
          {m.status === "failed" && (
            <span className="text-bad">
              {" "}
              · failed{m.error ? `: ${m.error}` : ""}
            </span>
          )}
          {m.status === "blocked" && (
            <span className="text-bad"> · not sent (STOP)</span>
          )}
          {m.media_count > 0 &&
            m.media.length === 0 &&
            ` · ${m.media_count} photo${m.media_count === 1 ? "" : "s"}, filing`}
        </div>
      </div>
    </div>
  );
}

function SaveReplyDialog({
  body,
  onClose,
}: {
  body: string;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  async function save() {
    if (!title.trim()) return;
    setBusy(true);
    try {
      await api.saveReply(title.trim(), body);
      toast.success("Reply saved");
      void qc.invalidateQueries({ queryKey: ["texts", "replies"] });
      onClose();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save it"));
      setBusy(false);
    }
  }
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Save as a reply
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Everyone on the team can insert it from Saved replies.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <Field label="Name">
            {(p) => (
              <Input
                {...p}
                autoFocus
                placeholder="e.g. Rent reminder"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
              />
            )}
          </Field>
          <p className="rounded-xl border border-line bg-fill px-3 py-2 text-[13px] whitespace-pre-wrap text-fg-2">
            {body}
          </p>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !title.trim()}>
              <Bookmark />
              Save
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ManageRepliesDialog({
  replies,
  onClose,
}: {
  replies: SavedReply[];
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState<string | null>(null);
  async function remove(r: SavedReply) {
    if (!window.confirm(`Delete "${r.title}"?`)) return;
    setBusy(r.id);
    try {
      await api.deleteReply(r.id);
      toast.success("Reply deleted");
      await qc.invalidateQueries({ queryKey: ["texts", "replies"] });
    } catch (e) {
      toast.error(errMsg(e, "Couldn't delete it"));
    } finally {
      setBusy(null);
    }
  }
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Saved replies
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Shared with everyone who answers texts.
        </DialogDescription>
        <ul className="mt-4 max-h-[60dvh] divide-y divide-line overflow-y-auto rounded-xl border border-line">
          {replies.length === 0 && (
            <li className="px-3 py-6 text-center text-[13px] text-fg-3">
              No saved replies.
            </li>
          )}
          {replies.map((r) => (
            <li key={r.id} className="flex items-start gap-3 px-3 py-2.5">
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-fg">
                  {r.title}
                </div>
                <div className="line-clamp-2 text-xs text-fg-3">{r.body}</div>
              </div>
              <Button
                size="sm"
                variant="ghost"
                disabled={busy === r.id}
                onClick={() => remove(r)}
                aria-label={`Delete ${r.title}`}
              >
                <Trash2 />
              </Button>
            </li>
          ))}
        </ul>
        <div className="mt-4 flex justify-end">
          <Button variant="secondary" onClick={onClose}>
            Done
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function NewTextDialog({
  onClose,
  onSent,
}: {
  onClose: () => void;
  onSent: (d: TextThreadDetail) => void;
}) {
  const qc = useQueryClient();
  const [phone, setPhone] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit() {
    setBusy(true);
    try {
      const d = await api.startText(phone.trim(), body.trim());
      qc.setQueryData(["texts", "thread", d.thread.id], d);
      void qc.invalidateQueries({ queryKey: ["texts", "threads"] });
      toast.success("Text sent");
      onSent(d);
    } catch (e) {
      toast.error(errMsg(e, "Couldn't send the text"));
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          New text
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Text a number from the office line. A number that texted STOP
          can&apos;t be texted.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <Field label="Phone number">
            {(p) => (
              <Input
                {...p}
                type="tel"
                inputMode="tel"
                autoFocus
                placeholder="(760) 555-1234"
                value={phone}
                onChange={(e) => setPhone(e.target.value)}
              />
            )}
          </Field>
          <Field label="Message" hint={`${body.length}/${MAX_LEN}`}>
            {(p) => (
              <textarea
                {...p}
                className={cn(fieldClass, "min-h-[120px] w-full text-[14px]")}
                maxLength={MAX_LEN}
                value={body}
                onChange={(e) => setBody(e.target.value)}
              />
            )}
          </Field>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button
              type="submit"
              loading={busy}
              disabled={!phone.trim() || !body.trim()}
            >
              {!busy && <Send />}
              Send
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
