"use client";

// "Messages" — the resident's line to the management office: start a
// conversation, read replies, and keep the thread going. Staff answer from
// the console and the resident is notified by email + in-app.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  api,
  ApiError,
  type MessageThread,
  type MessageThreadDetail,
} from "@/lib/api";
import { toast } from "sonner";
import { SiteHeader } from "@/components/SiteHeader";
import { Badge, Button, Card } from "@/components/ui";
import { useAuth } from "@/lib/auth";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

function fmtWhen(iso: string) {
  return iso.slice(0, 16).replace("T", " ");
}

export default function MyMessagesPage() {
  const { user, loading } = useAuth();
  const [threads, setThreads] = useState<MessageThread[] | null>(null);
  const [noLease, setNoLease] = useState(false);
  const [openId, setOpenId] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setThreads(await api.myThreads());
      setNoLease(false);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) setNoLease(true);
    }
  }, []);

  useEffect(() => {
    if (user) void load();
  }, [user, load]);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[860px] px-6 py-8">
        <h1 className="mb-1 font-display text-3xl font-extrabold tracking-tight">
          Messages
        </h1>
        <p className="mb-6 text-ink-3">
          Reach the management office — we reply here and by email.
        </p>

        {!loading && !user && (
          <Card className="p-8 text-center">
            <p className="mb-3 text-ink-2">Sign in to message your manager.</p>
            <Link
              href="/login"
              className="inline-block rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent"
            >
              Sign in
            </Link>
          </Card>
        )}

        {user && noLease && (
          <Card className="p-8 text-center text-ink-3">
            No lease is linked to your account yet, so there&apos;s no office to
            message.
          </Card>
        )}

        {user && threads && (
          <div className="space-y-5">
            <NewThreadCard onCreated={load} />
            <Card>
              <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
                Conversations
              </div>
              {threads.length === 0 ? (
                <div className="px-5 py-6 text-sm text-ink-3">
                  No conversations yet.
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
                          <div className="truncate font-semibold">
                            {t.subject}
                          </div>
                          <div className="truncate text-xs text-ink-3">
                            {t.last_preview ?? ""} ·{" "}
                            {fmtWhen(t.last_message_at)}
                          </div>
                        </div>
                        <Badge tone={t.status === "open" ? "info" : "neutral"}>
                          {t.status}
                        </Badge>
                      </button>
                      {openId === t.id && (
                        <ThreadView threadId={t.id} onChange={load} />
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </Card>
          </div>
        )}
      </main>
    </>
  );
}

function NewThreadCard({ onCreated }: { onCreated: () => void }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!subject.trim() || !body.trim()) {
      toast.error("Add a subject and a message.");
      return;
    }
    setBusy(true);
    try {
      await api.createMyThread({ subject: subject.trim(), body: body.trim() });
      toast.success("Message sent — the office has been notified.");
      setSubject("");
      setBody("");
      setOpen(false);
      onCreated();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="p-5">
      {!open ? (
        <div className="flex items-center justify-between">
          <div>
            <div className="font-display text-lg font-bold">
              Need something?
            </div>
            <div className="text-sm text-ink-3">
              Start a conversation with the management office.
            </div>
          </div>
          <Button onClick={() => setOpen(true)}>New message</Button>
        </div>
      ) : (
        <form onSubmit={submit} className="space-y-3">
          <div className="font-display text-lg font-bold">New message</div>
          <input
            className={field}
            placeholder="Subject"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            maxLength={200}
          />
          <textarea
            className={field}
            rows={4}
            placeholder="Your message…"
            value={body}
            onChange={(e) => setBody(e.target.value)}
          />
          <div className="flex gap-2">
            <Button type="submit" disabled={busy}>
              {busy ? "Sending…" : "Send"}
            </Button>
            <Button
              variant="ghost"
              type="button"
              onClick={() => setOpen(false)}
            >
              Cancel
            </Button>
          </div>
        </form>
      )}
    </Card>
  );
}

function ThreadView({
  threadId,
  onChange,
}: {
  threadId: string;
  onChange: () => void;
}) {
  const [detail, setDetail] = useState<MessageThreadDetail | null>(null);
  const [reply, setReply] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setDetail(await api.myThread(threadId));
    } catch {
      // Leave the drawer empty; the list row still shows the thread.
    }
  }, [threadId]);

  useEffect(() => {
    void load();
    // A light poll while the thread is open so staff replies appear.
    const t = setInterval(() => void load(), 15000);
    return () => clearInterval(t);
  }, [load]);

  async function send(e: React.FormEvent) {
    e.preventDefault();
    if (!reply.trim()) return;
    setBusy(true);
    try {
      await api.replyMyThread(threadId, reply.trim());
      setReply("");
      await load();
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  if (!detail) return <div className="py-3 text-sm text-ink-3">Loading…</div>;

  return (
    <div className="mt-3 space-y-3 rounded-xl border border-line p-4 text-sm">
      <ul className="space-y-2">
        {detail.messages.map((m) => (
          <li
            key={m.id}
            className={
              m.sender_kind === "resident"
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
      <form onSubmit={send} className="flex gap-2">
        <input
          className={field}
          placeholder="Write a reply…"
          value={reply}
          onChange={(e) => setReply(e.target.value)}
        />
        <Button type="submit" disabled={busy || !reply.trim()}>
          Send
        </Button>
      </form>
    </div>
  );
}
