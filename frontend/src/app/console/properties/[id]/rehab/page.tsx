"use client";

// Rehab / construction workspace for a property (roadmap Phase 7, issue #40):
// a renovation budget with scope lines, change orders, and draw requests —
// each draw carrying progress photos (document service) and generated lien
// waivers.

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import {
  api,
  type RehabProject,
  type RehabProjectDetail,
  type RehabDraw,
  type RehabDrawDetail,
} from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile, statusTone } from "@/components/ui";
import { DocumentsCard } from "@/components/DocumentsCard";
import { logError } from "@/lib/log";

const WAIVER_TYPES = [
  { key: "conditional_progress", label: "Conditional — progress" },
  { key: "unconditional_progress", label: "Unconditional — progress" },
  { key: "conditional_final", label: "Conditional — final" },
  { key: "unconditional_final", label: "Unconditional — final" },
];

const DRAW_NEXT: Record<string, { label: string; status: string }[]> = {
  requested: [
    { label: "Approve", status: "approved" },
    { label: "Reject", status: "rejected" },
  ],
  approved: [{ label: "Mark funded", status: "funded" }],
  funded: [],
  rejected: [],
};

const dollars = (v: string): number | undefined => {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : Math.round(n * 100);
};

function fieldClass() {
  return "w-full rounded-lg border border-line bg-surface-2 px-3 py-2 text-sm";
}

function CreateProjectForm({
  propertyId,
  onCreated,
}: {
  propertyId: string;
  onCreated: (d: RehabProjectDetail) => void;
}) {
  const [name, setName] = useState("");
  const [budget, setBudget] = useState("");
  const [contingency, setContingency] = useState("10");
  const [saving, setSaving] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    setSaving(true);
    try {
      const d = await api.createRehabProject(propertyId, {
        name: name.trim(),
        budget_cents: dollars(budget),
        contingency_bps: contingency.trim()
          ? Math.round(Number(contingency) * 100)
          : undefined,
      });
      onCreated(d);
    } catch (e) {
      logError("failed to create rehab project", e);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card className="p-5">
      <h2 className="mb-3 font-display text-lg font-bold">
        Start a rehab project
      </h2>
      <form onSubmit={submit} className="grid gap-3 sm:grid-cols-3">
        <label className="text-sm sm:col-span-3">
          <span className="mb-1 block text-xs font-semibold text-ink-3">
            Project name
          </span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Full gut renovation"
            className={fieldClass()}
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block text-xs font-semibold text-ink-3">
            Budget ($)
          </span>
          <input
            value={budget}
            onChange={(e) => setBudget(e.target.value)}
            inputMode="decimal"
            className={fieldClass()}
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block text-xs font-semibold text-ink-3">
            Contingency (%)
          </span>
          <input
            value={contingency}
            onChange={(e) => setContingency(e.target.value)}
            inputMode="decimal"
            className={fieldClass()}
          />
        </label>
        <div className="flex items-end">
          <Button type="submit" disabled={saving}>
            {saving ? "Creating…" : "Create"}
          </Button>
        </div>
      </form>
    </Card>
  );
}

function BudgetSummary({ p }: { p: RehabProject }) {
  const pctDrawn =
    p.adjusted_budget_cents > 0
      ? Math.min(100, (p.drawn_cents / p.adjusted_budget_cents) * 100)
      : 0;
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatTile
          label="Adjusted budget"
          value={p.adjusted_budget_label}
          icon="dollar"
        />
        <StatTile label="Drawn" value={p.drawn_label} icon="dollar" />
        <StatTile
          label="Pending draws"
          value={p.pending_draws_label}
          icon="dollar"
        />
        <StatTile label="Remaining" value={p.remaining_label} icon="dollar" />
      </div>
      <Card className="p-4">
        <div className="mb-1 flex justify-between text-xs text-ink-3">
          <span>
            Base {p.base_budget_label} + change orders{" "}
            {p.approved_change_orders_label} · contingency {p.contingency_label}{" "}
            ({p.contingency_pct.toFixed(0)}%)
          </span>
          <span>{pctDrawn.toFixed(0)}% drawn</span>
        </div>
        <div className="h-3 overflow-hidden rounded-full bg-surface-2">
          <div
            className="h-full rounded-full bg-accent"
            style={{ width: `${pctDrawn}%` }}
          />
        </div>
      </Card>
    </div>
  );
}

