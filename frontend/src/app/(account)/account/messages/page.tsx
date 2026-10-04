"use client";

// The resident's line to the office: start a conversation, read replies and
// keep it going. The office answers from the console; the resident also
// gets an email. Links open a conversation directly (`?thread=<id>`).

import { Suspense, useState } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, ChevronRight, MessageSquare, Plus } from "lucide-react";
import { toast } from "sonner";
import { api, ApiError, type MessageThread } from "@/lib/api";
import { when } from "@/lib/portal-format";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2.5 text-[14px] text-fg outline-none focus:border-accent";

export default function MessagesPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64" />}>
      <Messages />
    </Suspense>
  );
}

function Messages() {
  const params = useSearchParams();
  const router = useRouter();
  const path = usePathname();
  const thread = params.get("thread");
  const composing = params.get("new") === "1";
  const go = (q: string) => router.push(q ? `${path}?${q}` : path);

  const list = useQuery({
    queryKey: ["my-threads"],
    queryFn: api.myThreads,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });

  if (thread) return <ThreadView id={thread} onBack={() => go("")} />;

  if (list.error) {
    const noLease = list.error instanceof ApiError && list.error.status === 404;
    return (
      <Panel>
        <EmptyState
          icon={<MessageSquare />}
          title={
            noLease ? "No home on file yet" : "Couldn't load your messages"
          }
          description={
            noLease
              ? "Once your lease is set up with this email, you can message the office here."
              : list.error.message
          }
        />
      </Panel>
    );
  }

  if (composing)
    return (
      <NewThread
        onCancel={() => go("")}
        onCreated={(id) => router.replace(`${path}?thread=${id}`)}
      />
    );

  const rows = list.data ?? [];
  return (
    <div className="space-y-6">
      <div className="flex items-end justify-between gap-3">
        <div>
          <h1 className="text-[24px] font-semibold text-fg">Messages</h1>
          <p className="text-[13px] text-fg-3">
            Reach the office. We reply here and by email.
          </p>
        </div>
        <Button onClick={() => go("new=1")}>
          <Plus />
          New message
        </Button>
      </div>

      {list.isLoading && <Skeleton className="h-40" />}

      {list.isSuccess && rows.length === 0 && (
        <Panel>
          <EmptyState
            icon={<MessageSquare />}
            title="No conversations yet"
            description="Questions about your lease, a package, a neighbor? Send the office a message."
          />
        </Panel>
      )}

      {rows.length > 0 && (
        <Panel className="divide-y divide-line">
          {rows.map((t) => (
            <ThreadRow key={t.id} t={t} onOpen={() => go(`thread=${t.id}`)} />
          ))}
        </Panel>
      )}
    </div>
  );
}

function ThreadRow({ t, onOpen }: { t: MessageThread; onOpen: () => void }) {
  const replied = t.status === "open" && t.last_sender_kind === "staff";
  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-fill/50"
    >
      <div className="min-w-0 flex-1">
        <div className="truncate text-[14px] font-medium text-fg">
          {t.subject}
        </div>
        <div className="truncate text-xs text-fg-3">
          {t.last_preview ? `${t.last_preview} · ` : ""}
          {when(t.last_message_at)}
        </div>
      </div>
      {replied ? (
        <Badge tone="accent">New reply</Badge>
      ) : (
        <Badge tone={t.status === "open" ? "info" : "neutral"}>
          {t.status === "open" ? "Open" : "Closed"}
        </Badge>
      )}
      <ChevronRight className="size-4 text-fg-4" />
    </button>
  );
}

