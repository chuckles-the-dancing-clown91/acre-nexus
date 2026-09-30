"use client";

// A CRM timeline for one subject (an owner or an owner lead): every note,
// call, email and meeting, pinned items first, with follow-up chips and a
// composer to log something new. Reads need `entity:read`; writing needs
// `entity:manage` (passed in as `canManage`).

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  Check,
  Mail,
  MessageSquare,
  Phone,
  Pin,
  PinOff,
  RefreshCw,
  StickyNote,
  Trash2,
  TriangleAlert,
  Users,
} from "lucide-react";
import {
  crm,
  isoDate,
  type CrmNote,
  type NoteKind,
  type SubjectType,
} from "@/lib/backoffice";
import { Badge, Button } from "@/components/ui";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

/** Kinds a person can log (system "update" notes are written by the server). */
export const NOTE_KINDS: Exclude<NoteKind, "update">[] = [
  "note",
  "call",
  "email",
  "meeting",
  "issue",
  "text",
];

const KIND_LABEL: Record<NoteKind, string> = {
  note: "Note",
  call: "Call",
  email: "Email",
  meeting: "Meeting",
  issue: "Issue",
  text: "Text",
  update: "Update",
};

const KIND_TONE: Record<NoteKind, "neutral" | "info" | "accent" | "bad"> = {
  note: "neutral",
  call: "info",
  email: "info",
  meeting: "accent",
  issue: "bad",
  text: "info",
  update: "neutral",
};

/** A small icon for a note kind. */
export function KindIcon({
  kind,
  size = 14,
}: {
  kind: NoteKind;
  size?: number;
}) {
  const props = { size, "aria-hidden": true } as const;
  switch (kind) {
    case "call":
      return <Phone {...props} />;
    case "email":
      return <Mail {...props} />;
    case "meeting":
      return <Users {...props} />;
    case "issue":
      return <TriangleAlert {...props} />;
    case "text":
      return <MessageSquare {...props} />;
    case "update":
      return <RefreshCw {...props} />;
    default:
      return <StickyNote {...props} />;
  }
}

/** A badge showing a note kind with its icon. */
export function KindBadge({ kind }: { kind: NoteKind }) {
  return (
    <Badge tone={KIND_TONE[kind]} className="px-2 py-0.5">
      <KindIcon kind={kind} size={12} />
      {KIND_LABEL[kind]}
    </Badge>
  );
}

/** Parse a `YYYY-MM-DD` as a local date. */
export function parseDay(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, (m ?? 1) - 1, d ?? 1);
}

