"use client";

// Photos and videos a resident is about to send: pick from the camera roll
// or take one now, see them, take one back out.

import { useRef } from "react";
import { Camera, Video, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";

export const MAX_PHOTO_MB = 25;
export const MAX_VIDEO_MB = 100;

/** Keep the photos and videos that fit; say why the rest didn't. */
export function acceptMedia(list: FileList | File[]): File[] {
  const ok: File[] = [];
  for (const f of Array.from(list)) {
    const video = f.type.startsWith("video/");
    if (!video && !f.type.startsWith("image/")) {
      toast.error(`${f.name} isn't a photo or video`);
      continue;
    }
    const max = video ? MAX_VIDEO_MB : MAX_PHOTO_MB;
    if (f.size > max * 1024 * 1024) {
      toast.error(`${f.name} is over ${max} MB`);
      continue;
    }
    ok.push(f);
  }
  return ok;
}

export function MediaPicker({
  files,
  onChange,
  disabled,
}: {
  files: File[];
  onChange: (files: File[]) => void;
  disabled?: boolean;
}) {
  const roll = useRef<HTMLInputElement>(null);
  const cam = useRef<HTMLInputElement>(null);

  function add(list: FileList | null) {
    if (list) onChange([...files, ...acceptMedia(list)]);
    if (roll.current) roll.current.value = "";
    if (cam.current) cam.current.value = "";
  }

  return (
    <div>
      {files.length > 0 && (
        <ul className="mb-2 flex flex-wrap gap-2">
          {files.map((f, i) => (
            <li
              key={`${f.name}-${i}`}
              className="flex max-w-full items-center gap-1.5 rounded-lg border border-line bg-surface px-2 py-1 text-xs text-fg-2"
            >
              {f.type.startsWith("video/") ? (
                <Video className="size-3.5 shrink-0" />
              ) : (
                <Camera className="size-3.5 shrink-0" />
              )}
              <span className="truncate">{f.name}</span>
              <button
                type="button"
                aria-label={`Remove ${f.name}`}
                onClick={() => onChange(files.filter((_, j) => j !== i))}
              >
                <X className="size-3" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <input
        ref={roll}
        type="file"
        accept="image/*,video/*"
        multiple
        className="hidden"
        onChange={(e) => add(e.target.files)}
      />
      <input
        ref={cam}
        type="file"
        accept="image/*"
        capture="environment"
        className="hidden"
        onChange={(e) => add(e.target.files)}
      />
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          size="sm"
          variant="secondary"
          disabled={disabled}
          onClick={() => cam.current?.click()}
        >
          <Camera />
          Take a photo
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={disabled}
          onClick={() => roll.current?.click()}
        >
          <Video />
          Add photos or video
        </Button>
      </div>
    </div>
  );
}
