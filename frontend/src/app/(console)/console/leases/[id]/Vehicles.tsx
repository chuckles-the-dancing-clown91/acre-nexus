"use client";

// Vehicles on the lease. Adding one can qualify the lease for a parking fee
// when the fee schedule is applied.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Car, Plus, X } from "lucide-react";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, inputClass, useRun } from "../_ui/shared";

export function Vehicles({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const vehicles = useQuery({
    queryKey: ["leases", leaseId, "vehicles"],
    queryFn: () => api.vehicles({ lease_id: leaseId }),
  });
  const { busy, run } = useRun([["leases", leaseId]]);
  const [adding, setAdding] = useState(false);
  const [make, setMake] = useState("");
  const [model, setModel] = useState("");
  const [year, setYear] = useState("");
  const [plate, setPlate] = useState("");

  return (
    <Panel className="h-fit">
      <PanelHeader
        title="Vehicles"
        action={
          manage &&
          !adding && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setAdding(true)}
            >
              <Plus />
              Add
            </Button>
          )
        }
      />
      <div className="space-y-3 p-5 pt-4">
        {adding && (
          <form
            className="grid grid-cols-2 gap-2 rounded-xl border border-line bg-fill/40 p-3"
            onSubmit={async (e) => {
              e.preventDefault();
              if (!make.trim() || !model.trim()) return;
              const ok = await run("add", () =>
                api.createVehicle({
                  lease_id: leaseId,
                  make: make.trim(),
                  model: model.trim(),
                  year: year ? parseInt(year, 10) : undefined,
                  license_plate: plate.trim() || undefined,
                })
              );
              if (ok) {
                setMake("");
                setModel("");
                setYear("");
                setPlate("");
                setAdding(false);
              }
            }}
          >
            <F label="Make">
              <input
                value={make}
                onChange={(e) => setMake(e.target.value)}
                className={inputClass}
                required
              />
            </F>
            <F label="Model">
              <input
                value={model}
                onChange={(e) => setModel(e.target.value)}
                className={inputClass}
                required
              />
            </F>
            <F label="Year">
              <input
                value={year}
                onChange={(e) => setYear(e.target.value)}
                inputMode="numeric"
                className={inputClass}
              />
            </F>
            <F label="Plate">
              <input
                value={plate}
                onChange={(e) => setPlate(e.target.value)}
                className={inputClass}
              />
            </F>
            <div className="col-span-2 flex justify-end gap-2 pt-1">
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => setAdding(false)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                size="sm"
                loading={busy === "add"}
                disabled={!make.trim() || !model.trim()}
              >
                Add vehicle
              </Button>
            </div>
          </form>
        )}

        {vehicles.isLoading && <Skeleton className="h-12" />}
        {vehicles.data?.length === 0 && !adding && (
          <EmptyState
            icon={<Car />}
            title="No vehicles on file"
            className="py-6"
          />
        )}
        <ul className="space-y-2">
          {vehicles.data?.map((v) => (
            <li
              key={v.id}
              className="flex items-center gap-3 rounded-xl border border-line px-3 py-2.5"
            >
              <Car className="size-4 shrink-0 text-fg-3" />
              <span className="min-w-0 flex-1 truncate text-[13px] text-fg">
                {v.label}
              </span>
              {manage && (
                <button
                  type="button"
                  aria-label={`Remove ${v.label}`}
                  disabled={busy === `del-${v.id}`}
                  onClick={() =>
                    run(`del-${v.id}`, () => api.deleteVehicle(v.id))
                  }
                  className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-bad"
                >
                  <X className="size-4" />
                </button>
              )}
            </li>
          ))}
        </ul>
      </div>
    </Panel>
  );
}
