"use client";

// Entity detail: contact details, inline notes, and a chronological notes log
// for a single counterparty. Users with "entity:manage" can append notes.

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api";
import type { CounterpartyDetail } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";
import { Icon } from "@/components/Icon";

/** Turn a snake/lower key into a human label, e.g. `property_manager` → `Property manager`. */
function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Format an ISO timestamp into a compact, locale-aware date-time string. */
function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function EntityDetailPage() {
  const params = useParams<{ id: string }>();
  const { can } = useAuth();
  const id = params.id;

  const [detail, setDetail] = useState<CounterpartyDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [note, setNote] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [noteError, setNoteError] = useState<string | null>(null);

  const canManage = can("entity:manage");

  const load = useCallback(() => {
    if (!id) return;
    api
      .entity(id)
      .then(setDetail)
      .catch((e: Error) => setError(e.message));
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  const addNote = useCallback(async () => {
    if (!id || !note.trim()) return;
    setSubmitting(true);
    setNoteError(null);
    try {
      await api.addEntityNote(id, note.trim());
      setNote("");
      load();
    } catch (err) {
      setNoteError(err instanceof Error ? err.message : "Couldn't add note.");
    } finally {
      setSubmitting(false);
    }
  }, [id, note, load]);

  if (error)
    return <p className="text-bad">Couldn&apos;t load entity: {error}</p>;
  if (!detail) return <p className="text-ink-3">Loading…</p>;

  const notesLog = [...detail.notes_log].sort((a, b) =>
    b.created_at.localeCompare(a.created_at)
  );

  return (
    <div className="space-y-6">
      <Link
        href="/console/entities"
        className="inline-flex items-center gap-2 text-sm font-semibold text-ink-2"
      >
        <Icon name="back" size={16} /> All entities
      </Link>

      <div className="flex flex-wrap items-center gap-3">
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          {detail.name}
        </h1>
        <Badge tone="info">{humanize(detail.kind)}</Badge>
      </div>

      <Card className="p-5">
        <h2 className="mb-4 font-display text-lg font-bold">Details</h2>
        <dl className="space-y-3 text-sm">
          <Row k="Contact" v={detail.contact_name ?? "—"} />
          <Row k="Email" v={detail.email ?? "—"} />
          <Row k="Phone" v={detail.phone ?? "—"} />
          <Row k="Website" v={detail.website ?? "—"} />
          <Row k="Address" v={detail.address ?? "—"} />
          <Row k="Notes" v={detail.notes ?? "—"} />
        </dl>
      </Card>

      <Card className="p-5">
        <h2 className="mb-4 font-display text-lg font-bold">Notes log</h2>

        {canManage && (
          <div className="mb-5 space-y-3">
            <textarea
              value={note}
              onChange={(e) => setNote(e.target.value)}
              rows={3}
              placeholder="Add a note about this entity…"
              className="w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink"
            />
            {noteError && <p className="text-bad">{noteError}</p>}
            <Button onClick={addNote} disabled={submitting || !note.trim()}>
              {submitting ? "Saving…" : "Add note"}
            </Button>
          </div>
        )}

        {notesLog.length === 0 ? (
          <p className="text-sm text-ink-3">No notes yet.</p>
        ) : (
          <div className="space-y-3">
            {notesLog.map((n) => (
              <div
                key={n.id}
                className="border-b border-line pb-3 last:border-0"
              >
                <p className="whitespace-pre-wrap text-sm text-ink">{n.body}</p>
                <p className="mt-1 font-mono text-xs text-ink-3">
                  {formatTimestamp(n.created_at)}
                </p>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}

function Row({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex items-center justify-between border-b border-line pb-2.5 last:border-0">
      <dt className="text-ink-3">{k}</dt>
      <dd className="font-semibold">{v}</dd>
    </div>
  );
}
