// Import & export: other tools' CSVs in (upload → map → preview → commit,
// and undo), the workspace's data out as CSV.

import {
  actingTenant,
  API_BASE,
  ApiError,
  request,
  tokenStore,
} from "@/lib/api";

export type ImportKind = "properties" | "tenants" | "owners" | "vendors";

export interface FieldDef {
  key: string;
  label: string;
  required: boolean;
  hint: string;
}

export interface Source {
  key: string;
  label: string;
  how: string;
}

export interface Dataset {
  key: string;
  label: string;
  description: string;
  importable: boolean;
}

export interface Catalog {
  sources: Source[];
  kinds: { key: ImportKind; label: string; fields: FieldDef[] }[];
  datasets: Dataset[];
  max_rows: number;
}

export interface Counts {
  rows: number;
  properties: number;
  units: number;
  leases: number;
  owners: number;
  vendors: number;
  updated: number;
  matched: number;
  skipped: number;
  errors: number;
}

export interface Batch {
  id: string;
  kind: ImportKind;
  kind_label: string;
  source: string;
  source_label: string;
  filename: string;
  status: "draft" | "done" | "undone";
  row_count: number;
  mapping: Record<string, string>;
  summary: Partial<Counts> & {
    undo?: { removed: Record<string, number>; kept: number };
  };
  errors: RowOutcome[];
  created_at: string;
  committed_at: string | null;
  undone_at: string | null;
}

export interface RowOutcome {
  line: number;
  action: "created" | "updated" | "matched" | "skipped" | "error";
  label: string;
  message: string;
}

export interface Preview {
  batch: Batch;
  headers: string[];
  sample: string[][];
  missing: string[];
  counts: Counts;
  rows: RowOutcome[];
}

export interface UndoReport {
  removed: Record<string, number>;
  kept: { t: string; id: string; reason: string }[];
}

export interface DatasetCount extends Dataset {
  rows: number;
}

function authHeaders(): Record<string, string> {
  const h: Record<string, string> = {};
  const token = tokenStore.access;
  if (token) h["Authorization"] = `Bearer ${token}`;
  const acting = actingTenant.get();
  if (acting) h["X-Tenant"] = acting;
  return h;
}

async function fail(res: Response): Promise<never> {
  let message = res.statusText;
  try {
    const data = await res.json();
    message = data?.error?.message ?? message;
  } catch {
    /* not JSON */
  }
  throw new ApiError(res.status, "error", message);
}

/** Save a blob as a file. */
export function saveFile(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** The filename a download says it has. */
function filenameOf(res: Response, fallback: string): string {
  const cd = res.headers.get("Content-Disposition") ?? "";
  return /filename="([^"]+)"/.exec(cd)?.[1] ?? fallback;
}

async function download(path: string, fallback: string) {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: authHeaders(),
    cache: "no-store",
  });
  if (!res.ok) await fail(res);
  saveFile(await res.blob(), filenameOf(res, fallback));
}

export const transfer = {
  catalog: () => request<Catalog>("/imports/catalog", { auth: true }),
  history: () => request<Batch[]>("/imports", { auth: true }),
  /** Send the file as it is; the server works out the rest. */
  upload: async (
    kind: ImportKind,
    file: File,
    source?: string
  ): Promise<Preview> => {
    const qs = new URLSearchParams({ kind, filename: file.name });
    if (source) qs.set("source", source);
    const res = await fetch(`${API_BASE}/imports?${qs}`, {
      method: "POST",
      headers: { ...authHeaders(), "Content-Type": "text/csv" },
      body: file,
      cache: "no-store",
    });
    if (!res.ok) await fail(res);
    return (await res.json()) as Preview;
  },
  update: (
    id: string,
    body: { mapping?: Record<string, string>; source?: string; kind?: string }
  ) =>
    request<Preview>(`/imports/${id}`, { method: "PATCH", auth: true, body }),
  preview: (id: string) =>
    request<Preview>(`/imports/${id}/preview`, { auth: true }),
  commit: (id: string) =>
    request<Batch>(`/imports/${id}/commit`, { method: "POST", auth: true }),
  discard: (id: string) =>
    request<{ ok: boolean }>(`/imports/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  undo: (id: string) =>
    request<{ batch: Batch; report: UndoReport }>(`/imports/${id}/undo`, {
      method: "POST",
      auth: true,
    }),
  template: (kind: ImportKind) =>
    download(`/import-templates/${kind}`, `vantedge-${kind}-template.csv`),
  exports: () => request<DatasetCount[]>("/exports", { auth: true }),
  download: (key: string) =>
    download(`/exports/${key}`, key === "all" ? "export.zip" : `${key}.csv`),
};

/** "3 properties, 12 units and 11 leases" from what an import made. */
export function madeSentence(c: Partial<Counts>): string {
  const parts = (
    [
      ["property", "properties", c.properties],
      ["unit", "units", c.units],
      ["lease", "leases", c.leases],
      ["owner", "owners", c.owners],
      ["vendor", "vendors", c.vendors],
    ] as const
  )
    .filter(([, , n]) => (n ?? 0) > 0)
    .map(([one, many, n]) => `${n} ${n === 1 ? one : many}`);
  if (parts.length === 0) return "nothing new";
  if (parts.length === 1) return parts[0];
  return `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
}