function NewThread({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: (id: string) => void;
}) {
  const qc = useQueryClient();
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!subject.trim() || !body.trim()) {
      toast.error("Add a subject and a message.");
      return;
    }
    setBusy(true);
    try {
      const t = await api.createMyThread({
        subject: subject.trim(),
        body: body.trim(),
      });
      toast.success("Sent. The office has been told.");
      void qc.invalidateQueries({ queryKey: ["my-threads"] });
      qc.setQueryData(["my-thread", t.id], t);
      onCreated(t.id);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send it");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <button
        type="button"
        onClick={onCancel}
        className="mb-4 inline-flex items-center gap-1.5 text-[13px] text-fg-3 hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        All messages
      </button>
      <Panel className="p-5">
        <form onSubmit={submit} className="space-y-4">
          <div>
            <h2 className="text-[20px] leading-tight font-semibold text-fg">
              Message the office
            </h2>
            <p className="mt-0.5 text-[13px] text-fg-3">
              For repairs, use Repairs so we can track the fix.
            </p>
          </div>
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-fg-2">
              Subject
            </span>
            <input
              className={field}
              placeholder="e.g. Parking spot for a guest"
              value={subject}
              maxLength={200}
              onChange={(e) => setSubject(e.target.value)}
              autoFocus
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-fg-2">
              Message
            </span>
            <textarea
              className={`${field} min-h-[120px]`}
              value={body}
              onChange={(e) => setBody(e.target.value)}
            />
          </label>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? "Sending…" : "Send"}
            </Button>
          </div>
        </form>
      </Panel>
    </div>
  );
}

function ThreadView({ id, onBack }: { id: string; onBack: () => void }) {
  const qc = useQueryClient();
  // A light poll while the conversation is open so replies show up.
  const q = useQuery({
    queryKey: ["my-thread", id],
    queryFn: () => api.myThread(id),
    refetchInterval: 15000,
  });
  const [reply, setReply] = useState("");
  const [busy, setBusy] = useState(false);

  async function send(e: React.FormEvent) {
    e.preventDefault();
    if (!reply.trim()) return;
    setBusy(true);
    try {
      await api.replyMyThread(id, reply.trim());
      setReply("");
      void qc.invalidateQueries({ queryKey: ["my-thread", id] });
      void qc.invalidateQueries({ queryKey: ["my-threads"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send it");
    } finally {
      setBusy(false);
    }
  }

  const back = (
    <button
      type="button"
      onClick={onBack}
      className="mb-4 inline-flex items-center gap-1.5 text-[13px] text-fg-3 hover:text-fg"
    >
      <ArrowLeft className="size-4" />
      All messages
    </button>
  );

  if (q.error)
    return (
      <div>
        {back}
        <Panel className="p-6 text-[14px] text-fg-2">
          We couldn&apos;t find that conversation.
        </Panel>
      </div>
    );
  const t = q.data;
  if (!t)
    return (
      <div>
        {back}
        <Skeleton className="h-64" />
      </div>
    );

  return (
    <div className="space-y-4">
      {back}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-[20px] leading-tight font-semibold text-fg">
            {t.subject}
          </h1>
          <p className="mt-1 text-[13px] text-fg-3">
            Started {when(t.created_at)}
          </p>
        </div>
        <Badge tone={t.status === "open" ? "info" : "neutral"}>
          {t.status === "open" ? "Open" : "Closed"}
        </Badge>
      </div>

      <Panel className="p-4">
        <ol className="space-y-3">
          {t.messages.map((m) => {
            const mine = m.sender_kind === "resident";
            return (
              <li
                key={m.id}
                className={cn("flex", mine ? "justify-end" : "justify-start")}
              >
                <div
                  className={cn(
                    "max-w-[85%] rounded-2xl px-3.5 py-2.5",
                    mine
                      ? "bg-accent text-accent-fg"
                      : "border border-line bg-surface text-fg"
                  )}
                >
                  <p className="text-[14px] whitespace-pre-wrap">{m.body}</p>
                  <div
                    className={cn(
                      "mt-1 text-[11px]",
                      mine ? "text-accent-fg/75" : "text-fg-3"
                    )}
                  >
                    {mine ? "You" : m.sender_name} · {when(m.created_at)}
                  </div>
                </div>
              </li>
            );
          })}
        </ol>
      </Panel>

      <Panel className="p-4">
        <form onSubmit={send} className="space-y-3">
          <textarea
            className={`${field} min-h-[72px] resize-y`}
            aria-label="Reply"
            placeholder={
              t.status === "closed"
                ? "Replying opens this conversation again"
                : "Write a reply"
            }
            value={reply}
            onChange={(e) => setReply(e.target.value)}
          />
          <div className="flex justify-end">
            <Button type="submit" disabled={busy || !reply.trim()}>
              {busy ? "Sending…" : "Send"}
            </Button>
          </div>
        </form>
      </Panel>
    </div>
  );
}
