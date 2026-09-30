"use client";

// Two-way texts: one conversation per phone number — residents texting the
// office number, console replies, and every automatic text (receipts,
// reminders) filed in the same thread. STOP always wins: a number that texted
// STOP can't be texted until it replies START. Gated by `message:read`;
// replying, starting and closing need `message:manage`.

import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  api,
  type TextThread,
  type TextThreadDetail,
  type TextsStatus,
} from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

/** How often the inbox refreshes while open. */
const POLL_MS = 15_000;

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

export default function TextsPage() {
  const { can } = useAuth();
  const read = can("message:read");
  const manage = can("message:manage");
  const [status, setStatus] = useState<"open" | "done" | "">("open");
  const [threads, setThreads] = useState<TextThread[]>([]);
  const [info, setInfo] = useState<TextsStatus | null>(null);
  const [openId, setOpenId] = useState<string | null>(null);
  const [composing, setComposing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    api
      .textThreads(status || undefined)
      .then(setThreads)
      .catch((e) => setError(e.message));
  }, [status]);

  useEffect(() => {
    if (!read) return;
    reload();
    api
      .textsStatus()
      .then(setInfo)
      .catch(() => undefined);
    const t = setInterval(reload, POLL_MS);
    return () => clearInterval(t);
  }, [reload, read]);

  if (!read) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to texts. Ask an admin for the{" "}
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
            Texts
          </h1>
          <p className="text-ink-3">
            Every text with residents, in one inbox — replies, reminders and
            receipts together.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {(["open", "done", ""] as const).map((s) => (
            <button
              key={s || "all"}
              onClick={() => setStatus(s)}
              className={`rounded-lg px-3 py-1.5 text-sm font-semibold ${
                status === s
                  ? "bg-accent-soft text-accent-2"
                  : "text-ink-3 hover:bg-surface-2"
              }`}
            >
              {s === "open" ? "Open" : s === "done" ? "Done" : "All"}
            </button>
          ))}
          {manage && (
            <Button onClick={() => setComposing(true)}>New text</Button>
          )}
        </div>
      </div>

      {info && !info.live && (
        <Card className="border-warn-soft bg-warn-soft/40 p-4 text-sm text-ink-2">
          <strong className="text-warn">Test mode.</strong> Texts are logged
          here but not really sent. Use <em>Pretend they texted back</em> in a
          conversation to try replies and STOP. To go live, add a Twilio
          provider in Integrations, set{" "}
          <span className="font-mono">LIVE_PROVIDERS=sms</span>, and point the
          number&apos;s &ldquo;A message comes in&rdquo; webhook at{" "}
          <span className="break-all font-mono">
            {info.inbound_webhook_url}
          </span>
          .
        </Card>
      )}

      {error && <p className="text-bad">{error}</p>}

      <div className="grid gap-5 lg:grid-cols-[340px_1fr]">
        <Card className="overflow-hidden">
          <div className="divide-y divide-line">
            {threads.map((t) => (
              <button
                key={t.id}
                onClick={() => {
                  setOpenId(t.id);
                  setComposing(false);
                }}
                className={`block w-full px-4 py-3 text-left hover:bg-surface-2 ${
                  openId === t.id ? "bg-surface-2" : ""
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span
                    className={`truncate ${t.unread_count > 0 ? "font-bold" : "font-semibold"}`}
                  >
                    {who(t)}
                  </span>
                  <span className="shrink-0 text-xs text-ink-3">
                    {when(t.last_message_at)}
                  </span>
                </div>
                <div className="mt-0.5 flex items-center gap-2">
                  <span className="min-w-0 flex-1 truncate text-sm text-ink-3">
                    {t.last_preview ?? "—"}
                  </span>
                  {t.opted_out && <Badge tone="bad">STOP</Badge>}
                  {t.unread_count > 0 && (
                    <span className="rounded-full bg-accent px-2 py-0.5 text-xs font-bold text-on-accent">
                      {t.unread_count}
                    </span>
                  )}
                </div>
              </button>
            ))}
            {threads.length === 0 && (
              <p className="px-4 py-10 text-center text-sm text-ink-3">
                {status === "open"
                  ? "Nothing waiting on you."
                  : "No conversations yet."}
              </p>
            )}
          </div>
        </Card>

        {composing ? (
          <NewText
            onSent={(d) => {
              setComposing(false);
              setOpenId(d.thread.id);
              reload();
            }}
            onCancel={() => setComposing(false)}
          />
        ) : openId ? (
          <Conversation
            key={openId}
            id={openId}
            manage={manage}
            testMode={!!info && !info.live}
            onChanged={reload}
          />
        ) : (
          <Card className="flex items-center justify-center p-10 text-ink-3">
            Pick a conversation.
          </Card>
        )}
      </div>
    </div>
  );
}

function Conversation({
  id,
  manage,
  testMode,
  onChanged,
}: {
  id: string;
  manage: boolean;
  testMode: boolean;
  onChanged: () => void;
}) {
  const [detail, setDetail] = useState<TextThreadDetail | null>(null);
  const [reply, setReply] = useState("");
  const [pretend, setPretend] = useState("");
  const [busy, setBusy] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);

  const load = useCallback(() => {
    api
      .textThread(id)
      .then((d) => {
        setDetail(d);
        onChanged();
      })
      .catch((e) => toast.error(e.message));
  }, [id, onChanged]);

  useEffect(() => {
    load();
    const t = setInterval(load, POLL_MS);
    return () => clearInterval(t);
  }, [load]);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [detail?.messages.length]);

  async function run(fn: () => Promise<TextThreadDetail | unknown>) {
    setBusy(true);
    try {
      const res = await fn();
      if (res && typeof res === "object" && "messages" in res) {
        setDetail(res as TextThreadDetail);
      } else {
        load();
      }
      onChanged();
      return true;
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Something went wrong");
      return false;
    } finally {
      setBusy(false);
    }
  }

  if (!detail) {
    return <Card className="p-10 text-center text-ink-3">Loading…</Card>;
  }
  const t = detail.thread;

  return (
    <Card className="flex min-h-[520px] flex-col">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-3">
        <div>
          <div className="font-display text-lg font-bold">{who(t)}</div>
          {(t.display_name || t.lease_id) && (
            <div className="text-xs text-ink-3">
              {t.display_name && prettyPhone(t.phone)}
              {t.display_name && t.lease_id && " · "}
              {t.lease_id && "resident"}
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          {t.opted_out && <Badge tone="bad">Texted STOP</Badge>}
          {manage && (
            <button
              disabled={busy}
              onClick={() =>
                run(() =>
                  api.updateTextThread(
                    t.id,
                    t.status === "open" ? "done" : "open"
                  )
                )
              }
              className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold text-ink-2 hover:border-accent"
            >
              {t.status === "open" ? "Mark done" : "Reopen"}
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 space-y-2 overflow-y-auto px-5 py-4">
        {detail.messages.map((m) => (
          <div
            key={m.id}
            className={`flex ${m.direction === "out" ? "justify-end" : "justify-start"}`}
          >
            <div className="max-w-[78%]">
              <div
                className={`whitespace-pre-wrap rounded-2xl px-3.5 py-2 text-sm ${
                  m.direction === "out"
                    ? "bg-accent text-on-accent"
                    : "bg-surface-2 text-ink"
                } ${m.status === "failed" || m.status === "blocked" ? "opacity-60" : ""}`}
              >
                {m.body}
              </div>
              <div
                className={`mt-0.5 text-[11px] text-ink-3 ${m.direction === "out" ? "text-right" : ""}`}
              >
                {when(m.created_at)}
                {m.direction === "out" &&
                  ` · ${m.sent_by ?? (m.template_key ? "automatic" : "office")}`}
                {m.status === "queued" && " · sending…"}
                {m.status === "failed" && (
                  <span className="text-bad">
                    {" "}
                    · failed{m.error ? `: ${m.error}` : ""}
                  </span>
                )}
                {m.status === "blocked" && (
                  <span className="text-bad"> · not sent (STOP)</span>
                )}
                {m.media_count > 0 && ` · ${m.media_count} photo(s)`}
              </div>
            </div>
          </div>
        ))}
        <div ref={bottom} />
      </div>

      {manage && (
        <div className="space-y-2 border-t border-line p-4">
          {t.opted_out ? (
            <p className="text-sm text-ink-3">
              This number texted STOP. You can&apos;t text it until they reply
              START.
            </p>
          ) : (
            <form
              className="flex gap-2"
              onSubmit={async (e) => {
                e.preventDefault();
                if (!reply.trim()) return;
                if (await run(() => api.replyText(t.id, reply.trim())))
                  setReply("");
              }}
            >
              <textarea
                className={`${field} min-h-[44px] flex-1 resize-y`}
                rows={2}
                placeholder="Write a text…"
                value={reply}
                maxLength={1600}
                onChange={(e) => setReply(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    e.currentTarget.form?.requestSubmit();
                  }
                }}
              />
              <Button type="submit" disabled={busy || !reply.trim()}>
                Send
              </Button>
            </form>
          )}
          {testMode && (
            <form
              className="flex gap-2"
              onSubmit={async (e) => {
                e.preventDefault();
                if (!pretend.trim()) return;
                if (
                  await run(() =>
                    api
                      .simulateText(t.phone, pretend.trim())
                      // re-read so the open conversation counts as read
                      .then(() => api.textThread(t.id))
                  )
                )
                  setPretend("");
              }}
            >
              <input
                className={`${field} flex-1 border-dashed`}
                placeholder="Pretend they texted back (try STOP or START)"
                value={pretend}
                onChange={(e) => setPretend(e.target.value)}
              />
              <button
                type="submit"
                disabled={busy || !pretend.trim()}
                className="rounded-xl border border-dashed border-line-2 px-3 text-sm font-semibold text-ink-2 disabled:opacity-50"
              >
                Receive
              </button>
            </form>
          )}
        </div>
      )}
    </Card>
  );
}

function NewText({
  onSent,
  onCancel,
}: {
  onSent: (d: TextThreadDetail) => void;
  onCancel: () => void;
}) {
  const [phone, setPhone] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      onSent(await api.startText(phone, body.trim()));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send the text");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="p-5">
      <h2 className="mb-4 font-display text-lg font-bold">New text</h2>
      <form onSubmit={submit} className="space-y-3">
        <input
          className={field}
          type="tel"
          placeholder="Phone number, e.g. (760) 555-1234"
          value={phone}
          onChange={(e) => setPhone(e.target.value)}
          autoFocus
        />
        <textarea
          className={`${field} min-h-[120px]`}
          placeholder="Your message"
          maxLength={1600}
          value={body}
          onChange={(e) => setBody(e.target.value)}
        />
        <div className="flex gap-2">
          <Button
            type="submit"
            disabled={busy || !phone.trim() || !body.trim()}
          >
            {busy ? "Sending…" : "Send"}
          </Button>
          <button
            type="button"
            onClick={onCancel}
            className="rounded-xl px-4 text-sm font-semibold text-ink-3 hover:text-ink"
          >
            Cancel
          </button>
        </div>
      </form>
    </Card>
  );
}
