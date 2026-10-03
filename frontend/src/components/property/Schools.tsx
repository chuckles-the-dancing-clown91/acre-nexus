"use client";

// The schools a property is zoned for, each with its own profile. Data-source
// rows are estimates until someone confirms the zone with the district; an
// edit makes the row the team's, and a data refresh leaves it alone.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  BadgeCheck,
  ExternalLink,
  GraduationCap,
  MapPin,
  Pencil,
  Phone,
  Plus,
} from "lucide-react";
import { toast } from "sonner";
import {
  day,
  label,
  records,
  SCHOOL_LEVELS,
  type SchoolInput,
  type SchoolRecord,
} from "@/lib/propertyRecords";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { F, FormDialog, input, why } from "./bits";

type Draft = {
  name: string;
  level: string;
  district: string;
  grades: string;
  rating: string;
  distance: string;
  assigned: boolean;
  zone_name: string;
  zone_verified_on: string;
  address: string;
  phone: string;
  website: string;
  enrollment: string;
  notes: string;
};

function draftOf(s?: SchoolRecord): Draft {
  return {
    name: s?.name ?? "",
    level: s?.level ?? "elementary",
    district: s?.district ?? "",
    grades: s?.grades ?? "",
    rating: s?.rating != null ? String(s.rating) : "",
    distance: s?.distance_mi != null ? String(s.distance_mi) : "",
    assigned: s?.assigned ?? true,
    zone_name: s?.zone_name ?? "",
    zone_verified_on: s?.zone_verified_on ?? "",
    address: s?.address ?? "",
    phone: s?.phone ?? "",
    website: s?.website ?? "",
    enrollment: s?.enrollment != null ? String(s.enrollment) : "",
    notes: s?.notes ?? "",
  };
}

const num = (v: string) => (v.trim() === "" ? null : Number(v));

function toInput(d: Draft): SchoolInput {
  return {
    ...d,
    rating: num(d.rating),
    distance_mi: num(d.distance),
    enrollment: num(d.enrollment),
  };
}

const today = () => {
  const n = new Date();
  return `${n.getFullYear()}-${String(n.getMonth() + 1).padStart(2, "0")}-${String(n.getDate()).padStart(2, "0")}`;
};

