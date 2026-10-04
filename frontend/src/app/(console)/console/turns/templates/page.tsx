"use client";

// The turnover recipe: the steps, who owns each, what each waits on and when
// it's due. Turns already started keep the steps they began with.

import { useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, ListChecks, Plus, Save, Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  OWNER_ROLES,
  turns,
  type Template,
  type TemplateStep,
} from "@/lib/turns";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass, Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

function slug(title: string, taken: Set<string>): string {
  const base =
    title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_|_$/g, "") || "step";
  let k = base;
  for (let i = 2; taken.has(k); i++) k = `${base}_${i}`;
  return k;
}

export default function TemplatesPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const manage = can("maintenance:manage");
  const list = useQuery({
    queryKey: ["turn-templates"],
    queryFn: turns.templates,
    enabled: scoped && can("maintenance:read"),
  });
  const [pickedId, setPickedId] = useState<string | null>(null);
  const current =
    list.data?.find((t) => t.id === pickedId) ?? list.data?.[0] ?? null;

  return (
    <div className="space-y-6">
      <Link
        href="/console/turns"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Turnovers
      </Link>
      <PageHeader
        eyebrow="Turnovers"
        title="Turnover steps"
        description="A step opens once the steps it waits on are done. Required steps must be done before a turn can finish."
        actions={
          (list.data?.length ?? 0) > 1 &&
          current && (
            <select
              aria-label="Template"
              className={fieldClass}
              value={current.id}
              onChange={(e) => setPickedId(e.target.value)}
            >
              {list.data?.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                  {t.is_default ? " (default)" : ""}
                </option>
              ))}
            </select>
          )
        }
      />
      {list.isLoading && <Skeleton className="h-96 rounded-2xl" />}
      {list.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load the steps: {list.error.message}
        </Panel>
      )}
      {list.data && !current && (
        <Panel>
          <EmptyState icon={<ListChecks />} title="No turnover template yet" />
        </Panel>
      )}
      {current && (
        <Editor
          key={`${current.id}:${current.steps.length}:${current.name}`}
          template={current}
          manage={manage}
        />
      )}
    </div>
  );
}

