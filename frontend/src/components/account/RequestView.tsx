"use client";

// One repair request as the resident sees it: where it stands, every update
// from the team, the photos and videos on it, a reply box, and a rating
// once it's done.

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Play, Star } from "lucide-react";
import { toast } from "sonner";
import { api, type MyTicketFile } from "@/lib/api";
import { residentStatus } from "@/lib/resident";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { MediaPicker } from "./MediaPicker";

function when(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function RequestView({
  id,
  onBack,
}: {
  id: string;
  onBack: () => void;
}) {
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["my-ticket", id],
    queryFn: () => api.myTicket(id),
  });
  const [body, setBody] = useState("");
  const [files, setFiles] = useState<File[]>([]);
  const [busy, setBusy] = useState(false);
  const [stars, setStars] = useState(0);
  const [review, setReview] = useState("");

  const byId = useMemo(
    () => new Map((q.data?.files ?? []).map((f) => [f.id, f])),
    [q.data]
  );

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["my-ticket", id] });
    void qc.invalidateQueries({ queryKey: ["my-tickets"] });
  };

  async function send() {
    if (!body.trim() && files.length === 0) return;
    setBusy(true);
    try {
      const ids: string[] = [];
      for (const f of files)
        ids.push((await api.uploadMyTicketPhoto(id, f)).id);
      await api.addMyTicketComment(id, body.trim(), ids);
      setBody("");
      setFiles([]);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send it");
    } finally {
      setBusy(false);
    }
  }

  async function rate() {
    if (!stars) return;
    setBusy(true);
    try {
      await api.reviewMyTicket(id, stars, review.trim() || undefined);
      toast.success("Thanks for telling us.");
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save it");
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
      All requests
    </button>
  );

  if (q.error)
    return (
      <div>
        {back}
        <Panel className="p-6 text-[14px] text-fg-2">
          We couldn&apos;t find that request. It may belong to another home.
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

  const done = t.status === "resolved" || t.status === "closed";
  const timeline = [...t.comments].sort((a, b) =>
    a.created_at.localeCompare(b.created_at)
  );

  return (
    <div className="space-y-4">
      {back}
      <Panel className="p-5">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <h1 className="text-[20px] leading-tight font-semibold text-fg">
              {t.title}
            </h1>
            <p className="mt-1 text-[13px] text-fg-3">
              Sent {new Date(t.created_at).toLocaleDateString()}
              {t.location && ` · ${t.location}`}
            </p>
          </div>
          <Badge tone={statusTone(t.status)}>
            {residentStatus(t.status, t.waiting_on)}
          </Badge>
        </div>
        {t.description && (
          <p className="mt-3 text-[14px] whitespace-pre-wrap text-fg-2">
            {t.description}
          </p>
        )}
        {t.status === "on_hold" && t.waiting_on === "resident" && (
          <p className="mt-3 rounded-xl border border-warn/30 bg-warn/10 px-3 py-2 text-[13px] text-fg">
            We need to get in to finish this. Reply below with a good time or
            how to get in.
          </p>
        )}
      </Panel>

      {t.files.length > 0 && (
        <Panel className="p-4">
          <div className="eyebrow mb-2">Photos and video</div>
          <div className="grid grid-cols-3 gap-2 sm:grid-cols-4">
            {t.files.map((f) => (
              <Thumb key={f.id} file={f} />
            ))}
          </div>
        </Panel>
      )}

      <Panel className="p-4">
        <div className="eyebrow mb-3">Updates</div>
        {timeline.length === 0 && (
          <p className="text-[13px] text-fg-3">
            We have your request. Updates show up here as we work on it.
          </p>
        )}
        <ol className="space-y-3">
          {timeline.map((c) => {
            const mine = c.action === "resident_comment";
            const media = (c.document_ids ?? [])
              .map((d) => byId.get(d))
              .filter((f): f is MyTicketFile => !!f);
            return (
              <li
                key={c.id}
                className={cn("flex", mine ? "justify-end" : "justify-start")}
              >
                <div
                  className={cn(
                    "max-w-[85%] rounded-2xl px-3.5 py-2.5",
                    mine
                      ? "bg-accent text-accent-fg"
                      : c.kind === "status" || c.kind === "action"
                        ? "bg-fill text-fg"
                        : "border border-line bg-surface text-fg"
                  )}
                >
                  {c.body && (
                    <p className="text-[14px] whitespace-pre-wrap">{c.body}</p>
                  )}
                  {media.length > 0 && (
                    <div className="mt-2 flex flex-wrap gap-1.5">
                      {media.map((f) => (
                        <Thumb key={f.id} file={f} className="size-20" />
                      ))}
                    </div>
                  )}
                  <div
                    className={cn(
                      "mt-1 text-[11px]",
                      mine ? "text-accent-fg/75" : "text-fg-3"
                    )}
                  >
                    {mine ? "You" : (c.author_name ?? "Maintenance")} ·{" "}
                    {when(c.created_at)}
                  </div>
                </div>
              </li>
            );
          })}
        </ol>
      </Panel>

      {t.status !== "cancelled" && (
        <Panel className="space-y-3 p-4">
          <textarea
            className="min-h-[72px] w-full resize-y rounded-xl border border-line bg-surface px-3 py-2.5 text-[14px] text-fg outline-none focus:border-accent"
            placeholder={
              done
                ? "Still a problem? Tell us what's happening."
                : "Add a note or a better time to come by"
            }
            value={body}
            onChange={(e) => setBody(e.target.value)}
          />
          <div className="flex flex-wrap items-end justify-between gap-2">
            <MediaPicker files={files} onChange={setFiles} disabled={busy} />
            <Button
              onClick={send}
              disabled={busy || (!body.trim() && files.length === 0)}
            >
              {busy ? "Sending…" : "Send"}
            </Button>
          </div>
        </Panel>
      )}

      {done && t.rating == null && (
        <Panel className="space-y-3 p-4">
          <div className="text-[15px] font-semibold text-fg">
            How did we do?
          </div>
          <div className="flex gap-1" role="radiogroup" aria-label="Rating">
            {[1, 2, 3, 4, 5].map((n) => (
              <button
                key={n}
                type="button"
                role="radio"
                aria-checked={stars === n}
                aria-label={`${n} star${n === 1 ? "" : "s"}`}
                onClick={() => setStars(n)}
              >
                <Star
                  className={cn(
                    "size-7",
                    n <= stars ? "fill-warn text-warn" : "text-fg-4"
                  )}
                />
              </button>
            ))}
          </div>
          {stars > 0 && (
            <>
              <textarea
                className="min-h-[64px] w-full rounded-xl border border-line bg-surface px-3 py-2.5 text-[14px] text-fg outline-none focus:border-accent"
                placeholder="Anything to add? (optional)"
                value={review}
                onChange={(e) => setReview(e.target.value)}
              />
              <Button onClick={rate} disabled={busy}>
                Send rating
              </Button>
            </>
          )}
        </Panel>
      )}
      {t.rating != null && (
        <p className="text-center text-[13px] text-fg-3">
          You rated this {t.rating} of 5. Thank you.
        </p>
      )}
    </div>
  );
}

function Thumb({
  file,
  className,
}: {
  file: MyTicketFile;
  className?: string;
}) {
  if (!file.url) return null;
  const video = file.kind === "video" || file.mime_type.startsWith("video/");
  if (file.kind === "document" && !video)
    return (
      <a
        href={file.url}
        target="_blank"
        rel="noreferrer"
        className="truncate text-xs text-accent underline"
      >
        {file.filename}
      </a>
    );
  return (
    <a
      href={file.url}
      target="_blank"
      rel="noreferrer"
      title={file.filename}
      className={cn(
        "relative block aspect-square overflow-hidden rounded-lg border border-line bg-fill",
        className
      )}
    >
      {video ? (
        <>
          <video
            src={file.url}
            preload="metadata"
            muted
            playsInline
            className="size-full object-cover"
          />
          <span className="absolute inset-0 flex items-center justify-center">
            <span className="flex size-8 items-center justify-center rounded-full bg-black/55 text-white">
              <Play className="size-4" />
            </span>
          </span>
        </>
      ) : (
        // eslint-disable-next-line @next/next/no-img-element -- signed blob URL
        <img
          src={file.url}
          alt={file.filename}
          className="size-full object-cover"
        />
      )}
    </a>
  );
}
