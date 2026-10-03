"use client";

// Notes on a work order, newest first, with the photos and videos taken
// along the way. Internal notes stay with staff; public ones reach the
// resident. Action buttons and the resident's own replies are marked.

import { useMemo, useRef, useState } from "react";
import { Camera, Home, Lock, MessageSquare, Play, X, Zap } from "lucide-react";
import { toast } from "sonner";
import { desk, type TicketFile } from "@/lib/servicedesk";
import type { TicketComment } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

function when(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function Notes({
  ticketId,
  comments,
  files,
  manage,
  onChange,
}: {
  ticketId: string;
  comments: TicketComment[];
  files: TicketFile[];
  manage: boolean;
  onChange: () => void;
}) {
  const [body, setBody] = useState("");
  const [internal, setInternal] = useState(true);
  const [attached, setAttached] = useState<TicketFile[]>([]);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const byId = useMemo(() => new Map(files.map((f) => [f.id, f])), [files]);

  async function attach(list: FileList) {
    setBusy(true);
    try {
      const added: TicketFile[] = [];
      for (const f of Array.from(list)) {
        const video = f.type.startsWith("video/");
        if (!video && !f.type.startsWith("image/")) {
          toast.error(`${f.name} isn't a photo or video`);
          continue;
        }
        if (f.size > (video ? 100 : 25) * 1024 * 1024) {
          toast.error(`${f.name} is over ${video ? 100 : 25} MB`);
          continue;
        }
        added.push(await desk.upload(ticketId, f, video ? "video" : "photo"));
      }
      setAttached((a) => [...a, ...added]);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setBusy(false);
      if (input.current) input.current.value = "";
    }
  }

  async function post() {
    if (!body.trim() && attached.length === 0) return;
    setBusy(true);
    try {
      await desk.note(ticketId, {
        body: body.trim() || "Photos",
        visibility: internal ? "internal" : "public",
        document_ids: attached.map((a) => a.id),
      });
      setBody("");
      setAttached([]);
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't post the note");
    } finally {
      setBusy(false);
    }
  }

  const timeline = [...comments].sort((a, b) =>
    b.created_at.localeCompare(a.created_at)
  );

  return (
    <Panel>
      <PanelHeader title="Notes" description="What happened, with photos." />
      <div className="space-y-4 p-5">
        {manage && (
          <div className="rounded-xl border border-line bg-fill/40 p-3">
            <textarea
              className="min-h-[72px] w-full resize-y bg-transparent text-[13px] text-fg outline-none placeholder:text-fg-4"
              placeholder="Add a note: what you found, what you did, what's next"
              value={body}
              onChange={(e) => setBody(e.target.value)}
            />
            {attached.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-2">
                {attached.map((a) => (
                  <span
                    key={a.id}
                    className="flex items-center gap-1 rounded-lg border border-line bg-surface px-2 py-1 text-xs text-fg-2"
                  >
                    <Camera className="size-3.5" />
                    {a.filename}
                    <button
                      type="button"
                      aria-label={`Remove ${a.filename}`}
                      onClick={() =>
                        setAttached((x) => x.filter((y) => y.id !== a.id))
                      }
                    >
                      <X className="size-3" />
                    </button>
                  </span>
                ))}
              </div>
            )}
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <input
                ref={input}
                type="file"
                accept="image/*,video/*"
                multiple
                className="hidden"
                onChange={(e) => e.target.files && attach(e.target.files)}
              />
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => input.current?.click()}
                disabled={busy}
              >
                <Camera />
                Photo or video
              </Button>
              <label className="flex items-center gap-1.5 text-xs text-fg-2">
                <input
                  type="checkbox"
                  checked={internal}
                  onChange={(e) => setInternal(e.target.checked)}
                />
                Staff only
              </label>
              <Button
                size="sm"
                className="ml-auto"
                onClick={post}
                disabled={busy || (!body.trim() && attached.length === 0)}
              >
                Post
              </Button>
            </div>
          </div>
        )}

        {timeline.length === 0 && (
          <EmptyState
            icon={<MessageSquare />}
            title="No notes yet"
            className="py-6"
          />
        )}
        <ol className="space-y-3">
          {timeline.map((c) => {
            const photos = (c.document_ids ?? [])
              .map((id) => byId.get(id))
              .filter((f): f is TicketFile => !!f);
            return (
              <li
                key={c.id}
                className={cn(
                  "rounded-xl border px-3 py-2.5",
                  c.kind === "status" || c.kind === "action"
                    ? "border-transparent bg-fill/40"
                    : "border-line"
                )}
              >
                <div className="flex items-center gap-2 text-xs text-fg-3">
                  <span className="font-medium text-fg-2">
                    {c.author_name ?? "System"}
                  </span>
                  <span>{when(c.created_at)}</span>
                  {c.action === "resident_comment" && (
                    <Badge tone="accent">
                      <Home className="size-3" />
                      Resident
                    </Badge>
                  )}
                  {c.action && c.action !== "resident_comment" && (
                    <Badge>
                      <Zap className="size-3" />
                      Update
                    </Badge>
                  )}
                  {c.visibility === "internal" && (
                    <Badge>
                      <Lock className="size-3" />
                      Staff
                    </Badge>
                  )}
                </div>
                <p className="mt-1 text-[13px] whitespace-pre-wrap text-fg">
                  {c.body}
                </p>
                {photos.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-2">
                    {photos.map((p) => (
                      <Media key={p.id} file={p} className="size-20" />
                    ))}
                  </div>
                )}
              </li>
            );
          })}
        </ol>
      </div>
    </Panel>
  );
}

/** A photo or video thumbnail that opens the full file. */
export function Media({
  file,
  className,
}: {
  file: TicketFile;
  className?: string;
}) {
  if (!file.url) return null;
  const video = file.kind === "video" || file.mime_type.startsWith("video/");
  return (
    <a
      href={file.url}
      target="_blank"
      rel="noreferrer"
      title={file.filename}
      className={cn(
        "relative block overflow-hidden rounded-lg border border-line bg-fill",
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