export default function RehabPage() {
  const params = useParams<{ id: string }>();
  const propertyId = params?.id;
  const { can } = useAuth();
  const canManage = can("rehab:manage");

  const [detail, setDetail] = useState<RehabProjectDetail | null>(null);
  const [hasProject, setHasProject] = useState<boolean | null>(null);
  const [drawDetail, setDrawDetail] = useState<RehabDrawDetail | null>(null);
  const [selectedDraw, setSelectedDraw] = useState<string | null>(null);

  const load = useCallback(() => {
    if (!propertyId) return;
    api
      .rehabProjects(propertyId)
      .then((ps) => {
        setHasProject(ps.length > 0);
        if (ps[0]) {
          api
            .rehabProject(ps[0].id)
            .then(setDetail)
            .catch((e) => logError("failed to load rehab project", e));
        }
      })
      .catch((e) => logError("failed to load rehab projects", e));
  }, [propertyId]);

  useEffect(() => {
    load();
  }, [load]);

  const loadDraw = useCallback((id: string) => {
    setSelectedDraw(id);
    api
      .rehabDraw(id)
      .then(setDrawDetail)
      .catch((e) => logError("failed to load draw", e));
  }, []);

  if (!propertyId) return null;

  return (
    <div className="space-y-6">
      <Link
        href={`/console/properties/${propertyId}`}
        className="text-sm text-ink-3 hover:text-ink"
      >
        ← Property
      </Link>
      <h1 className="font-display text-2xl font-bold">
        Rehab &amp; construction
      </h1>

      {hasProject === false && canManage && (
        <CreateProjectForm
          propertyId={propertyId}
          onCreated={(d) => {
            setDetail(d);
            setHasProject(true);
          }}
        />
      )}
      {hasProject === false && !canManage && (
        <Card className="p-5 text-sm text-ink-3">
          No rehab project on this property yet.
        </Card>
      )}

      {detail && (
        <>
          <div className="flex flex-wrap items-center gap-3">
            <h2 className="font-display text-lg font-bold">{detail.name}</h2>
            <Badge tone={statusTone(detail.status)}>{detail.status}</Badge>
          </div>

          <BudgetSummary p={detail} />

          <div className="grid gap-4 lg:grid-cols-2">
            <ScopeLines
              detail={detail}
              canManage={canManage}
              onChange={setDetail}
            />
            <ChangeOrders
              detail={detail}
              canManage={canManage}
              onChange={setDetail}
            />
          </div>

          <Draws
            detail={detail}
            canManage={canManage}
            selectedDraw={selectedDraw}
            onSelect={loadDraw}
            onChange={(d) => {
              setDetail(d);
            }}
          />

          {selectedDraw && drawDetail && (
            <DrawPanel
              draw={drawDetail}
              canManage={canManage}
              reload={() => loadDraw(selectedDraw)}
            />
          )}
        </>
      )}
    </div>
  );
}

