"use client";

// "About" and "Facts and features": the property in the team's words, and
// its features in groups. The numbers (beds, baths, lot, zoning…) come from
// the public record; the rest is written here.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Pencil } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { FEATURE_GROUPS, records, splitFeature } from "@/lib/propertyRecords";
import type { PropertyProfile } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { Skeleton } from "@/components/ui/misc";
import { F, FormDialog, input, why } from "./bits";

type Draft = { description: string; groups: Record<string, string> };

function draftOf(
  description: string | null | undefined,
  features: Record<string, string[]> | undefined
): Draft {
  return {
    description: description ?? "",
    groups: Object.fromEntries(
      FEATURE_GROUPS.map((g) => [g.key, (features?.[g.key] ?? []).join("\n")])
    ),
  };
}

const sqft = (n: number | null | undefined) =>
  n == null ? null : `${n.toLocaleString()} sq ft`;

export function Facts({
  property: p,
  propertyId,
  manage,
}: {
  property: PropertyProfile | undefined;
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const intel = useQuery({
    queryKey: ["intel", propertyId],
    queryFn: () => api.propertyIntel(propertyId),
  });
  const [editing, setEditing] = useState(false);
  const [d, setD] = useState<Draft>(draftOf(null, undefined));
  const [busy, setBusy] = useState(false);

  if (intel.isLoading) return <Skeleton className="h-48" />;
  const detail = intel.data?.detail;
  const features = detail?.features ?? {};
  const record: [string, string | null | undefined][] = [
    ["Type", detail?.property_type ?? p?.property_type?.replace(/_/g, " ")],
    ["Year built", p && p.year_built > 0 ? String(p.year_built) : null],
    ["Bedrooms", detail?.beds != null ? String(detail.beds) : null],
    ["Bathrooms", detail?.baths != null ? String(detail.baths) : null],
    ["Living area", sqft(detail?.sqft)],
    ["Lot", sqft(detail?.lot_size_sqft)],
    ["Stories", detail?.stories != null ? String(detail.stories) : null],
    [
      "Parking",
      detail?.parking_spaces != null ? `${detail.parking_spaces} spaces` : null,
    ],
    ["Heating", detail?.heating],
    ["Cooling", detail?.cooling],
    ["Zoning", detail?.zoning],
    ["Parcel (APN)", detail?.apn],
  ];
  const groups = FEATURE_GROUPS.filter((g) => (features[g.key] ?? []).length);

  async function save() {
    setBusy(true);
    try {
      await records.saveStory(propertyId, {
        description: d.description,
        features: Object.fromEntries(
          Object.entries(d.groups).map(([k, v]) => [k, v.split("\n")])
        ),
      });
      setEditing(false);
      void qc.invalidateQueries({ queryKey: ["intel", propertyId] });
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel>
      <PanelHeader
        title="About this property"
        description="In your words, then the facts."
        action={
          manage && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                setD(draftOf(detail?.description, features));
                setEditing(true);
              }}
            >
              <Pencil />
              Edit
            </Button>
          )
        }
      />
      <div className="space-y-5 p-5 pt-4">
        {detail?.description ? (
          <p className="text-[14px] leading-relaxed whitespace-pre-wrap text-fg-2">
            {detail.description}
          </p>
        ) : (
          <p className="text-[13px] text-fg-3">
            {manage
              ? "Describe the property: the neighborhood, what's been updated, what a resident would like about it."
              : "No description yet."}
          </p>
        )}

        <div>
          <div className="eyebrow mb-2">Facts and features</div>
          <dl className="grid gap-x-8 sm:grid-cols-2">
            {record
              .filter(([, v]) => v)
              .map(([k, v]) => (
                <div
                  key={k}
                  className="flex items-baseline justify-between gap-4 border-b border-line py-1.5 text-[13px]"
                >
                  <dt className="text-fg-3">{k}</dt>
                  <dd className="text-right text-fg">{v}</dd>
                </div>
              ))}
          </dl>
        </div>

        {groups.length > 0 && (
          <div className="grid gap-5 sm:grid-cols-2">
            {groups.map((g) => (
              <section key={g.key}>
                <div className="eyebrow mb-2">{g.label}</div>
                <ul className="space-y-1.5 text-[13px]">
                  {(features[g.key] ?? []).map((e) => {
                    const f = splitFeature(e);
                    return (
                      <li key={e} className="flex gap-2 text-fg-2">
                        <Check className="mt-0.5 size-3.5 shrink-0 text-good" />
                        <span>
                          {f.label && (
                            <span className="text-fg-3">{f.label}: </span>
                          )}
                          <span className="text-fg">{f.value}</span>
                        </span>
                      </li>
                    );
                  })}
                </ul>
              </section>
            ))}
          </div>
        )}
      </div>

      <FormDialog
        open={editing}
        onOpenChange={setEditing}
        title="Edit the description and features"
        description="One feature per line. Write “Flooring: Hardwood” to show a label."
        busy={busy}
        onSave={save}
        wide
      >
        <F label="Description" className="sm:col-span-2">
          <textarea
            className={`${input} min-h-[110px]`}
            value={d.description}
            onChange={(e) => setD({ ...d, description: e.target.value })}
          />
        </F>
        {FEATURE_GROUPS.map((g) => (
          <F key={g.key} label={g.label}>
            <textarea
              className={`${input} min-h-[96px]`}
              value={d.groups[g.key]}
              onChange={(e) =>
                setD({ ...d, groups: { ...d.groups, [g.key]: e.target.value } })
              }
            />
          </F>
        ))}
      </FormDialog>
    </Panel>
  );
}
