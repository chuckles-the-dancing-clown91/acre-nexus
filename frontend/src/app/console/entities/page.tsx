"use client";

// Entities registry: a searchable list of counterparties (banks, lenders,
// contractors, insurers, etc.). Gated by the "entity:read" permission; users
// with "entity:manage" can add new entities inline.

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { Counterparty, CreateCounterpartyInput } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

/** The counterparty kinds we offer in selects, in display order. */
const KINDS = [
  "bank",
  "lender",
  "insurer",
  "title",
  "contractor",
  "inspector",
  "appraiser",
  "attorney",
  "property_manager",
  "utility",
  "other",
] as const;

/** Turn a snake/lower key into a human label, e.g. `property_manager` → `Property manager`. */
function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

const EMPTY_FORM: CreateCounterpartyInput = {
  kind: "bank",
  name: "",
  contact_name: "",
  email: "",
  phone: "",
  website: "",
  address: "",
  notes: "",
};

export default function EntitiesPage() {
  const { can } = useAuth();
  const [entities, setEntities] = useState<Counterparty[]>([]);
  const [kind, setKind] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<CreateCounterpartyInput>(EMPTY_FORM);
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const canRead = can("entity:read");
  const canManage = can("entity:manage");

  useEffect(() => {
    if (!canRead) return;
    setError(null);
    api
      .entities(kind || undefined)
      .then(setEntities)
      .catch((e: Error) => setError(e.message));
  }, [kind, canRead]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!form.name.trim()) {
      setFormError("Name is required.");
      return;
    }
    setSubmitting(true);
    setFormError(null);
    try {
      const created = await api.createEntity({
        kind: form.kind,
        name: form.name.trim(),
        contact_name: form.contact_name?.trim() || undefined,
        email: form.email?.trim() || undefined,
        phone: form.phone?.trim() || undefined,
        website: form.website?.trim() || undefined,
        address: form.address?.trim() || undefined,
        notes: form.notes?.trim() || undefined,
      });
      setEntities((prev) => [created, ...prev]);
      setForm(EMPTY_FORM);
      setShowForm(false);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "Couldn't add entity.");
    } finally {
      setSubmitting(false);
    }
  }

  if (!canRead) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to the entities registry. Ask an admin to
          grant the <span className="font-mono">entity:read</span> permission.
        </p>
      </Card>
    );
  }

  const inputClass =
    "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Entities
          </h1>
          <p className="text-ink-3">
            Counterparties across your deals — banks, lenders, contractors and
            more.
          </p>
        </div>
        <div className="flex flex-wrap items-end gap-3">
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Filter by kind
            <select
              value={kind}
              onChange={(e) => setKind(e.target.value)}
              className="rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink"
            >
              <option value="">All kinds</option>
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {humanize(k)}
                </option>
              ))}
            </select>
          </label>
          {canManage && (
            <Button
              variant={showForm ? "outline" : "primary"}
              onClick={() => {
                setShowForm((s) => !s);
                setFormError(null);
              }}
            >
              {showForm ? "Cancel" : "Add entity"}
            </Button>
          )}
        </div>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {canManage && showForm && (
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">New entity</h2>
          <form onSubmit={submit} className="space-y-4">
            <div className="grid gap-4 sm:grid-cols-2">
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Kind
                <select
                  value={form.kind}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, kind: e.target.value }))
                  }
                  className="rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink"
                >
                  {KINDS.map((k) => (
                    <option key={k} value={k}>
                      {humanize(k)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Name
                <input
                  value={form.name}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, name: e.target.value }))
                  }
                  required
                  placeholder="Acme Bank, N.A."
                  className={`${inputClass} font-normal`}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Contact name
                <input
                  value={form.contact_name ?? ""}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, contact_name: e.target.value }))
                  }
                  className={`${inputClass} font-normal`}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Email
                <input
                  type="email"
                  value={form.email ?? ""}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, email: e.target.value }))
                  }
                  className={`${inputClass} font-normal`}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Phone
                <input
                  value={form.phone ?? ""}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, phone: e.target.value }))
                  }
                  className={`${inputClass} font-normal`}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                Website
                <input
                  value={form.website ?? ""}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, website: e.target.value }))
                  }
                  placeholder="https://"
                  className={`${inputClass} font-normal`}
                />
              </label>
            </div>
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Address
              <input
                value={form.address ?? ""}
                onChange={(e) =>
                  setForm((f) => ({ ...f, address: e.target.value }))
                }
                className={`${inputClass} font-normal`}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
              Notes
              <textarea
                value={form.notes ?? ""}
                onChange={(e) =>
                  setForm((f) => ({ ...f, notes: e.target.value }))
                }
                rows={3}
                className={`${inputClass} font-normal`}
              />
            </label>
            {formError && <p className="text-bad">{formError}</p>}
            <div className="flex items-center gap-3">
              <Button type="submit" disabled={submitting}>
                {submitting ? "Saving…" : "Create entity"}
              </Button>
            </div>
          </form>
        </Card>
      )}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.6fr_.7fr_1fr_.9fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Name</span>
          <span>Kind</span>
          <span>Contact</span>
          <span className="text-right">Phone</span>
        </div>
        {entities.length === 0 ? (
          <div className="px-5 py-10 text-center text-ink-3">
            No entities yet.
          </div>
        ) : (
          <div className="divide-y divide-line">
            {entities.map((c) => (
              <Link
                key={c.id}
                href={`/console/entities/${c.id}`}
                className="grid grid-cols-[1.6fr_.7fr_1fr_.9fr] items-center gap-4 px-5 py-3.5 hover:bg-surface-2"
              >
                <div className="min-w-0 truncate font-semibold">{c.name}</div>
                <span className="flex">
                  <Badge tone="info">{humanize(c.kind)}</Badge>
                </span>
                <span className="truncate text-sm text-ink-2">
                  {c.contact_name ?? "—"}
                </span>
                <span className="text-right font-mono text-sm text-ink-2">
                  {c.phone ?? "—"}
                </span>
              </Link>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