function ScopeLines({
  detail,
  canManage,
  onChange,
}: {
  detail: RehabProjectDetail;
  canManage: boolean;
  onChange: (d: RehabProjectDetail) => void;
}) {
  const [cat, setCat] = useState("");
  const [amt, setAmt] = useState("");

  async function add(e: React.FormEvent) {
    e.preventDefault();
    if (!cat.trim()) return;
    try {
      const d = await api.createRehabLine(detail.id, {
        category: cat.trim(),
        budget_cents: dollars(amt),
      });
      onChange(d);
      setCat("");
      setAmt("");
    } catch (e) {
      logError("failed to add line", e);
    }
  }

  async function remove(id: string) {
    try {
      onChange(await api.deleteRehabLine(id));
    } catch (e) {
      logError("failed to delete line", e);
    }
  }

  return (
    <Card className="space-y-3 p-4">
      <h3 className="font-display text-sm font-bold">
        Scope &amp; budget lines
        <span className="ml-2 font-normal text-ink-3">
          {detail.lines_budget_label} itemised
        </span>
      </h3>
      {detail.lines.length === 0 ? (
        <p className="text-sm text-ink-3">No lines yet.</p>
      ) : (
        <ul className="divide-y divide-line text-sm">
          {detail.lines.map((l) => (
            <li key={l.id} className="flex items-center justify-between py-2">
              <span>{l.category}</span>
              <span className="flex items-center gap-3">
                <span className="font-semibold tabular-nums">
                  {l.budget_label}
                </span>
                {canManage && (
                  <button
                    onClick={() => remove(l.id)}
                    className="text-xs text-ink-3 hover:text-bad"
                  >
                    remove
                  </button>
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
      {canManage && (
        <form onSubmit={add} className="flex gap-2">
          <input
            value={cat}
            onChange={(e) => setCat(e.target.value)}
            placeholder="Category"
            className={fieldClass()}
          />
          <input
            value={amt}
            onChange={(e) => setAmt(e.target.value)}
            inputMode="decimal"
            placeholder="$"
            className="w-28 rounded-lg border border-line bg-surface-2 px-3 py-2 text-sm"
          />
          <Button type="submit" variant="outline">
            Add
          </Button>
        </form>
      )}
    </Card>
  );
}

function ChangeOrders({
  detail,
  canManage,
  onChange,
}: {
  detail: RehabProjectDetail;
  canManage: boolean;
  onChange: (d: RehabProjectDetail) => void;
}) {
  const [desc, setDesc] = useState("");
  const [amt, setAmt] = useState("");

  async function add(e: React.FormEvent) {
    e.preventDefault();
    const cents = dollars(amt);
    if (!desc.trim() || cents === undefined) return;
    try {
      const d = await api.createChangeOrder(detail.id, {
        description: desc.trim(),
        amount_cents: cents,
      });
      onChange(d);
      setDesc("");
      setAmt("");
    } catch (e) {
      logError("failed to add change order", e);
    }
  }

  async function decide(id: string, approve: boolean) {
    try {
      onChange(await api.decideChangeOrder(id, approve));
    } catch (e) {
      logError("failed to decide change order", e);
    }
  }

  return (
    <Card className="space-y-3 p-4">
      <h3 className="font-display text-sm font-bold">Change orders</h3>
      {detail.change_orders.length === 0 ? (
        <p className="text-sm text-ink-3">No change orders.</p>
      ) : (
        <ul className="divide-y divide-line text-sm">
          {detail.change_orders.map((c) => (
            <li
              key={c.id}
              className="flex items-center justify-between gap-2 py-2"
            >
              <span className="min-w-0">
                <span className="block truncate">{c.description}</span>
                <span className="text-xs text-ink-3">{c.amount_label}</span>
              </span>
              <span className="flex shrink-0 items-center gap-2">
                <Badge tone={statusTone(c.status)}>{c.status}</Badge>
                {canManage && c.status === "pending" && (
                  <>
                    <button
                      onClick={() => decide(c.id, true)}
                      className="text-xs font-semibold text-good hover:underline"
                    >
                      approve
                    </button>
                    <button
                      onClick={() => decide(c.id, false)}
                      className="text-xs font-semibold text-bad hover:underline"
                    >
                      reject
                    </button>
                  </>
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
      {canManage && (
        <form onSubmit={add} className="flex gap-2">
          <input
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
            placeholder="Description"
            className={fieldClass()}
          />
          <input
            value={amt}
            onChange={(e) => setAmt(e.target.value)}
            inputMode="decimal"
            placeholder="±$"
            className="w-28 rounded-lg border border-line bg-surface-2 px-3 py-2 text-sm"
          />
          <Button type="submit" variant="outline">
            Add
          </Button>
        </form>
      )}
    </Card>
  );
}

function Draws({
  detail,
  canManage,
  selectedDraw,
  onSelect,
  onChange,
}: {
  detail: RehabProjectDetail;
  canManage: boolean;
  selectedDraw: string | null;
  onSelect: (id: string) => void;
  onChange: (d: RehabProjectDetail) => void;
}) {
  const [title, setTitle] = useState("");
  const [amt, setAmt] = useState("");
  const [adding, setAdding] = useState(false);

  async function add(e: React.FormEvent) {
    e.preventDefault();
    const cents = dollars(amt);
    if (!title.trim() || cents === undefined) return;
    try {
      const d = await api.createRehabDraw(detail.id, {
        title: title.trim(),
        amount_cents: cents,
      });
      onChange(d);
      setTitle("");
      setAmt("");
      setAdding(false);
    } catch (e) {
      logError("failed to create draw", e);
    }
  }

  async function setStatus(draw: RehabDraw, status: string) {
    try {
      onChange(await api.setDrawStatus(draw.id, status));
    } catch (e) {
      logError("failed to update draw status", e);
    }
  }

  return (
    <Card className="space-y-3 p-4">
      <div className="flex items-center justify-between">
        <h3 className="font-display text-sm font-bold">Draw requests</h3>
        {canManage &&
          (adding ? null : (
            <Button variant="outline" onClick={() => setAdding(true)}>
              + Request draw
            </Button>
          ))}
      </div>

      {adding && (
        <form onSubmit={add} className="flex flex-wrap gap-2">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Draw title"
            className={fieldClass()}
          />
          <input
            value={amt}
            onChange={(e) => setAmt(e.target.value)}
            inputMode="decimal"
            placeholder="$"
            className="w-28 rounded-lg border border-line bg-surface-2 px-3 py-2 text-sm"
          />
          <Button type="submit">Create</Button>
          <Button
            type="button"
            variant="ghost"
            onClick={() => setAdding(false)}
          >
            Cancel
          </Button>
        </form>
      )}

      {detail.draws.length === 0 ? (
        <p className="text-sm text-ink-3">No draws yet.</p>
      ) : (
        <ul className="divide-y divide-line text-sm">
          {detail.draws.map((d) => (
            <li
              key={d.id}
              className="flex flex-wrap items-center justify-between gap-2 py-2"
            >
              <button
                onClick={() => onSelect(d.id)}
                className={`text-left ${selectedDraw === d.id ? "text-accent-2" : ""}`}
              >
                <span className="font-semibold">
                  #{d.number} {d.title}
                </span>
                <span className="ml-2 text-ink-3">{d.amount_label}</span>
                {d.contractor_name && (
                  <span className="ml-2 text-xs text-ink-3">
                    · {d.contractor_name}
                  </span>
                )}
              </button>
              <span className="flex items-center gap-2">
                <Badge tone={statusTone(d.status)}>{d.status}</Badge>
                {canManage &&
                  (DRAW_NEXT[d.status] ?? []).map((n) => (
                    <button
                      key={n.status}
                      onClick={() => setStatus(d, n.status)}
                      className="text-xs font-semibold text-accent-2 hover:underline"
                    >
                      {n.label}
                    </button>
                  ))}
              </span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function DrawPanel({
  draw,
  canManage,
  reload,
}: {
  draw: RehabDrawDetail;
  canManage: boolean;
  reload: () => void;
}) {
  const [waiverType, setWaiverType] = useState(WAIVER_TYPES[0].key);
  const [busy, setBusy] = useState(false);

  async function generate() {
    setBusy(true);
    try {
      await api.createLienWaiver(draw.id, { waiver_type: waiverType });
      reload();
    } catch (e) {
      logError("failed to generate lien waiver", e);
    } finally {
      setBusy(false);
    }
  }

  async function markReceived(id: string) {
    try {
      await api.updateLienWaiver(id, "received");
      reload();
    } catch (e) {
      logError("failed to update waiver", e);
    }
  }

  return (
    <Card className="space-y-4 p-5">
      <h3 className="font-display text-lg font-bold">
        Draw #{draw.number}: {draw.title}{" "}
        <span className="font-normal text-ink-3">{draw.amount_label}</span>
      </h3>

      <div className="grid gap-4 lg:grid-cols-2">
        <DocumentsCard
          ownerType="rehab_draw"
          ownerId={draw.id}
          title="Progress photos & docs"
        />

        <div className="space-y-3">
          <h4 className="font-display text-sm font-bold">Lien waivers</h4>
          {draw.lien_waivers.length === 0 ? (
            <p className="text-sm text-ink-3">None yet.</p>
          ) : (
            <ul className="space-y-2 text-sm">
              {draw.lien_waivers.map((w) => (
                <li
                  key={w.id}
                  className="flex items-center justify-between gap-2 border-b border-line pb-2"
                >
                  <span className="min-w-0">
                    <span className="block truncate">
                      {w.waiver_type_label}
                    </span>
                    <span className="text-xs text-ink-3">
                      {w.contractor_name} · {w.amount_label}
                    </span>
                  </span>
                  <span className="flex shrink-0 items-center gap-2">
                    <Badge tone={w.status === "received" ? "good" : "warn"}>
                      {w.status}
                    </Badge>
                    {canManage && w.status !== "received" && (
                      <button
                        onClick={() => markReceived(w.id)}
                        className="text-xs font-semibold text-accent-2 hover:underline"
                      >
                        mark received
                      </button>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          )}
          {canManage && (
            <div className="flex gap-2">
              <select
                value={waiverType}
                onChange={(e) => setWaiverType(e.target.value)}
                className={fieldClass()}
              >
                {WAIVER_TYPES.map((t) => (
                  <option key={t.key} value={t.key}>
                    {t.label}
                  </option>
                ))}
              </select>
              <Button variant="outline" onClick={generate} disabled={busy}>
                {busy ? "Generating…" : "Generate"}
              </Button>
            </div>
          )}
        </div>
      </div>
    </Card>
  );
}
