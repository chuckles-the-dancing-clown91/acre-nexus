"use client";

// English / Español for the messages a resident gets.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Languages } from "lucide-react";
import { toast } from "sonner";
import { language, LANGUAGE_WORDS, type Language } from "@/lib/language";
import { fieldClass } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/** For staff on a lease: what the resident's messages are written in. */
export function LeaseLanguage({
  leaseId,
  canEdit,
}: {
  leaseId: string;
  canEdit: boolean;
}) {
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["lease-language", leaseId],
    queryFn: () => language.lease(leaseId),
  });
  const m = useMutation({
    mutationFn: (l: Language) => language.setLease(leaseId, l),
    onSuccess: (r) => {
      qc.setQueryData(["lease-language", leaseId], r);
      toast.success(
        `Messages to this resident now go in ${LANGUAGE_WORDS[r.language]}`
      );
    },
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Couldn't change it"),
  });
  if (!q.data) return null;
  return (
    <label
      className="inline-flex items-center gap-1.5 text-[13px] text-fg-2"
      title="The language reminders, receipts and updates to this resident are written in"
    >
      <Languages className="size-3.5 text-fg-3" />
      <span className="sr-only">Messages in</span>
      <select
        aria-label="Messages in"
        className={cn(fieldClass, "py-1 text-[13px]")}
        value={q.data.language}
        disabled={!canEdit || m.isPending}
        onChange={(e) => m.mutate(e.target.value as Language)}
      >
        {(Object.keys(LANGUAGE_WORDS) as Language[]).map((l) => (
          <option key={l} value={l}>
            {LANGUAGE_WORDS[l]}
          </option>
        ))}
      </select>
    </label>
  );
}

/** For the resident: the language they get messages in. */
export function MyLanguage() {
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["my-language"], queryFn: language.mine });
  const m = useMutation({
    mutationFn: language.setMine,
    onSuccess: (r) => {
      qc.setQueryData(["my-language"], r);
      toast.success(
        r.language === "es"
          ? "Le escribiremos en español"
          : "We'll write to you in English"
      );
    },
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Couldn't change it"),
  });
  const current = q.data?.language;
  return (
    <div
      className="flex flex-wrap gap-2"
      role="radiogroup"
      aria-label="Messages in"
    >
      {(Object.keys(LANGUAGE_WORDS) as Language[]).map((l) => (
        <button
          key={l}
          type="button"
          role="radio"
          aria-checked={current === l}
          disabled={!q.data || m.isPending}
          onClick={() => current !== l && m.mutate(l)}
          className={cn(
            "rounded-xl border px-4 py-2 text-[14px] font-medium transition",
            current === l
              ? "border-accent bg-accent/10 text-fg"
              : "border-line text-fg-2 hover:border-fg-4"
          )}
        >
          {LANGUAGE_WORDS[l]}
        </button>
      ))}
    </div>
  );
}