export function Schools({
  propertyId,
  manage,
}: {
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const schools = useQuery({
    queryKey: ["schools", propertyId],
    queryFn: () => records.schools(propertyId),
  });
  const [editing, setEditing] = useState<SchoolRecord | "new" | null>(null);
  const [d, setD] = useState<Draft>(draftOf());
  const [busy, setBusy] = useState(false);
  const set = (k: keyof Draft) => (e: { target: { value: string } }) =>
    setD({ ...d, [k]: e.target.value });
  const open = (s: SchoolRecord | "new") => {
    setD(draftOf(s === "new" ? undefined : s));
    setEditing(s);
  };
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["schools", propertyId] });
    void qc.invalidateQueries({ queryKey: ["attention", propertyId] });
  };

  async function save() {
    setBusy(true);
    try {
      if (editing === "new") await records.createSchool(propertyId, toInput(d));
      else if (editing)
        await records.updateSchool(propertyId, editing.id, toInput(d));
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function confirmZone(s: SchoolRecord) {
    try {
      await records.updateSchool(propertyId, s.id, {
        ...toInput(draftOf(s)),
        zone_verified_on: today(),
      });
      toast.success(`${s.name} zone confirmed`);
      refresh();
    } catch (e) {
      toast.error(why(e));
    }
  }

  async function remove() {
    if (!editing || editing === "new" || !confirm("Remove this school?"))
      return;
    setBusy(true);
    try {
      await records.deleteSchool(propertyId, editing.id);
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  const rows = schools.data ?? [];
  const zoned = rows.filter((s) => s.assigned);
  const nearby = rows.filter((s) => !s.assigned);

  return (
    <Panel>
      <PanelHeader
        title="Schools"
        description="The schools this address is zoned for, and others nearby."
        action={
          manage && (
            <Button size="sm" variant="secondary" onClick={() => open("new")}>
              <Plus />
              School
            </Button>
          )
        }
      />
      <div className="space-y-4 p-5 pt-4">
        {schools.isLoading && <Skeleton className="h-28" />}
        {schools.isSuccess && rows.length === 0 && (
          <EmptyState
            icon={<GraduationCap />}
            title="No schools on file"
            description="Look the address up in the district's boundary tool and add the zoned schools."
            className="py-6"
          />
        )}
        {zoned.length > 0 && (
          <div className="grid gap-3 lg:grid-cols-3">
            {zoned.map((s) => (
              <SchoolCard
                key={s.id}
                s={s}
                manage={manage}
                onEdit={() => open(s)}
                onConfirm={() => confirmZone(s)}
              />
            ))}
          </div>
        )}
        {nearby.length > 0 && (
          <div>
            <div className="eyebrow mb-2">Nearby, not zoned</div>
            <div className="grid gap-3 lg:grid-cols-3">
              {nearby.map((s) => (
                <SchoolCard
                  key={s.id}
                  s={s}
                  manage={manage}
                  onEdit={() => open(s)}
                />
              ))}
            </div>
          </div>
        )}
      </div>

      <FormDialog
        open={!!editing}
        onOpenChange={(o) => !o && setEditing(null)}
        title={editing === "new" ? "Add a school" : "Edit school"}
        description="Saving makes this your team's record; a data refresh won't overwrite it."
        busy={busy}
        onSave={save}
        onDelete={editing && editing !== "new" ? remove : undefined}
        wide
      >
        <F label="School" className="sm:col-span-2">
          <input
            className={input}
            required
            value={d.name}
            onChange={set("name")}
          />
        </F>
        <F label="Level">
          <select className={input} value={d.level} onChange={set("level")}>
            {SCHOOL_LEVELS.map((l) => (
              <option key={l} value={l}>
                {label(l)}
              </option>
            ))}
          </select>
        </F>
        <F label="Grades">
          <input
            className={input}
            placeholder="K-5"
            value={d.grades}
            onChange={set("grades")}
          />
        </F>
        <F label="District">
          <input
            className={input}
            value={d.district}
            onChange={set("district")}
          />
        </F>
        <F label="Rating (1-10)">
          <input
            className={input}
            inputMode="numeric"
            value={d.rating}
            onChange={set("rating")}
          />
        </F>
        <label className="flex items-center gap-2 text-[13px] text-fg sm:col-span-2">
          <input
            type="checkbox"
            checked={d.assigned}
            onChange={(e) => setD({ ...d, assigned: e.target.checked })}
          />
          This address is in the school&apos;s attendance zone
        </label>
        {d.assigned && (
          <>
            <F label="Zone name">
              <input
                className={input}
                value={d.zone_name}
                onChange={set("zone_name")}
              />
            </F>
            <F label="Zone confirmed with the district on">
              <input
                type="date"
                className={input}
                value={d.zone_verified_on}
                onChange={set("zone_verified_on")}
              />
            </F>
          </>
        )}
        <F label="Distance (miles)">
          <input
            className={input}
            inputMode="decimal"
            value={d.distance}
            onChange={set("distance")}
          />
        </F>
        <F label="Students">
          <input
            className={input}
            inputMode="numeric"
            value={d.enrollment}
            onChange={set("enrollment")}
          />
        </F>
        <F label="Address" className="sm:col-span-2">
          <input
            className={input}
            value={d.address}
            onChange={set("address")}
          />
        </F>
        <F label="Phone">
          <input
            className={input}
            type="tel"
            value={d.phone}
            onChange={set("phone")}
          />
        </F>
        <F label="Website">
          <input
            className={input}
            inputMode="url"
            placeholder="https://"
            value={d.website}
            onChange={set("website")}
          />
        </F>
        <F label="Notes" className="sm:col-span-2">
          <textarea
            className={`${input} min-h-[64px]`}
            placeholder="Bus stop, enrollment window, magnet programs…"
            value={d.notes}
            onChange={set("notes")}
          />
        </F>
      </FormDialog>
    </Panel>
  );
}

function SchoolCard({
  s,
  manage,
  onEdit,
  onConfirm,
}: {
  s: SchoolRecord;
  manage: boolean;
  onEdit: () => void;
  onConfirm?: () => void;
}) {
  return (
    <div className="flex flex-col rounded-xl border border-line p-4">
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <div className="eyebrow">
            {label(s.level)}
            {s.grades && ` · ${s.grades}`}
          </div>
          <div className="mt-0.5 text-[14px] font-semibold text-fg">
            {s.name}
          </div>
          {s.district && <div className="text-xs text-fg-3">{s.district}</div>}
        </div>
        {s.rating != null && (
          <span
            className={cn(
              "figure flex size-9 shrink-0 items-center justify-center rounded-full text-[14px] font-semibold",
              s.rating >= 8
                ? "bg-good/15 text-good"
                : s.rating >= 5
                  ? "bg-warn/15 text-warn"
                  : "bg-bad/15 text-bad"
            )}
            title="Rating out of 10"
          >
            {s.rating}
          </span>
        )}
      </div>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {s.assigned &&
          (s.zone_verified_on ? (
            <Badge tone="good">
              <BadgeCheck className="size-3" />
              Zoned · confirmed {day(s.zone_verified_on)}
            </Badge>
          ) : (
            <Badge tone="warn">Zoned · not confirmed</Badge>
          ))}
        {s.simulated && <Badge>estimate</Badge>}
      </div>
      <div className="mt-2 space-y-1 text-xs text-fg-3">
        {s.distance_mi != null && (
          <div className="flex items-center gap-1.5">
            <MapPin className="size-3" />
            {s.distance_mi} mi
            {s.address && ` · ${s.address}`}
          </div>
        )}
        {s.enrollment != null && <div>{s.enrollment} students</div>}
        {s.phone && (
          <a
            href={`tel:${s.phone}`}
            className="flex items-center gap-1.5 hover:text-fg"
          >
            <Phone className="size-3" />
            {s.phone}
          </a>
        )}
        {s.website && (
          <a
            href={s.website}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1.5 text-accent hover:underline"
          >
            <ExternalLink className="size-3" />
            Website
          </a>
        )}
        {s.notes && <div className="text-fg-2">{s.notes}</div>}
      </div>
      {manage && (
        <div className="mt-auto flex gap-1.5 pt-3">
          {onConfirm && s.assigned && !s.zone_verified_on && (
            <Button size="sm" variant="secondary" onClick={onConfirm}>
              <BadgeCheck />
              Zone confirmed
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={onEdit}>
            <Pencil />
            Edit
          </Button>
        </div>
      )}
    </div>
  );
}
