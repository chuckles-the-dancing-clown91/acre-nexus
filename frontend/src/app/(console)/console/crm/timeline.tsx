"use client";

// A CRM timeline for one owner or owner lead: every note, call, email and
// meeting, pinned first, with follow-up chips and a composer to log
// something new. Reading needs `entity:read`; writing `entity:manage`.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
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
import { toast } from "sonner";
import {
  crm,
  isoDate,
  type CrmNote,
  type NoteKind,
  type SubjectType,
} from "@/lib/backoffice";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/misc";
import { cn } from "@/lib/utils";

/** Kinds a person can log ("update" notes are written by the server). */
const NOTE_KINDS: Exclude<NoteKind, "update">[] = [
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

const KIND_TONE: Record<NoteKind, Tone> = {
  note: "neutral",
  call: "info",
  email: "info",
  meeting: "accent",
  issue: "bad",
  text: "info",
  update: "neutral",
};

export function KindIcon({
  kind,
  className,
}: {
  kind: NoteKind;
  className?: string;
}) {
  const props = { className: cn("size-3", className), "aria-hidden": true };
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

export function KindBadge({ kind }: { kind: NoteKind }) {
  return (
    <Badge tone={KIND_TONE[kind]}>
      <KindIcon kind={kind} />
      {KIND_LABEL[kind]}
    </Badge>
  );
}

/** Parse a `YYYY-MM-DD` as a local date. */
export function parseDay(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, (m ?? 1) - 1, d ?? 1);
}

/** `Oct 3`, or `Oct 3, 2025` outside this year. */
export function prettyDay(day: string): string {
  const d = parseDay(day);
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleDateString([], {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

/** Tone and label for a follow-up date against today. */
export function followUpState(
  day: string,
  done = false
): { tone: Tone; label: string } {
  if (done) return { tone: "good", label: `Done · ${prettyDay(day)}` };
  const today = isoDate(new Date());
  if (day < today) return { tone: "bad", label: `Overdue · ${prettyDay(day)}` };
  if (day === today) return { tone: "warn", label: "Today" };
  return { tone: "neutral", label: prettyDay(day) };
}

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
}: {
  subjectType: SubjectType;
  subjectId: string;
  canManage: boolean;
}) {
  const qc = useQueryClient();
  const notes = useQuery({
    queryKey: ["crm", "notes", subjectType, subjectId],
    queryFn: () => crm.notes(subjectType, subjectId),
  });
  // Every write can move follow-up counts, owners' last contact, and leads.
  const changed = () => qc.invalidateQueries({ queryKey: ["crm"] });

  async function patch(
    n: CrmNote,
    body: Parameters<typeof crm.updateNote>[1],
    ok?: string
  ) {
    try {
      await crm.updateNote(n.id, body);
      if (ok) toast.success(ok);
      await changed();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save that");
    }
  }

  async function remove(n: CrmNote) {
    if (!window.confirm("Delete this note? This can't be undone.")) return;
    try {
      await crm.deleteNote(n.id);
      toast.success("Note deleted");
      await changed();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't delete the note");
    }
  }

  const sorted = notes.data
    ? [...notes.data].sort(
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

      {notes.error && (
        <p className="text-[13px] text-bad">
          Couldn&apos;t load the timeline: {notes.error.message}
        </p>
      )}
      {notes.isLoading && (
        <div className="space-y-2">
          <Skeleton className="h-20" />
          <Skeleton className="h-20" />
        </div>
      )}

      <ol className="space-y-2.5">
        {sorted?.map((n) => {
          const system = n.kind === "update";
          const fu = n.follow_up_on
            ? followUpState(n.follow_up_on, n.follow_up_done)
            : null;
          return (
            <li
              key={n.id}
              className={cn(
                "rounded-xl border px-4 py-3",
                n.pinned
                  ? "border-accent/40 bg-accent/5"
                  : system
                    ? "border-line bg-fill/60"
                    : "border-line bg-surface"
              )}
            >
              <div className="flex flex-wrap items-center gap-2 text-xs text-fg-3">
                {system ? (
                  <span className="inline-flex items-center gap-1">
                    <KindIcon kind="update" />
                    Update
                  </span>
                ) : (
                  <KindBadge kind={n.kind} />
                )}
                {n.pinned && (
                  <span className="inline-flex items-center gap-1 font-medium text-accent">
                    <Pin className="size-3" aria-hidden />
                    Pinned
                  </span>
                )}
                <span>
                  {n.author ?? "System"} · {stamp(n.created_at)}
                </span>
                {canManage && (
                  <span className="ml-auto flex items-center">
                    {!system && (
                      <Button
                        size="icon"
                        variant="ghost"
                        className="size-7"
                        onClick={() => patch(n, { pinned: !n.pinned })}
                        aria-label={n.pinned ? "Unpin" : "Pin to top"}
                        title={n.pinned ? "Unpin" : "Pin to top"}
                      >
                        {n.pinned ? <PinOff /> : <Pin />}
                      </Button>
                    )}
                    <Button
                      size="icon"
                      variant="ghost"
                      className="size-7 hover:text-bad"
                      onClick={() => remove(n)}
                      aria-label="Delete note"
                      title="Delete"
                    >
                      <Trash2 />
                    </Button>
                  </span>
                )}
              </div>
              <p
                className={cn(
                  "mt-1.5 text-[14px] whitespace-pre-wrap",
                  system ? "text-fg-3" : "text-fg"
                )}
              >
                {n.body}
              </p>
              {fu && (
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <Badge tone={fu.tone}>Follow up · {fu.label}</Badge>
                  {canManage && (
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7"
                      onClick={() =>
                        patch(
                          n,
                          { follow_up_done: !n.follow_up_done },
                          n.follow_up_done
                            ? "Follow-up reopened"
                            : "Follow-up done"
                        )
                      }
                    >
                      <Check />
                      {n.follow_up_done ? "Reopen" : "Mark done"}
                    </Button>
                  )}
                </div>
              )}
            </li>
          );
        })}
        {sorted && sorted.length === 0 && (
          <li className="py-6 text-center text-[13px] text-fg-3">
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
  onAdded: () => Promise<unknown>;
}) {
  const [kind, setKind] = useState<NoteKind>("note");
  const [body, setBody] = useState("");
  const [followUp, setFollowUp] = useState("");
  const [pinned, setPinned] = useState(false);
  const [busy, setBusy] = useState(false);

  async function submit() {
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
      await onAdded();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the note");
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
      className="space-y-2.5 rounded-xl border border-line bg-fill p-3"
    >
      <div className="flex flex-wrap gap-1" role="radiogroup" aria-label="Kind">
        {NOTE_KINDS.map((k) => (
          <button
            type="button"
            role="radio"
            aria-checked={kind === k}
            key={k}
            onClick={() => setKind(k)}
            className={cn(
              "inline-flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-medium transition",
              kind === k
                ? "bg-accent/12 text-accent"
                : "text-fg-3 hover:bg-fill-2 hover:text-fg"
            )}
          >
            <KindIcon kind={k} />
            {KIND_LABEL[k]}
          </button>
        ))}
      </div>
      <textarea
        aria-label="What happened"
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
        className={cn(fieldClass, "w-full text-[14px]")}
      />
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2 text-xs text-fg-3">
          Follow up on
          <input
            type="date"
            value={followUp}
            onChange={(e) => setFollowUp(e.target.value)}
            className={cn(fieldClass, "py-1")}
          />
        </label>
        <label className="flex items-center gap-1.5 text-xs text-fg-3">
          <input
            type="checkbox"
            checked={pinned}
            onChange={(e) => setPinned(e.target.checked)}
            className="size-4 accent-[var(--accent)]"
          />
          Pin to top
        </label>
        <Button
          type="submit"
          size="sm"
          loading={busy}
          disabled={!body.trim()}
          className="ml-auto"
        >
          Add to timeline
        </Button>
      </div>
    </form>
  );
}
