"use client";

// Entities: the counterparties across your deals and operations (banks,
// lenders, insurers, contractors and more), each with its own page.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { motion } from "motion/react";
import { Briefcase, Lock, Plus, Search } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import type { CreateCounterpartyInput } from "@/lib/types";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass, Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { F, FormDialog, input } from "@/components/property/bits";
import { humanize, KINDS } from "./kinds";

const EMPTY: CreateCounterpartyInput = {
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
  const scoped = useHasTenantScope();
  const canRead = can("entity:read");
  const canManage = can("entity:manage");
  const router = useRouter();
  const qc = useQueryClient();
  const [kind, setKind] = useState("");
  const [q, setQ] = useState("");
  const [adding, setAdding] = useState(false);
  const [form, setForm] = useState<CreateCounterpartyInput>(EMPTY);
  const [saving, setSaving] = useState(false);
  const list = useQuery({
    queryKey: ["entities", kind],
    queryFn: () => api.entities(kind || undefined),
    enabled: scoped && canRead,
  });
  const rows = useMemo(() => {
    const n = q.trim().toLowerCase();
    return (list.data ?? []).filter(
      (c) =>
        !n ||
        c.name.toLowerCase().includes(n) ||
        (c.contact_name ?? "").toLowerCase().includes(n) ||
        (c.email ?? "").toLowerCase().includes(n)
    );
  }, [list.data, q]);

  const set =
    (k: keyof CreateCounterpartyInput) =>
    (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) =>
      setForm((f) => ({ ...f, [k]: e.target.value }));

  async function create() {
    if (!form.name.trim()) {
      toast.error("Give it a name");
      return;
    }
    setSaving(true);
    const opt = (v?: string) => v?.trim() || undefined;
    try {
      const made = await api.createEntity({
        kind: form.kind,
        name: form.name.trim(),
        contact_name: opt(form.contact_name),
        email: opt(form.email),
        phone: opt(form.phone),
        website: opt(form.website),
        address: opt(form.address),
        notes: opt(form.notes),
      });
      void qc.invalidateQueries({ queryKey: ["entities"] });
      setAdding(false);
      setForm(EMPTY);
      toast.success(`${made.name} added`);
      router.push(`/console/entities/${made.id}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't add it");
    } finally {
      setSaving(false);
    }
  }

  if (!canRead)
    return (
      <Panel className="mx-auto mt-10 max-w-lg">
        <EmptyState
          icon={<Lock />}
          title="No access to entities"
          description="Ask an admin for the entity:read permission."
        />
      </Panel>
    );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Entities and contacts"
        title="Entities"
        description="Banks, lenders, contractors and the other companies you work with."
        actions={
          canManage && (
            <Button onClick={() => setAdding(true)}>
              <Plus />
              Add entity
            </Button>
          )
        }
      />

      <Panel className="overflow-hidden">
        <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center">
          <select
            aria-label="Kind"
            className={fieldClass}
            value={kind}
            onChange={(e) => setKind(e.target.value)}
          >
            <option value="">All kinds</option>
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {humanize(k)}
              </option>
            ))}
          </select>
          <div className="relative sm:ml-auto sm:w-72">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search name, contact or email"
              aria-label="Search entities"
              className="pl-9"
            />
          </div>
        </div>
        {list.isLoading && (
          <div className="space-y-2 p-3">
            {Array.from({ length: 5 }, (_, i) => (
              <Skeleton key={i} className="h-12" />
            ))}
          </div>
        )}
        {list.error && (
          <p className="p-4 text-[13px] text-bad">
            Couldn&apos;t load entities: {list.error.message}
          </p>
        )}
        {list.data && rows.length === 0 && (
          <EmptyState
            icon={<Briefcase />}
            title={q || kind ? "Nothing matches" : "No entities yet"}
            description={
              !q && !kind && canManage
                ? "Add the banks, lenders and contractors you work with."
                : undefined
            }
          />
        )}
        {rows.length > 0 && (
          <div className="hidden grid-cols-[minmax(0,1.6fr)_8rem_minmax(0,1fr)_9rem] gap-4 border-b border-line px-4 py-2.5 md:grid">
            <span className="eyebrow">Name</span>
            <span className="eyebrow">Kind</span>
            <span className="eyebrow">Contact</span>
            <span className="eyebrow text-right">Phone</span>
          </div>
        )}
        <ul className="divide-y divide-line">
          {rows.map((c, i) => (
            <motion.li
              key={c.id}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: Math.min(i, 15) * 0.02, duration: 0.3 }}
            >
              <Link
                href={`/console/entities/${c.id}`}
                className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-4 gap-y-1 px-4 py-3 transition hover:bg-fill-2 md:grid-cols-[minmax(0,1.6fr)_8rem_minmax(0,1fr)_9rem]"
              >
                <span className="truncate text-[14px] font-medium text-fg">
                  {c.name}
                  {c.partner_kind && (
                    <Badge tone="accent" className="ml-2">
                      {c.partner_kind}
                    </Badge>
                  )}
                </span>
                <span className="flex">
                  <Badge tone="info">{humanize(c.kind)}</Badge>
                </span>
                <span className="truncate text-[13px] text-fg-2">
                  {c.contact_name ?? "—"}
                  {c.email ? (
                    <span className="text-fg-3"> · {c.email}</span>
                  ) : null}
                </span>
                <span className="figure text-[13px] text-fg-2 md:text-right">
                  {c.phone ?? "—"}
                </span>
              </Link>
            </motion.li>
          ))}
        </ul>
      </Panel>

      <FormDialog
        open={adding}
        onOpenChange={setAdding}
        title="New entity"
        description="Only the name is needed. Contractors get a tax and insurance card on their page."
        busy={saving}
        onSave={create}
        wide
      >
        <F label="Kind">
          <select
            className={input}
            value={form.kind}
            onChange={(e) => setForm((f) => ({ ...f, kind: e.target.value }))}
          >
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {humanize(k)}
              </option>
            ))}
          </select>
        </F>
        <F label="Name">
          <input
            className={input}
            value={form.name}
            onChange={set("name")}
            placeholder="Acme Bank, N.A."
            required
          />
        </F>
        <F label="Contact name">
          <input
            className={input}
            value={form.contact_name ?? ""}
            onChange={set("contact_name")}
          />
        </F>
        <F label="Email">
          <input
            className={input}
            type="email"
            value={form.email ?? ""}
            onChange={set("email")}
          />
        </F>
        <F label="Phone">
          <input
            className={input}
            value={form.phone ?? ""}
            onChange={set("phone")}
          />
        </F>
        <F label="Website">
          <input
            className={input}
            value={form.website ?? ""}
            onChange={set("website")}
            placeholder="https://"
          />
        </F>
        <F label="Address" className="sm:col-span-2">
          <input
            className={input}
            value={form.address ?? ""}
            onChange={set("address")}
          />
        </F>
        <F label="Notes" className="sm:col-span-2">
          <textarea
            className={input}
            rows={3}
            value={form.notes ?? ""}
            onChange={set("notes")}
          />
        </F>
      </FormDialog>
    </div>
  );
}
