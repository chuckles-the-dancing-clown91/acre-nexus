"use client";

// Importing from another tool: pick what's in the file and where it came
// from, drop it in, check how its columns line up (with a preview of exactly
// what will happen to every row), then import. Undo is one click away.

import { useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  ArrowLeft,
  Building2,
  CheckCircle2,
  FileDown,
  FileUp,
  HardHat,
  Landmark,
  Users,
} from "lucide-react";
import { toast } from "sonner";
import {
  madeSentence,
  transfer,
  type Batch,
  type ImportKind,
  type Preview,
  type RowOutcome,
} from "@/lib/datatransfer";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const KIND_LOOK: Record<
  ImportKind,
  { icon: React.ReactNode; title: string; blurb: string }
> = {
  tenants: {
    icon: <Users />,
    title: "Rent roll",
    blurb: "Tenants and leases. Properties and units it names come in too.",
  },
  properties: {
    icon: <Building2 />,
    title: "Properties and units",
    blurb: "A property list or unit directory.",
  },
  owners: {
    icon: <Landmark />,
    title: "Owners",
    blurb: "People and companies who own the properties.",
  },
  vendors: {
    icon: <HardHat />,
    title: "Vendors",
    blurb: "Contractors, with their trades.",
  },
};

const ACTION_TONE: Record<RowOutcome["action"], Tone> = {
  created: "good",
  updated: "info",
  matched: "neutral",
  skipped: "neutral",
  error: "bad",
};

const ACTION_WORD: Record<RowOutcome["action"], string> = {
  created: "new",
  updated: "filled in",
  matched: "already here",
  skipped: "skipped",
  error: "problem",
};

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