/** `Oct 3` (or `Oct 3, 2025` outside this year) for a `YYYY-MM-DD`. */
export function prettyDay(day: string): string {
  const d = parseDay(day);
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleDateString([], {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

/** Tone + label for a follow-up date relative to today. */
export function followUpState(
  day: string,
  done = false
): { tone: "good" | "bad" | "warn" | "neutral"; label: string } {
  if (done) return { tone: "good", label: `Done · ${prettyDay(day)}` };
  const today = isoDate(new Date());
  if (day < today) return { tone: "bad", label: `Overdue · ${prettyDay(day)}` };
  if (day === today) return { tone: "warn", label: "Today" };
  return { tone: "neutral", label: prettyDay(day) };
}

/** A timestamp as `Oct 3, 2:15 PM`. */
function stamp(iso: string) {
  return new Date(iso).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function CrmTimeline({
  subjectType,
  subjectId,
  canManage,
  onChange,
}: {
  subjectType: SubjectType;
  subjectId: string;
  canManage: boolean;
  /** Called after any write, so the parent can refresh counts. */
  onChange?: () => void;
}) {
  const [notes, setNotes] = useState<CrmNote[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    crm
      .notes(subjectType, subjectId)
      .then(setNotes)
      .catch((e) => setError(e.message));
  }, [subjectType, subjectId]);

  useEffect(() => {
    reload();
  }, [reload]);

  const changed = useCallback(() => {
    reload();
    onChange?.();
  }, [reload, onChange]);

  async function patch(
    n: CrmNote,
    body: Parameters<typeof crm.updateNote>[1],
    ok?: string
  ) {
    try {
      await crm.updateNote(n.id, body);
      if (ok) toast.success(ok);
      changed();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save that");
    }
  }

  async function remove(n: CrmNote) {
    if (!window.confirm("Delete this note? This can't be undone.")) return;
    try {
      await crm.deleteNote(n.id);
      toast.success("Note deleted");
      changed();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't delete the note");
    }
  }

  const sorted = notes
    ? [...notes].sort(
        (a, b) =>
          Number(b.pinned) - Number(a.pinned) ||
          b.created_at.localeCompare(a.created_at)
      )
    : null;

  return (
    <div className="space-y-4">
      {canManage && (
        <Composer
          subjectType={subjectType}
          subjectId={subjectId}
          onAdded={changed}
        />
      )}

      {error && <p className="text-sm text-bad">{error}</p>}

      <ol className="space-y-3">
        {sorted?.map((n) => {
          const system = n.kind === "update";
          const fu = n.follow_up_on
            ? followUpState(n.follow_up_on, n.follow_up_done)
            : null;
          return (
            <li
              key={n.id}
              className={`rounded-xl border px-4 py-3 ${
                n.pinned
                  ? "border-accent bg-accent-soft/40"
                  : system
                    ? "border-line bg-surface-2/60"
                    : "border-line bg-surface"
              }`}
            >
              <div className="flex flex-wrap items-center gap-2 text-xs text-ink-3">
                {system ? (
                  <span className="inline-flex items-center gap-1">
                    <KindIcon kind="update" size={12} />
                    Update
                  </span>
                ) : (
                  <KindBadge kind={n.kind} />
                )}
                {n.pinned && (
                  <span className="inline-flex items-center gap-1 font-semibold text-accent-2">
                    <Pin size={12} aria-hidden /> Pinned
                  </span>
                )}
                <span>
                  {n.author ?? "System"} · {stamp(n.created_at)}
                </span>
                <span className="ml-auto flex items-center gap-1">
                  {canManage && !system && (
                    <button
                      onClick={() => patch(n, { pinned: !n.pinned })}
                      className="rounded-lg p-1 text-ink-3 hover:bg-surface-2 hover:text-ink"
                      title={n.pinned ? "Unpin" : "Pin to top"}
                      aria-label={n.pinned ? "Unpin" : "Pin to top"}
                    >
                      {n.pinned ? <PinOff size={14} /> : <Pin size={14} />}
                    </button>
                  )}
                  {canManage && (
                    <button
                      onClick={() => remove(n)}
                      className="rounded-lg p-1 text-ink-3 hover:bg-bad-soft hover:text-bad"
                      title="Delete"
                      aria-label="Delete note"
                    >
                      <Trash2 size={14} />
                    </button>
                  )}
                </span>
              </div>
              <p
                className={`mt-1.5 whitespace-pre-wrap text-sm ${
                  system ? "text-ink-3" : "text-ink"
                }`}
              >
                {n.body}
              </p>
              {fu && n.follow_up_on && (
                <div className="mt-2 flex items-center gap-2">
                  <Badge tone={fu.tone} className="px-2 py-0.5">
                    Follow up · {fu.label}
                  </Badge>
                  {canManage && (
                    <button
                      onClick={() =>
                        patch(
                          n,
                          { follow_up_done: !n.follow_up_done },
                          n.follow_up_done
                            ? "Follow-up reopened"
                            : "Follow-up done"
                        )
                      }
                      className="inline-flex items-center gap-1 rounded-lg border border-line px-2 py-0.5 text-xs font-semibold text-ink-2 hover:bg-surface-2"
                    >
                      <Check size={12} aria-hidden />
                      {n.follow_up_done ? "Reopen" : "Mark done"}
                    </button>
                  )}
                </div>
              )}
            </li>
          );
        })}
        {sorted === null && !error && (
          <li className="py-6 text-center text-sm text-ink-3">Loading…</li>
        )}
        {sorted && sorted.length === 0 && (
          <li className="py-6 text-center text-sm text-ink-3">
            Nothing logged yet.
            {canManage && " Add a note, call or meeting above."}
          </li>
        )}
      </ol>
    </div>
  );
}

function Composer({
  subjectType,
  subjectId,
  onAdded,
}: {
  subjectType: SubjectType;
  subjectId: string;
  onAdded: () => void;
}) {
  const [kind, setKind] = useState<NoteKind>("note");
  const [body, setBody] = useState("");
  const [followUp, setFollowUp] = useState("");
  const [pinned, setPinned] = useState(false);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!body.trim()) return;
    setBusy(true);
    try {
      await crm.addNote({
        subject_type: subjectType,
        subject_id: subjectId,
        kind,
        body: body.trim(),
        pinned,
        follow_up_on: followUp || null,
      });
      setBody("");
      setFollowUp("");
      setPinned(false);
      toast.success(followUp ? "Logged, with a follow-up" : "Logged");
      onAdded();
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : "Couldn't save the note"
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={submit}
      className="space-y-2 rounded-xl border border-line bg-surface-2 p-3"
    >
      <div className="flex flex-wrap gap-1">
        {NOTE_KINDS.map((k) => (
          <button
            type="button"
            key={k}
            onClick={() => setKind(k)}
            className={`inline-flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-semibold ${
              kind === k
                ? "bg-accent-soft text-accent-2"
                : "text-ink-3 hover:bg-surface"
            }`}
          >
            <KindIcon kind={k} size={12} />
            {KIND_LABEL[k]}
          </button>
        ))}
      </div>
      <textarea
        value={body}
        onChange={(e) => setBody(e.target.value)}
        rows={3}
        placeholder={
          kind === "call"
            ? "What did you talk about?"
            : kind === "issue"
              ? "What's the problem?"
              : "What happened?"
        }
        className={field}
      />
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2 text-xs font-semibold text-ink-3">
          Follow up on
          <input
            type="date"
            value={followUp}
            onChange={(e) => setFollowUp(e.target.value)}
            className="rounded-lg border border-line bg-surface px-2 py-1 text-sm font-normal text-ink"
          />
        </label>
        <label className="flex items-center gap-1.5 text-xs font-semibold text-ink-3">
          <input
            type="checkbox"
            checked={pinned}
            onChange={(e) => setPinned(e.target.checked)}
          />
          Pin to top
        </label>
        <Button
          type="submit"
          disabled={busy || !body.trim()}
          className="ml-auto px-3 py-1.5"
        >
          {busy ? "Saving…" : "Add to timeline"}
        </Button>
      </div>
    </form>
  );
}
