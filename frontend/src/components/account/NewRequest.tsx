"use client";

// A resident asks for a repair: what's wrong, where, whether we can let
// ourselves in, and photos or a video of it.

import { useState } from "react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";
import { MediaPicker } from "./MediaPicker";

export const REQUEST_KINDS = [
  { key: "plumbing", label: "Plumbing" },
  { key: "electrical", label: "Electrical" },
  { key: "hvac", label: "Heating or cooling" },
  { key: "appliance", label: "Appliance" },
  { key: "structural", label: "Doors, walls, floors" },
  { key: "general", label: "Something else" },
] as const;

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2.5 text-[14px] text-fg outline-none focus:border-accent";

export function NewRequest({
  prefill,
  onCancel,
  onCreated,
}: {
  prefill: { title: string; category: string; description: string } | null;
  onCancel: () => void;
  onCreated: (id: string) => void;
}) {
  const known = REQUEST_KINDS.some((k) => k.key === prefill?.category);
  const [title, setTitle] = useState(prefill?.title ?? "");
  const [category, setCategory] = useState(
    known ? prefill!.category : "general"
  );
  const [description, setDescription] = useState(prefill?.description ?? "");
  const [location, setLocation] = useState("");
  const [enter, setEnter] = useState(false);
  const [access, setAccess] = useState("");
  const [urgent, setUrgent] = useState(false);
  const [files, setFiles] = useState<File[]>([]);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim()) {
      toast.error("Say what needs fixing.");
      return;
    }
    setBusy(true);
    try {
      const t = await api.createMyTicket({
        title: title.trim(),
        description: description.trim() || undefined,
        category,
        priority: urgent ? "urgent" : "normal",
        location: location.trim() || undefined,
        permission_to_enter: enter,
        access_notes: access.trim() || undefined,
      });
      // The request exists now; files go up one by one and land on it.
      let failed = 0;
      const ids: string[] = [];
      for (const f of files) {
        try {
          ids.push((await api.uploadMyTicketPhoto(t.id, f)).id);
        } catch {
          failed += 1;
        }
      }
      if (ids.length) {
        await api
          .addMyTicketComment(
            t.id,
            ids.length === 1 ? "Added a photo." : "Added photos.",
            ids
          )
          .catch(() => undefined);
      }
      if (failed)
        toast.error(
          `Request sent, but ${failed} ${failed === 1 ? "file" : "files"} didn't upload. Try adding ${failed === 1 ? "it" : "them"} again.`
        );
      else toast.success("Request sent. We'll keep you posted here.");
      onCreated(t.id);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Couldn't send it");
      setBusy(false);
    }
  }

  return (
    <Panel className="p-5">
      <form onSubmit={submit} className="space-y-4">
        <div>
          <h2 className="text-[18px] font-semibold text-fg">
            Ask for a repair
          </h2>
          <p className="mt-0.5 text-[13px] text-fg-3">
            Photos or a short video help us bring the right part the first time.
          </p>
        </div>
        <label className="block">
          <span className="mb-1 block text-xs font-medium text-fg-2">
            What needs fixing?
          </span>
          <input
            className={field}
            placeholder="e.g. The kitchen faucet drips"
            value={title}
            maxLength={200}
            onChange={(e) => setTitle(e.target.value)}
            autoFocus={!prefill?.title}
          />
        </label>
        <div className="grid gap-4 sm:grid-cols-2">
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-fg-2">
              Kind of problem
            </span>
            <select
              className={field}
              value={category}
              onChange={(e) => setCategory(e.target.value)}
            >
              {REQUEST_KINDS.map((k) => (
                <option key={k.key} value={k.key}>
                  {k.label}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-fg-2">
              Where in your home?
            </span>
            <input
              className={field}
              placeholder="Kitchen, hall bath…"
              value={location}
              onChange={(e) => setLocation(e.target.value)}
            />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-xs font-medium text-fg-2">
            Tell us more
          </span>
          <textarea
            className={`${field} min-h-[96px]`}
            placeholder="When it started, what you've tried, anything we should know"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </label>
        <MediaPicker files={files} onChange={setFiles} disabled={busy} />
        <div className="space-y-2 rounded-xl border border-line bg-fill/40 p-3 text-[13px]">
          <label className="flex items-start gap-2">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={enter}
              onChange={(e) => setEnter(e.target.checked)}
            />
            <span className="text-fg">
              You can come in if I&apos;m not home
            </span>
          </label>
          {enter && (
            <input
              className={field}
              placeholder="Pets, alarm code, where the key is…"
              value={access}
              onChange={(e) => setAccess(e.target.value)}
            />
          )}
          <label className="flex items-start gap-2">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={urgent}
              onChange={(e) => setUrgent(e.target.checked)}
            />
            <span className="text-fg">
              It&apos;s urgent: water is leaking, no heat, no power, or a door
              won&apos;t lock
            </span>
          </label>
        </div>
        <p className="text-xs text-fg-3">
          Gas smell, fire or flooding? Leave and call 911 first.
        </p>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy}>
            {busy ? "Sending…" : "Send request"}
          </Button>
        </div>
      </form>
    </Panel>
  );
}