export function ImportWizard({
  resume,
  onClose,
}: {
  /** A draft to pick up again, from the history. */
  resume?: Preview;
  onClose?: () => void;
}) {
  const qc = useQueryClient();
  const catalog = useQuery({
    queryKey: ["import-catalog"],
    queryFn: transfer.catalog,
  });
  const [kind, setKind] = useState<ImportKind>(resume?.batch.kind ?? "tenants");
  const [source, setSource] = useState<string>("");
  const [preview, setPreview] = useState<Preview | null>(resume ?? null);
  const [done, setDone] = useState<Batch | null>(null);
  const [busy, setBusy] = useState(false);
  const [drag, setDrag] = useState(false);
  const input = useRef<HTMLInputElement>(null);

  const sourceDef = catalog.data?.sources.find((s) => s.key === source);

  async function upload(file: File) {
    setBusy(true);
    try {
      const p = await transfer.upload(kind, file, source || undefined);
      setPreview(p);
      void qc.invalidateQueries({ queryKey: ["imports"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't read that file");
    } finally {
      setBusy(false);
    }
  }

  async function remap(next: Record<string, string>) {
    if (!preview) return;
    setBusy(true);
    try {
      setPreview(await transfer.update(preview.batch.id, { mapping: next }));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change that");
    } finally {
      setBusy(false);
    }
  }

  async function commit() {
    if (!preview) return;
    setBusy(true);
    try {
      const b = await transfer.commit(preview.batch.id);
      setDone(b);
      setPreview(null);
      for (const k of [
        "imports",
        "properties",
        "portfolio-summary",
        "exports",
      ]) {
        void qc.invalidateQueries({ queryKey: [k] });
      }
      toast.success(`Imported ${madeSentence(b.summary)}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "The import didn't run");
    } finally {
      setBusy(false);
    }
  }

  async function discard() {
    if (preview) {
      await transfer.discard(preview.batch.id).catch(() => undefined);
      void qc.invalidateQueries({ queryKey: ["imports"] });
    }
    setPreview(null);
    onClose?.();
  }

  if (done) return <Done batch={done} onAgain={() => setDone(null)} />;
  if (preview) {
    return (
      <Mapping
        preview={preview}
        fields={
          catalog.data?.kinds.find((k) => k.key === preview.batch.kind)
            ?.fields ?? []
        }
        busy={busy}
        onRemap={remap}
        onCommit={commit}
        onDiscard={discard}
      />
    );
  }

  return (
    <div className="space-y-4">
      <Panel>
        <PanelHeader
          title="What's in the file?"
          description="Most tools export each of these as its own report."
        />
        <div className="grid gap-2 p-5 sm:grid-cols-2 xl:grid-cols-4">
          {(Object.keys(KIND_LOOK) as ImportKind[]).map((k) => (
            <button
              key={k}
              type="button"
              onClick={() => setKind(k)}
              aria-pressed={kind === k}
              className={cn(
                "flex flex-col items-start gap-2 rounded-2xl border p-4 text-left transition",
                kind === k
                  ? "border-accent bg-accent/10"
                  : "border-line hover:bg-fill-2"
              )}
            >
              <span className="flex size-9 items-center justify-center rounded-xl border border-line bg-fill text-fg-2 [&_svg]:size-[18px]">
                {KIND_LOOK[k].icon}
              </span>
              <span className="text-[14px] font-semibold text-fg">
                {KIND_LOOK[k].title}
              </span>
              <span className="text-xs text-fg-3">{KIND_LOOK[k].blurb}</span>
            </button>
          ))}
        </div>
      </Panel>

      <Panel>
        <PanelHeader
          title="Where is it from?"
          description="We work it out from the columns; picking the tool shows where its export lives."
        />
        <div className="grid gap-4 p-5 md:grid-cols-[16rem_minmax(0,1fr)]">
          <select
            className={field}
            value={source}
            onChange={(e) => setSource(e.target.value)}
            aria-label="Source"
          >
            <option value="">Work it out from the file</option>
            {catalog.data?.sources.map((s) => (
              <option key={s.key} value={s.key}>
                {s.label}
              </option>
            ))}
          </select>
          <p className="self-center text-[13px] text-fg-3">
            {sourceDef?.how ??
              "AppFolio, Buildium, Yardi Breeze, Rent Manager, DoorLoop, or any spreadsheet with a header row."}
          </p>
        </div>
      </Panel>

      <div
        onDragOver={(e) => {
          e.preventDefault();
          setDrag(true);
        }}
        onDragLeave={() => setDrag(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDrag(false);
          const f = e.dataTransfer.files[0];
          if (f) void upload(f);
        }}
        className={cn(
          "flex flex-col items-center gap-3 rounded-2xl border-2 border-dashed px-6 py-10 text-center transition",
          drag ? "border-accent bg-accent/5" : "border-line"
        )}
      >
        <span className="flex size-12 items-center justify-center rounded-2xl border border-line bg-fill text-fg-2">
          <FileUp className="size-5" />
        </span>
        <div>
          <div className="text-[15px] font-semibold text-fg">
            Drop the {KIND_LOOK[kind].title.toLowerCase()} CSV here
          </div>
          <div className="mt-1 text-xs text-fg-3">
            Up to {catalog.data?.max_rows.toLocaleString() ?? "5,000"} rows.
            Nothing is saved until you check the preview and import.
          </div>
        </div>
        <div className="flex flex-wrap justify-center gap-2">
          <Button onClick={() => input.current?.click()} disabled={busy}>
            {busy ? "Reading…" : "Choose a file"}
          </Button>
          <Button
            variant="ghost"
            onClick={() =>
              transfer
                .template(kind)
                .catch((e) =>
                  toast.error(e instanceof Error ? e.message : "No template")
                )
            }
          >
            <FileDown />
            Template
          </Button>
        </div>
        <input
          ref={input}
          type="file"
          accept=".csv,.tsv,.txt,text/csv"
          className="hidden"
          aria-label="CSV file"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void upload(f);
            e.target.value = "";
          }}
        />
      </div>
    </div>
  );
}

function Mapping({
  preview,
  fields,
  busy,
  onRemap,
  onCommit,
  onDiscard,
}: {
  preview: Preview;
  fields: { key: string; label: string; required: boolean; hint: string }[];
  busy: boolean;
  onRemap: (m: Record<string, string>) => void;
  onCommit: () => void;
  onDiscard: () => void;
}) {
  const [filter, setFilter] = useState<"all" | RowOutcome["action"]>("all");
  const [showAll, setShowAll] = useState(false);
  const { batch, headers, sample, counts, missing } = preview;
  const column = (h: string) => headers.indexOf(h);
  const rows = useMemo(
    () => preview.rows.filter((r) => filter === "all" || r.action === filter),
    [preview.rows, filter]
  );
  const willRun = counts.rows - counts.errors;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2 text-[13px] text-fg-3">
        <button
          type="button"
          onClick={onDiscard}
          className="inline-flex items-center gap-1.5 transition hover:text-fg"
        >
          <ArrowLeft className="size-4" />
          Start over
        </button>
        <span>·</span>
        <span className="font-medium text-fg">{batch.filename}</span>
        <Badge tone="info">
          {batch.source === "generic"
            ? "A spreadsheet"
            : `Looks like ${batch.source_label}`}
        </Badge>
        <span>
          {counts.rows} rows · {batch.kind_label}
        </span>
      </div>

      <Panel>
        <PanelHeader
          title="Match the columns"
          description="We matched what we could. Change any that are wrong; the preview below follows."
        />
        {missing.length > 0 && (
          <div className="mx-5 mt-4 rounded-xl border border-warn/30 bg-warn/10 px-3 py-2 text-[13px] text-fg">
            Choose a column for {missing.join(", ")} to see the preview.
          </div>
        )}
        <div className="overflow-x-auto p-2 pt-3">
          <table className="w-full min-w-[640px] text-[13px]">
            <thead>
              <tr className="text-left text-xs text-fg-3">
                <th className="px-3 py-2 font-medium">Vantedge field</th>
                <th className="px-3 py-2 font-medium">Column in your file</th>
                <th className="px-3 py-2 font-medium">First rows</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {fields
                .filter((f) => showAll || f.required || batch.mapping[f.key])
                .map((f) => {
                  const h = batch.mapping[f.key] ?? "";
                  const i = h ? column(h) : -1;
                  return (
                    <tr key={f.key}>
                      <td className="px-3 py-2 align-top">
                        <div className="font-medium text-fg">
                          {f.label}
                          {f.required && <span className="text-bad"> *</span>}
                        </div>
                        {f.hint && (
                          <div className="text-xs text-fg-3">{f.hint}</div>
                        )}
                      </td>
                      <td className="px-3 py-2 align-top">
                        <select
                          className={cn(field, "max-w-64")}
                          value={h}
                          disabled={busy}
                          aria-label={`Column for ${f.label}`}
                          onChange={(e) => {
                            const next = { ...batch.mapping };
                            if (e.target.value) next[f.key] = e.target.value;
                            else delete next[f.key];
                            onRemap(next);
                          }}
                        >
                          <option value="">Not in this file</option>
                          {headers.map((x) => (
                            <option key={x} value={x}>
                              {x}
                            </option>
                          ))}
                        </select>
                      </td>
                      <td className="max-w-80 px-3 py-2 align-top text-xs text-fg-3">
                        {i >= 0
                          ? sample
                              .map((r) => r[i])
                              .filter(Boolean)
                              .slice(0, 3)
                              .join(" · ") || "(blank)"
                          : ""}
                      </td>
                    </tr>
                  );
                })}
            </tbody>
          </table>
          {fields.some((f) => !f.required && !batch.mapping[f.key]) && (
            <button
              type="button"
              onClick={() => setShowAll((v) => !v)}
              className="mx-3 mt-2 text-xs font-medium text-accent hover:underline"
            >
              {showAll
                ? "Hide the fields this file doesn't have"
                : `Show ${fields.filter((f) => !f.required && !batch.mapping[f.key]).length} more fields this file doesn't have`}
            </button>
          )}
        </div>
      </Panel>

      {missing.length === 0 && (
        <Panel>
          <PanelHeader
            title="What will happen"
            description="This is the import run for real and rolled back: committing does exactly this."
          />
          <div className="flex flex-wrap gap-2 px-5 pt-4">
            <Chip tone="good" label="New" value={madeSentence(counts)} />
            {counts.matched > 0 && (
              <Chip label="Already here" value={rowsWord(counts.matched)} />
            )}
            {counts.updated > 0 && (
              <Chip
                tone="info"
                label="Filled in"
                value={rowsWord(counts.updated)}
              />
            )}
            {counts.skipped > 0 && (
              <Chip label="Skipped" value={rowsWord(counts.skipped)} />
            )}
            {counts.errors > 0 && (
              <Chip
                tone="bad"
                label="Problems"
                value={rowsWord(counts.errors)}
              />
            )}
          </div>
          <div className="flex gap-1 px-5 pt-4" role="tablist">
            {(["all", "error", "created", "matched"] as const).map((k) => (
              <button
                key={k}
                role="tab"
                aria-selected={filter === k}
                onClick={() => setFilter(k)}
                className={cn(
                  "rounded-lg px-3 py-1.5 text-xs font-medium transition",
                  filter === k ? "bg-fill-2 text-fg" : "text-fg-3 hover:text-fg"
                )}
              >
                {k === "all"
                  ? "All"
                  : ACTION_WORD[k][0].toUpperCase() + ACTION_WORD[k].slice(1)}
              </button>
            ))}
          </div>
          <ul className="max-h-[28rem] divide-y divide-line overflow-y-auto p-2">
            {rows.length === 0 && (
              <li className="px-3 py-6 text-center text-[13px] text-fg-3">
                Nothing here.
              </li>
            )}
            {rows.map((r) => (
              <li
                key={`${r.line}-${r.action}`}
                className="flex items-center gap-3 px-3 py-2 text-[13px]"
              >
                <span className="figure w-12 shrink-0 text-right text-xs text-fg-4">
                  line {r.line}
                </span>
                <span className="min-w-0 flex-1 truncate text-fg">
                  {r.label}
                </span>
                <span
                  className={cn(
                    "hidden truncate text-xs sm:block",
                    r.action === "error" ? "text-bad" : "text-fg-3"
                  )}
                >
                  {r.message}
                </span>
                <Badge tone={ACTION_TONE[r.action]}>
                  {ACTION_WORD[r.action]}
                </Badge>
              </li>
            ))}
          </ul>
        </Panel>
      )}

      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:items-center sm:justify-end">
        {counts.errors > 0 && missing.length === 0 && (
          <span className="text-xs text-fg-3 sm:mr-auto">
            Rows with problems are skipped; fix them in the file and import it
            again. Rows already here are matched, not doubled.
          </span>
        )}
        <Button variant="ghost" onClick={onDiscard} disabled={busy}>
          Discard
        </Button>
        <Button
          onClick={onCommit}
          disabled={busy || missing.length > 0 || willRun === 0}
        >
          {busy
            ? "Working…"
            : `Import ${willRun} row${willRun === 1 ? "" : "s"}`}
        </Button>
      </div>
    </div>
  );
}

function rowsWord(n: number): string {
  return `${n} row${n === 1 ? "" : "s"}`;
}

function Chip({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "good" | "bad" | "info";
}) {
  return (
    <span
      className={cn(
        "rounded-xl border px-3 py-1.5 text-[13px]",
        tone === "good"
          ? "border-good/30 bg-good/10"
          : tone === "bad"
            ? "border-bad/30 bg-bad/10"
            : tone === "info"
              ? "border-info/30 bg-info/10"
              : "border-line bg-fill"
      )}
    >
      <span className="text-fg-3">{label}: </span>
      <span className="font-medium text-fg">{value}</span>
    </span>
  );
}

function Done({ batch, onAgain }: { batch: Batch; onAgain: () => void }) {
  const errors = batch.errors ?? [];
  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-4"
    >
      <Panel className="flex flex-col items-start gap-4 p-6 sm:flex-row sm:items-center">
        <span className="flex size-12 shrink-0 items-center justify-center rounded-2xl border border-good/30 bg-good/10 text-good">
          <CheckCircle2 className="size-6" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[17px] font-semibold text-fg">
            Imported {madeSentence(batch.summary)}
          </div>
          <div className="mt-1 text-[13px] text-fg-3">
            From {batch.filename}
            {batch.summary.matched
              ? ` · ${batch.summary.matched} already here`
              : ""}
            {errors.length ? ` · ${errors.length} skipped with problems` : ""}.
            Changed your mind? Undo it from the history below.
          </div>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={onAgain}>
            Import another file
          </Button>
          {batch.kind === "tenants" || batch.kind === "properties" ? (
            <Button asChild>
              <Link href="/console/properties">See properties</Link>
            </Button>
          ) : null}
        </div>
      </Panel>
      {errors.length > 0 && (
        <Panel>
          <PanelHeader
            title="Skipped rows"
            description="Fix these in the file and import it again; the rest will match."
          />
          <ul className="divide-y divide-line p-2">
            {errors.map((r) => (
              <li key={r.line} className="flex gap-3 px-3 py-2 text-[13px]">
                <span className="figure w-12 shrink-0 text-right text-xs text-fg-4">
                  line {r.line}
                </span>
                <span className="min-w-0 flex-1 truncate text-fg">
                  {r.label}
                </span>
                <span className="text-xs text-bad">{r.message}</span>
              </li>
            ))}
          </ul>
        </Panel>
      )}
    </motion.div>
  );
}

export function WizardSkeleton() {
  return <Skeleton className="h-96 rounded-2xl" />;
}
