"use client";

// "Maintenance" — the resident's work-order surface: submit a request
// (category, priority, description, photos), then follow its status and
// timeline as staff triage and resolve it.

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { api, ApiError, type MyTicketDetail } from "@/lib/api";
import type { MaintenanceTicket } from "@/lib/types";
import { toast } from "sonner";
import { SiteHeader } from "@/components/SiteHeader";
import { Badge, Button, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";

const CATEGORIES = [
  "general",
  "plumbing",
  "electrical",
  "hvac",
  "appliance",
  "structural",
];
const PRIORITIES = ["low", "normal", "high", "urgent"];

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

export default function MyMaintenancePage() {
  const { user, loading } = useAuth();
  const [tickets, setTickets] = useState<MaintenanceTicket[] | null>(null);
  const [noLease, setNoLease] = useState(false);

  const load = useCallback(async () => {
    try {
      setTickets(await api.myTickets());
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
          Maintenance
        </h1>
        <p className="mb-6 text-ink-3">
          Report an issue in your home and follow it to resolution.
        </p>

        {!loading && !user && (
          <Card className="p-8 text-center">
            <p className="mb-3 text-ink-2">
              Sign in to submit a maintenance request.
            </p>
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
            No lease is linked to your account yet, so there&apos;s nowhere to
            file a request.
          </Card>
        )}

        {user && tickets && (
          <div className="space-y-5">
            <NewRequestCard onCreated={load} />
            <RequestList tickets={tickets} onChange={load} />
          </div>
        )}
      </main>
    </>
  );
}

function NewRequestCard({ onCreated }: { onCreated: () => void }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [title, setTitle] = useState("");
  const [category, setCategory] = useState("general");
  const [priority, setPriority] = useState("normal");
  const [description, setDescription] = useState("");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim()) {
      toast.error("Give the request a short title.");
      return;
    }
    setBusy(true);
    try {
      await api.createMyTicket({
        title: title.trim(),
        description: description.trim() || undefined,
        category,
        priority,
      });
      toast.success("Request submitted — we're on it.");
      setTitle("");
      setDescription("");
      setCategory("general");
      setPriority("normal");
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
              Something need fixing?
            </div>
            <div className="text-sm text-ink-3">
              Submit a request and the management team is notified immediately.
            </div>
          </div>
          <Button onClick={() => setOpen(true)}>New request</Button>
        </div>
      ) : (
        <form onSubmit={submit} className="space-y-3">
          <div className="font-display text-lg font-bold">New request</div>
          <input
            className={field}
            placeholder="Short title, e.g. “Kitchen sink is dripping”"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            maxLength={120}
          />
          <div className="grid grid-cols-2 gap-3">
            <label className="text-sm">
              <span className="mb-1 block text-xs uppercase tracking-wide text-ink-3">
                Category
              </span>
              <select
                className={field}
                value={category}
                onChange={(e) => setCategory(e.target.value)}
              >
                {CATEGORIES.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </label>
            <label className="text-sm">
              <span className="mb-1 block text-xs uppercase tracking-wide text-ink-3">
                Priority
              </span>
              <select
                className={field}
                value={priority}
                onChange={(e) => setPriority(e.target.value)}
              >
                {PRIORITIES.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <textarea
            className={field}
            rows={3}
            placeholder="What's happening? Anything that helps us fix it faster."
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
          <div className="flex gap-2">
            <Button type="submit" disabled={busy}>
              {busy ? "Submitting…" : "Submit request"}
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

function RequestList({
  tickets,
  onChange,
}: {
  tickets: MaintenanceTicket[];
  onChange: () => void;
}) {
  const [openId, setOpenId] = useState<string | null>(null);
  return (
    <Card>
      <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
        My requests
      </div>
      {tickets.length === 0 ? (
        <div className="px-5 py-6 text-sm text-ink-3">
          No requests yet — everything working as it should.
        </div>
      ) : (
        <ul className="divide-y divide-line">
          {tickets.map((t) => (
            <li key={t.id} className="px-5 py-3">
              <button
                className="flex w-full flex-wrap items-center justify-between gap-3 text-left"
                onClick={() => setOpenId(openId === t.id ? null : t.id)}
              >
                <div className="min-w-0">
                  <div className="truncate font-semibold">{t.title}</div>
                  <div className="text-xs text-ink-3">
                    {t.category} · {t.priority} · {t.created_at.slice(0, 10)}
                  </div>
                </div>
                <Badge tone={statusTone(t.status)}>
                  {t.status.replace("_", " ")}
                </Badge>
              </button>
              {openId === t.id && (
                <RequestDetail ticketId={t.id} onChange={onChange} />
              )}
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function RequestDetail({
  ticketId,
  onChange,
}: {
  ticketId: string;
  onChange: () => void;
}) {
  const [detail, setDetail] = useState<MyTicketDetail | null>(null);
  const [comment, setComment] = useState("");
  const [busy, setBusy] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  const load = useCallback(async () => {
    try {
      setDetail(await api.myTicket(ticketId));
    } catch {
      // The row list already shows the ticket; the drawer just stays empty.
    }
  }, [ticketId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function sendComment(e: React.FormEvent) {
    e.preventDefault();
    if (!comment.trim()) return;
    setBusy(true);
    try {
      await api.addMyTicketComment(ticketId, comment.trim());
      setComment("");
      await load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  async function upload(file: File) {
    setBusy(true);
    try {
      await api.uploadMyTicketPhoto(ticketId, file);
      toast.success("Photo attached.");
      await load();
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setBusy(false);
    }
  }

  async function openDoc(id: string) {
    try {
      const { url } = await api.myDocumentDownloadUrl(id);
      window.open(url, "_blank", "noopener");
    } catch {
      toast.error("Download failed — try again in a moment.");
    }
  }

  if (!detail) return <div className="py-3 text-sm text-ink-3">Loading…</div>;

  return (
    <div className="mt-3 space-y-3 rounded-xl border border-line p-4 text-sm">
      {detail.description && <p className="text-ink-2">{detail.description}</p>}

      {detail.documents.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {detail.documents.map((d) => (
            <button
              key={d.id}
              onClick={() => void openDoc(d.id)}
              disabled={d.status !== "stored"}
              className="rounded-xl border border-line px-3 py-1.5 text-xs font-semibold hover:bg-surface-2 disabled:opacity-50"
            >
              📎 {d.filename}
            </button>
          ))}
        </div>
      )}

      {detail.comments.length > 0 && (
        <ul className="space-y-2">
          {detail.comments.map((c) => (
            <li key={c.id} className="rounded-xl bg-surface-2 px-3 py-2">
              <span className="text-xs text-ink-3">
                {c.kind === "status" ? "status change" : "comment"} ·{" "}
                {c.created_at.slice(0, 10)}
              </span>
              <div>{c.body}</div>
            </li>
          ))}
        </ul>
      )}

      <form onSubmit={sendComment} className="flex gap-2">
        <input
          className={field}
          placeholder="Add a comment…"
          value={comment}
          onChange={(e) => setComment(e.target.value)}
        />
        <Button type="submit" disabled={busy || !comment.trim()}>
          Send
        </Button>
        <input
          ref={fileInput}
          type="file"
          accept="image/*,.pdf"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void upload(f);
            e.target.value = "";
          }}
        />
        <Button
          variant="outline"
          type="button"
          disabled={busy}
          onClick={() => fileInput.current?.click()}
        >
          Add photo
        </Button>
      </form>
    </div>
  );
}