function Editor({ template, manage }: { template: Template; manage: boolean }) {
  const qc = useQueryClient();
  const [name, setName] = useState(template.name);
  const [steps, setSteps] = useState<TemplateStep[]>(template.steps);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);

  const patch = (i: number, p: Partial<TemplateStep>) => {
    setSteps((s) => s.map((x, j) => (j === i ? { ...x, ...p } : x)));
    setDirty(true);
  };
  const remove = (i: number) => {
    const gone = steps[i].key;
    setSteps((s) =>
      s
        .filter((_, j) => j !== i)
        .map((x) => ({
          ...x,
          depends_on: x.depends_on.filter((d) => d !== gone),
        }))
    );
    setDirty(true);
  };
  const add = () => {
    setSteps((s) => [
      ...s,
      {
        key: slug("New step", new Set(s.map((x) => x.key))),
        title: "New step",
        description: null,
        owner_role: "office",
        depends_on: s.length ? [s[s.length - 1].key] : [],
        due_offset_days: 0,
        required: true,
        requires_photo: false,
        ticket_category: null,
        ticket_priority: null,
      },
    ]);
    setDirty(true);
  };

  async function save() {
    setSaving(true);
    try {
      const saved = await turns.saveTemplate(template.id, {
        name,
        is_default: template.is_default,
        steps,
      });
      qc.setQueryData<Template[]>(["turn-templates"], (l) =>
        l?.map((t) => (t.id === saved.id ? saved : t))
      );
      setSteps(saved.steps);
      setDirty(false);
      toast.success("Steps saved");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the steps");
    } finally {
      setSaving(false);
    }
  }

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 border-b border-line p-4">
        <Input
          value={name}
          disabled={!manage}
          onChange={(e) => {
            setName(e.target.value);
            setDirty(true);
          }}
          aria-label="Template name"
          className="max-w-sm font-medium"
        />
        {template.is_default && <Badge tone="accent">default</Badge>}
        <span className="text-xs text-fg-3">
          {steps.length} {steps.length === 1 ? "step" : "steps"}
        </span>
        {manage && (
          <div className="ml-auto flex gap-2">
            <Button variant="secondary" size="sm" onClick={add}>
              <Plus />
              Add step
            </Button>
            <Button size="sm" onClick={save} disabled={!dirty} loading={saving}>
              <Save />
              Save steps
            </Button>
          </div>
        )}
      </div>
      <ol className="divide-y divide-line">
        {steps.map((s, i) => (
          <li
            key={s.key}
            className="grid items-start gap-3 px-4 py-3.5 md:grid-cols-[1.75rem_minmax(0,1fr)_9rem_6.5rem_7rem]"
          >
            <span className="flex size-7 items-center justify-center rounded-full border border-line bg-fill text-xs font-semibold text-fg-2">
              {i + 1}
            </span>
            <div className="space-y-2">
              <input
                className={`${fieldClass} w-full`}
                value={s.title}
                disabled={!manage}
                onChange={(e) => patch(i, { title: e.target.value })}
                aria-label={`Step ${i + 1} title`}
              />
              {i > 0 && (
                <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-fg-3">
                  <span>Waits on</span>
                  {steps.slice(0, i).map((o) => (
                    <label
                      key={o.key}
                      className="inline-flex items-center gap-1.5 text-fg-2"
                    >
                      <input
                        type="checkbox"
                        className="accent-[var(--accent)]"
                        disabled={!manage}
                        checked={s.depends_on.includes(o.key)}
                        onChange={(e) =>
                          patch(i, {
                            depends_on: e.target.checked
                              ? [...s.depends_on, o.key]
                              : s.depends_on.filter((d) => d !== o.key),
                          })
                        }
                      />
                      {o.title}
                    </label>
                  ))}
                </div>
              )}
            </div>
            <label className="text-xs text-fg-3">
              <span className="sr-only">Owner</span>
              <select
                className={`${fieldClass} w-full capitalize`}
                value={s.owner_role}
                disabled={!manage}
                onChange={(e) => patch(i, { owner_role: e.target.value })}
                aria-label={`Step ${i + 1} owner`}
              >
                {OWNER_ROLES.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-2 text-xs text-fg-3">
              Day
              <input
                type="number"
                className={`${fieldClass} w-16`}
                value={s.due_offset_days}
                disabled={!manage}
                onChange={(e) =>
                  patch(i, { due_offset_days: Number(e.target.value) })
                }
                aria-label={`Step ${i + 1} due day`}
              />
            </label>
            <div className="flex flex-col gap-1.5 text-xs text-fg-2">
              <label className="inline-flex items-center gap-1.5">
                <input
                  type="checkbox"
                  className="accent-[var(--accent)]"
                  checked={s.required}
                  disabled={!manage}
                  onChange={(e) => patch(i, { required: e.target.checked })}
                />
                Required
              </label>
              <label className="inline-flex items-center gap-1.5">
                <input
                  type="checkbox"
                  className="accent-[var(--accent)]"
                  checked={s.requires_photo}
                  disabled={!manage}
                  onChange={(e) =>
                    patch(i, { requires_photo: e.target.checked })
                  }
                />
                Needs a photo
              </label>
              {manage && (
                <button
                  type="button"
                  className="inline-flex items-center gap-1 text-left text-fg-3 transition hover:text-bad"
                  onClick={() => remove(i)}
                >
                  <Trash2 className="size-3.5" />
                  Remove
                </button>
              )}
            </div>
          </li>
        ))}
      </ol>
      {steps.length === 0 && (
        <EmptyState
          icon={<ListChecks />}
          title="No steps"
          description={manage ? "Add the first step." : undefined}
          className="py-8"
        />
      )}
    </Panel>
  );
}
