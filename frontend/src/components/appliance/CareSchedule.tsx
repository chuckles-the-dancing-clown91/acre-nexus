"use client";

// How to look after an appliance and when: the care instructions, the care
// guide's suggestion for this kind of appliance, and the routine jobs on the
// maintenance schedule.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarClock, Pause, Play, Plus, Sparkles } from "lucide-react";
import { toast } from "sonner";
import { api, type AssetHistory } from "@/lib/api";
import { day } from "@/lib/propertyRecords";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, input, why } from "@/components/property/bits";

function every(days: number) {
  if (days % 365 === 0)
    return days === 365 ? "every year" : `every ${days / 365} years`;
  if (days % 30 === 0 && days >= 60) return `every ${days / 30} months`;
  if (days % 7 === 0 && days <= 28)
    return days === 7 ? "every week" : `every ${days / 7} weeks`;
  return `every ${days} days`;
}

export function CareSchedule({
  a,
  manage,
}: {
  a: AssetHistory;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const [pick, setPick] = useState<string | undefined>(undefined);
  const care = useQuery({
    queryKey: ["asset-care", a.id, pick ?? ""],
    queryFn: () => api.careSuggestion(a.id, pick),
  });
  const [chosen, setChosen] = useState<Record<string, boolean>>({});
  const [text, setText] = useState(a.care_instructions ?? "");
  const [busy, setBusy] = useState(false);
  const [adding, setAdding] = useState(false);
  const [job, setJob] = useState({ title: "", days: "90" });
  const today = new Date().toISOString().slice(0, 10);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["asset", a.id] });
    void qc.invalidateQueries({ queryKey: ["asset-care", a.id] });
  };

  async function saveText() {
    setBusy(true);
    try {
      await api.updateAsset(a.id, { care_instructions: text });
      toast.success("Saved");
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function applyGuide(withText: boolean) {
    const jobs = (care.data?.jobs ?? [])
      .filter((j) => !j.scheduled && (chosen[j.title] ?? true))
      .map((j) => j.title);
    setBusy(true);
    try {
      const r = await api.applyCare(a.id, {
        key: pick ?? care.data?.key ?? undefined,
        instructions: withText,
        life: true,
        jobs,
      });
      if (withText) setText(care.data?.instructions ?? "");
      toast.success(
        r.plans_created
          ? `${r.plans_created} ${r.plans_created === 1 ? "job" : "jobs"} added to the schedule`
          : "Saved"
      );
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggle(id: string, active: boolean) {
    try {
      await api.updateMaintenancePlan(id, { active });
      refresh();
    } catch (e) {
      toast.error(why(e));
    }
  }

  async function addJob() {
    const days = Number(job.days);
    if (!job.title.trim() || !Number.isInteger(days) || days < 1)
      return void toast.error("Name the job and how many days between");
    setBusy(true);
    try {
      const next = new Date(Date.now() + days * 86400000)
        .toISOString()
        .slice(0, 10);
      await api.createMaintenancePlan({
        property_id: a.property_id,
        unit_id: a.unit_id ?? undefined,
        asset_id: a.id,
        title: `${a.name}: ${job.title.trim()}`,
        category: a.kind === "other" ? "general" : a.kind,
        cadence_days: days,
        next_due_date: next,
      });
      setJob({ title: "", days: "90" });
      setAdding(false);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  const g = care.data;
  const open = (g?.jobs ?? []).filter((j) => !j.scheduled);

  return (
    <div className="space-y-6">
      <Panel>
        <PanelHeader
          title="Care instructions"
          description="What anyone who uses or looks after this should know."
        />
        <div className="space-y-3 px-5 pt-3 pb-5">
          <textarea
            className={`${input} min-h-36`}
            value={text}
            readOnly={!manage}
            placeholder="How to clean it, what not to do, who to call."
            onChange={(e) => setText(e.target.value)}
          />
          {manage && (
            <div className="flex justify-end">
              <Button
                size="sm"
                variant="secondary"
                disabled={busy || text === (a.care_instructions ?? "")}
                onClick={() => void saveText()}
              >
                Save instructions
              </Button>
            </div>
          )}
        </div>
      </Panel>

      {manage && (
        <Panel>
          <PanelHeader
            title="Care guide"
            description={
              g?.label
                ? `A starting point for a ${g.label.toLowerCase()}.`
                : "We could not tell what this is. Pick a guide."
            }
            action={<Sparkles className="size-4 text-accent" aria-hidden />}
          />
          <div className="space-y-4 px-5 pt-3 pb-5">
            {care.isLoading && <Skeleton className="h-24" />}
            {g && (
              <>
                <F label="Guide">
                  <select
                    className={input}
                    value={pick ?? g.key ?? ""}
                    onChange={(e) => {
                      setPick(e.target.value || undefined);
                      setChosen({});
                    }}
                  >
                    {!g.key && <option value="">Choose one</option>}
                    {g.library.map((l) => (
                      <option key={l.key} value={l.key}>
                        {l.label}
                      </option>
                    ))}
                  </select>
                </F>
                {g.instructions && (
                  <pre className="rounded-xl border border-line bg-fill/40 p-3 font-sans text-[13px] leading-relaxed whitespace-pre-wrap text-fg-2">
                    {g.instructions}
                  </pre>
                )}
                {g.jobs.length > 0 && (
                  <ul className="divide-y divide-line rounded-xl border border-line">
                    {g.jobs.map((j) => (
                      <li
                        key={j.title}
                        className="flex items-start gap-3 px-3 py-2.5 text-[13px]"
                      >
                        <input
                          type="checkbox"
                          className="mt-1"
                          disabled={j.scheduled}
                          checked={j.scheduled || (chosen[j.title] ?? true)}
                          onChange={(e) =>
                            setChosen({
                              ...chosen,
                              [j.title]: e.target.checked,
                            })
                          }
                          aria-label={j.title}
                        />
                        <div className="min-w-0 flex-1">
                          <div className="font-medium text-fg">{j.title}</div>
                          <div className="text-xs text-fg-3">
                            {j.description}
                          </div>
                        </div>
                        <Badge tone={j.scheduled ? "good" : "neutral"}>
                          {j.scheduled ? "scheduled" : every(j.cadence_days)}
                        </Badge>
                      </li>
                    ))}
                  </ul>
                )}
                <div className="flex flex-wrap justify-end gap-2">
                  {g.instructions && (
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={busy}
                      onClick={() => void applyGuide(true)}
                    >
                      Use these instructions
                    </Button>
                  )}
                  {open.length > 0 && (
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() => void applyGuide(false)}
                    >
                      <CalendarClock />
                      Add jobs to the schedule
                    </Button>
                  )}
                </div>
              </>
            )}
          </div>
        </Panel>
      )}

      <Panel>
        <PanelHeader
          title="Maintenance schedule"
          description="Each job opens a work order when it comes due."
          action={
            manage && (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setAdding(!adding)}
              >
                <Plus />
                Add a job
              </Button>
            )
          }
        />
        <div className="space-y-3 px-5 pt-3 pb-5">
          {adding && (
            <div className="grid gap-3 rounded-xl border border-line bg-fill/40 p-3 sm:grid-cols-[1fr_9rem_auto] sm:items-end">
              <F label="Job">
                <input
                  className={input}
                  value={job.title}
                  onChange={(e) => setJob({ ...job, title: e.target.value })}
                  placeholder="Replace the filter"
                />
              </F>
              <F label="Every (days)">
                <input
                  className={input}
                  inputMode="numeric"
                  value={job.days}
                  onChange={(e) => setJob({ ...job, days: e.target.value })}
                />
              </F>
              <Button disabled={busy} onClick={() => void addJob()}>
                Add
              </Button>
            </div>
          )}
          {a.plans.length === 0 && !adding && (
            <p className="text-[13px] text-fg-3">
              Nothing scheduled yet. Use the care guide above or add a job.
            </p>
          )}
          <ul className="divide-y divide-line">
            {a.plans.map((p) => {
              const overdue = p.active && p.next_due_date < today;
              return (
                <li
                  key={p.id}
                  className="flex flex-wrap items-center gap-3 py-2.5 text-[13px]"
                >
                  <div className="min-w-[200px] flex-1">
                    <div className="font-medium text-fg">{p.title}</div>
                    <div className="text-xs text-fg-3">
                      {every(p.cadence_days)}
                      {p.active
                        ? ` · next ${day(p.next_due_date)}`
                        : " · paused"}
                    </div>
                  </div>
                  {overdue && <Badge tone="bad">overdue</Badge>}
                  {manage && (
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label={p.active ? "Pause" : "Resume"}
                      onClick={() => void toggle(p.id, !p.active)}
                    >
                      {p.active ? <Pause /> : <Play />}
                    </Button>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      </Panel>
    </div>
  );
}
