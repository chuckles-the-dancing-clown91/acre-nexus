"use client";

// Resident messaging console: every resident ↔ manager thread, filterable by
// status, with an inline conversation view. Gated by `message:read`; replying
// and closing need `message:manage`.

import { useCallback, useEffect, useState } from "react";
import { api, type MessageThread, type MessageThreadDetail } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { toast } from "sonner";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

function fmtWhen(iso: string) {
  return iso.slice(0, 16).replace("T", " ");
}

export default function MessagesPage() {
  const { can } = useAuth();
  const manage = can("message:manage");
  const [threads, setThreads] = useState<MessageThread[]>([]);
  const [status, setStatus] = useState("");
  const [openId, setOpenId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    api
      .messageThreads(status || undefined)
      .then(setThreads)
      .catch((e) => setError(e.message));
  }, [status]);

  useEffect(() => {
    if (!can("message:read")) return;
    reload();
  }, [reload, can]);

  if (!can("message:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to resident messages. Ask an admin for the{" "}
          <span className="font-mono">message:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Messages
          </h1>
          <p className="text-ink-3">
            Resident conversations across the portfolio.
          </p>
        </div>
        <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
          Status
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className="rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink"
          >
            <option value="">All</option>
            <option value="open">Open</option>
            <option value="closed">Closed</option>
          </select>
        </label>
      </div>

      {error && (
        <Card className="p-4 text-sm text-bad">
          <p role="alert">{error}</p>
        </Card>
      )}

      <Card>
        {threads.length === 0 ? (
          <div className="px-5 py-6 text-sm text-ink-3">
            No resident messages{status ? ` (${status})` : ""}.
          </div>
        ) : (
          <ul className="divide-y divide-line">
            {threads.map((t) => (
              <li key={t.id} className="px-5 py-3">
                <button
                  className="flex w-full flex-wrap items-center justify-between gap-3 text-left"
                  onClick={() => setOpenId(openId === t.id ? null : t.id)}
                >
                  <div className="min-w-0">
                    <div className="truncate font-semibold">{t.subject}</div>
                    <div className="truncate text-xs text-ink-3">
                      {t.resident_name ?? "Resident"}
                      {t.property_address
                        ? ` · ${t.property_address}`
                        : ""} · {fmtWhen(t.last_message_at)}
                      {t.last_sender_kind === "resident" && (
                        <span className="ml-1 font-semibold text-accent">
                          awaiting reply
                        </span>
                      )}
                    </div>
                  </div>
                  <Badge tone={t.status === "open" ? "info" : "neutral"}>
                    {t.status}
                  </Badge>
                </button>
                {openId === t.id && (
                  <ThreadView
                    threadId={t.id}
                    manage={manage}
                    onChange={reload}
                  />
                )}
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

function ThreadView({
  threadId,
  manage,
  onChange,
}: {
  threadId: string;
  manage: boolean;
  onChange: () => void;
}) {
  const [detail, setDetail] = useState<MessageThreadDetail | null>(null);
  const [reply, setReply] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setDetail(await api.messageThread(threadId));
    } catch {
      // The list row already shows the thread; the drawer just stays empty.
    }
  }, [threadId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function run(fn: () => Promise<unknown>) {
    setBusy(true);
    try {
      await fn();
      await load();
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  async function send(e: React.FormEvent) {
    e.preventDefault();
    if (!reply.trim()) return;
    await run(async () => {
      await api.replyMessageThread(threadId, reply.trim());
      setReply("");
    });
  }

  if (!detail) return <div className="py-3 text-sm text-ink-3">Loading…</div>;

  return (
    <div className="mt-3 space-y-3 rounded-xl border border-line p-4 text-sm">
      <ul className="space-y-2">
        {detail.messages.map((m) => (
          <li
            key={m.id}
            className={
              m.sender_kind === "staff"
                ? "ml-8 rounded-xl bg-accent-soft px-3 py-2"
                : "mr-8 rounded-xl bg-surface-2 px-3 py-2"
            }
          >
            <div className="text-xs text-ink-3">
              {m.sender_name} · {fmtWhen(m.created_at)}
            </div>
            <div className="whitespace-pre-wrap">{m.body}</div>
          </li>
        ))}
      </ul>
      {manage && (
        <form onSubmit={send} className="flex gap-2">
          <input
            className={field}
            placeholder="Reply to the resident…"
            value={reply}
            onChange={(e) => setReply(e.target.value)}
          />
          <Button type="submit" disabled={busy || !reply.trim()}>
            Send
          </Button>
          <Button
            variant="outline"
            type="button"
            disabled={busy}
            onClick={() =>
              void run(() =>
                api.updateMessageThread(
                  threadId,
                  detail.status === "open" ? "closed" : "open"
                )
              )
            }
          >
            {detail.status === "open" ? "Close" : "Reopen"}
          </Button>
        </form>
      )}
    </div>
  );
}
