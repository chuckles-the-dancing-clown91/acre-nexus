"use client";

// The turnover recipe: the steps, who owns each, what each waits on, and when
// it is due. Runs already started keep the steps they began with.

import { useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import {
  OWNER_ROLES,
  turns,
  type Template,
  type TemplateStep,
} from "@/lib/turns";
import { useAuth } from "@/lib/auth";
import { Button, Card } from "@/components/ui";

const field =
  "rounded-lg border border-line bg-surface px-2 py-1.5 text-sm text-ink";

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
  const manage = can("maintenance:manage");
  const [list, setList] = useState<Template[]>([]);
  const [current, setCurrent] = useState<Template | null>(null);
  const [name, setName] = useState("");
  const [steps, setSteps] = useState<TemplateStep[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const pick = (t: Template) => {
    setCurrent(t);
    setName(t.name);
    setSteps(t.steps);
  };

  useEffect(() => {
    turns
      .templates()
      .then((l) => {
        setList(l);
        if (l[0]) pick(l[0]);
      })
      .catch((e: Error) => setError(e.message));
  }, []);

  const patch = (i: number, p: Partial<TemplateStep>) =>
    setSteps((s) => s.map((x, j) => (j === i ? { ...x, ...p } : x)));

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
  };

  const add = () =>
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

  const save = async () => {
    if (!current) return;
    setSaving(true);
    try {
      const saved = await turns.saveTemplate(current.id, {
        name,
        is_default: current.is_default,
        steps,
      });
      setList((l) => l.map((t) => (t.id === saved.id ? saved : t)));
      pick(saved);
      toast.success("Steps saved");
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-5">
      <div>
        <Link href="/console/turns" className="text-xs text-ink-3">
          ← Turnovers
        </Link>
        <h1 className="font-display text-2xl font-bold">Turnover steps</h1>
        <p className="text-sm text-ink-3">
          A step opens once the steps it waits on are done. Required steps must
          be done before a turn can finish.
        </p>
      </div>
      {error && <p className="text-sm text-bad">{error}</p>}

      {current && (
        <Card className="space-y-3 p-5">
          <input
            className={`${field} w-full max-w-sm font-semibold`}
            value={name}
            disabled={!manage}
            onChange={(e) => setName(e.target.value)}
            aria-label="Template name"
          />
          {list.length > 1 && (
            <select
              className={field}
              value={current.id}
              onChange={(e) => {
                const t = list.find((x) => x.id === e.target.value);
                if (t) pick(t);
              }}
            >
              {list.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                  {t.is_default ? " (default)" : ""}
                </option>
              ))}
            </select>
          )}
          <div className="space-y-2">
            {steps.map((s, i) => (
              <div
                key={s.key}
                className="grid items-start gap-2 rounded-xl border border-line p-3 md:grid-cols-[2rem_1fr_9rem_6rem_auto]"
              >
                <span className="pt-1.5 text-xs font-bold text-ink-3">
                  {i + 1}
                </span>
                <div className="space-y-2">
                  <input
                    className={`${field} w-full`}
                    value={s.title}
                    disabled={!manage}
                    onChange={(e) => patch(i, { title: e.target.value })}
                    aria-label="Step title"
                  />
                  <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-ink-3">
                    <span>Waits on:</span>
                    {steps
                      .filter((_, j) => j < i)
                      .map((o) => (
                        <label key={o.key} className="flex items-center gap-1">
                          <input
                            type="checkbox"
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
                </div>
                <select
                  className={field}
                  value={s.owner_role}
                  disabled={!manage}
                  onChange={(e) => patch(i, { owner_role: e.target.value })}
                  aria-label="Owner"
                >
                  {OWNER_ROLES.map((r) => (
                    <option key={r} value={r}>
                      {r}
                    </option>
                  ))}
                </select>
                <label className="flex items-center gap-1 text-xs text-ink-3">
                  Day
                  <input
                    type="number"
                    className={`${field} w-16`}
                    value={s.due_offset_days}
                    disabled={!manage}
                    onChange={(e) =>
                      patch(i, { due_offset_days: Number(e.target.value) })
                    }
                  />
                </label>
                <div className="flex flex-col gap-1 text-xs">
                  <label className="flex items-center gap-1">
                    <input
                      type="checkbox"
                      checked={s.required}
                      disabled={!manage}
                      onChange={(e) => patch(i, { required: e.target.checked })}
                    />
                    Required
                  </label>
                  <label className="flex items-center gap-1">
                    <input
                      type="checkbox"
                      checked={s.requires_photo}
                      disabled={!manage}
                      onChange={(e) =>
                        patch(i, { requires_photo: e.target.checked })
                      }
                    />
                    Photo
                  </label>
                  {manage && (
                    <button
                      className="text-left text-bad"
                      onClick={() => remove(i)}
                    >
                      Remove
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
          {manage && (
            <div className="flex gap-2">
              <Button variant="outline" onClick={add}>
                Add step
              </Button>
              <Button onClick={save} disabled={saving}>
                Save steps
              </Button>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}
