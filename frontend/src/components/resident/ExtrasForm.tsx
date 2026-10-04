"use client";

// The parts of a resident's profile that a landlord asks about: work, who to
// call, who lives there, pets and where they rented before. Used by the
// resident on their own profile and by a property manager on theirs.

import { useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import type {
  PriorRental,
  ResidentExtras,
  ResidentOccupant,
  ResidentPet,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

export const EMPTY_EXTRAS: ResidentExtras = {
  employer: null,
  job_title: null,
  employer_phone: null,
  emergency_contact_name: null,
  emergency_contact_phone: null,
  emergency_contact_relation: null,
  occupants: [],
  pets: [],
  prior_rentals: [],
};

const PET_KINDS = ["dog", "cat", "bird", "fish", "other"] as const;

function L({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label className={className ?? "block"}>
      <span className="mb-1 block text-xs font-medium text-fg-2">{label}</span>
      {children}
    </label>
  );
}

function RowCard({
  onRemove,
  label,
  children,
}: {
  onRemove: () => void;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="relative rounded-xl border border-line bg-fill/40 p-3">
      <button
        type="button"
        onClick={onRemove}
        aria-label={`Remove ${label}`}
        className="absolute top-2 right-2 rounded-lg p-1.5 text-fg-3 hover:bg-fill-2 hover:text-bad"
      >
        <Trash2 className="size-4" />
      </button>
      <div className="grid gap-3 pr-8 sm:grid-cols-2">{children}</div>
    </div>
  );
}

const text = (v: string | null | undefined) => v ?? "";
const orNull = (v: string) => (v.trim() === "" ? null : v);
const num = (v: string) => (v.trim() === "" ? null : Number(v));

export function ExtrasForm({
  value,
  onSave,
  busy,
  error,
  saveLabel = "Save",
}: {
  value: ResidentExtras;
  onSave: (v: ResidentExtras) => void;
  busy?: boolean;
  error?: string | null;
  saveLabel?: string;
}) {
  const [x, setX] = useState<ResidentExtras>(value);
  const patch = (p: Partial<ResidentExtras>) => setX((c) => ({ ...c, ...p }));
  const setPet = (i: number, p: Partial<ResidentPet>) =>
    patch({ pets: x.pets.map((v, n) => (n === i ? { ...v, ...p } : v)) });
  const setOcc = (i: number, p: Partial<ResidentOccupant>) =>
    patch({
      occupants: x.occupants.map((v, n) => (n === i ? { ...v, ...p } : v)),
    });
  const setRent = (i: number, p: Partial<PriorRental>) =>
    patch({
      prior_rentals: x.prior_rentals.map((v, n) =>
        n === i ? { ...v, ...p } : v
      ),
    });

  return (
    <form
      className="space-y-6"
      onSubmit={(e) => {
        e.preventDefault();
        onSave(x);
      }}
    >
      <Panel>
        <PanelHeader title="Work" description="Where you work." />
        <div className="grid gap-3 px-5 pt-3 pb-5 sm:grid-cols-3">
          <L label="Employer">
            <input
              className={field}
              value={text(x.employer)}
              onChange={(e) => patch({ employer: orNull(e.target.value) })}
            />
          </L>
          <L label="Job title">
            <input
              className={field}
              value={text(x.job_title)}
              onChange={(e) => patch({ job_title: orNull(e.target.value) })}
            />
          </L>
          <L label="Work phone">
            <input
              className={field}
              inputMode="tel"
              value={text(x.employer_phone)}
              onChange={(e) =>
                patch({ employer_phone: orNull(e.target.value) })
              }
            />
          </L>
        </div>
      </Panel>

      <Panel>
        <PanelHeader
          title="Emergency contact"
          description="Someone we can call if we can't reach you."
        />
        <div className="grid gap-3 px-5 pt-3 pb-5 sm:grid-cols-3">
          <L label="Name">
            <input
              className={field}
              value={text(x.emergency_contact_name)}
              onChange={(e) =>
                patch({ emergency_contact_name: orNull(e.target.value) })
              }
            />
          </L>
          <L label="Phone">
            <input
              className={field}
              inputMode="tel"
              value={text(x.emergency_contact_phone)}
              onChange={(e) =>
                patch({ emergency_contact_phone: orNull(e.target.value) })
              }
            />
          </L>
          <L label="Relationship">
            <input
              className={field}
              value={text(x.emergency_contact_relation)}
              onChange={(e) =>
                patch({ emergency_contact_relation: orNull(e.target.value) })
              }
            />
          </L>
        </div>
      </Panel>

      <Panel>
        <PanelHeader
          title="Household"
          description="Everyone who will live with you, besides you."
          action={
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() =>
                patch({ occupants: [...x.occupants, { name: "" }] })
              }
            >
              <Plus className="size-4" /> Add person
            </Button>
          }
        />
        <div className="space-y-3 px-5 pt-3 pb-5">
          {x.occupants.length === 0 && (
            <p className="text-[13px] text-fg-3">Just you.</p>
          )}
          {x.occupants.map((o, i) => (
            <RowCard
              key={i}
              label={o.name || "person"}
              onRemove={() =>
                patch({ occupants: x.occupants.filter((_, n) => n !== i) })
              }
            >
              <L label="Name">
                <input
                  className={field}
                  value={o.name}
                  onChange={(e) => setOcc(i, { name: e.target.value })}
                />
              </L>
              <div className="grid grid-cols-2 gap-3">
                <L label="Relation">
                  <input
                    className={field}
                    value={text(o.relation)}
                    onChange={(e) =>
                      setOcc(i, { relation: orNull(e.target.value) })
                    }
                  />
                </L>
                <L label="Age">
                  <input
                    className={field}
                    inputMode="numeric"
                    value={o.age ?? ""}
                    onChange={(e) => setOcc(i, { age: num(e.target.value) })}
                  />
                </L>
              </div>
            </RowCard>
          ))}
        </div>
      </Panel>

      <Panel>
        <PanelHeader
          title="Pets"
          description="Add each one. A pet can mean a pet fee and an addendum on your lease."
          action={
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() =>
                patch({ pets: [...x.pets, { name: "", kind: "dog" }] })
              }
            >
              <Plus className="size-4" /> Add pet
            </Button>
          }
        />
        <div className="space-y-3 px-5 pt-3 pb-5">
          {x.pets.length === 0 && (
            <p className="text-[13px] text-fg-3">No pets.</p>
          )}
          {x.pets.map((p, i) => (
            <RowCard
              key={i}
              label={p.name || "pet"}
              onRemove={() => patch({ pets: x.pets.filter((_, n) => n !== i) })}
            >
              <div className="grid grid-cols-2 gap-3">
                <L label="Name">
                  <input
                    className={field}
                    value={p.name}
                    onChange={(e) => setPet(i, { name: e.target.value })}
                  />
                </L>
                <L label="Kind">
                  <select
                    className={field}
                    value={p.kind}
                    onChange={(e) => setPet(i, { kind: e.target.value })}
                  >
                    {PET_KINDS.map((k) => (
                      <option key={k} value={k}>
                        {k}
                      </option>
                    ))}
                  </select>
                </L>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <L label="Breed">
                  <input
                    className={field}
                    value={text(p.breed)}
                    onChange={(e) =>
                      setPet(i, { breed: orNull(e.target.value) })
                    }
                  />
                </L>
                <L label="Weight (lb)">
                  <input
                    className={field}
                    inputMode="decimal"
                    value={p.weight_lb ?? ""}
                    onChange={(e) =>
                      setPet(i, { weight_lb: num(e.target.value) })
                    }
                  />
                </L>
              </div>
              <L label="Vaccines good through">
                <input
                  type="date"
                  className={field}
                  value={text(p.vaccinated_through)}
                  onChange={(e) =>
                    setPet(i, { vaccinated_through: orNull(e.target.value) })
                  }
                />
              </L>
              <label className="flex items-center gap-2 self-end pb-2 text-[13px] text-fg">
                <input
                  type="checkbox"
                  checked={p.service_animal ?? false}
                  onChange={(e) =>
                    setPet(i, { service_animal: e.target.checked })
                  }
                />
                Service or support animal
              </label>
            </RowCard>
          ))}
        </div>
      </Panel>

      <Panel>
        <PanelHeader
          title="Rental history"
          description="Where you've rented before, newest first."
          action={
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() =>
                patch({ prior_rentals: [...x.prior_rentals, { address: "" }] })
              }
            >
              <Plus className="size-4" /> Add place
            </Button>
          }
        />
        <div className="space-y-3 px-5 pt-3 pb-5">
          {x.prior_rentals.length === 0 && (
            <p className="text-[13px] text-fg-3">Nothing added yet.</p>
          )}
          {x.prior_rentals.map((r, i) => (
            <RowCard
              key={i}
              label={r.address || "place"}
              onRemove={() =>
                patch({
                  prior_rentals: x.prior_rentals.filter((_, n) => n !== i),
                })
              }
            >
              <L label="Address" className="block sm:col-span-2">
                <input
                  className={field}
                  value={r.address}
                  onChange={(e) => setRent(i, { address: e.target.value })}
                />
              </L>
              <L label="Landlord or company">
                <input
                  className={field}
                  value={text(r.landlord_name)}
                  onChange={(e) =>
                    setRent(i, { landlord_name: orNull(e.target.value) })
                  }
                />
              </L>
              <L label="Landlord phone">
                <input
                  className={field}
                  inputMode="tel"
                  value={text(r.landlord_phone)}
                  onChange={(e) =>
                    setRent(i, { landlord_phone: orNull(e.target.value) })
                  }
                />
              </L>
              <div className="grid grid-cols-2 gap-3">
                <L label="From">
                  <input
                    type="date"
                    className={field}
                    value={text(r.from)}
                    onChange={(e) =>
                      setRent(i, { from: orNull(e.target.value) })
                    }
                  />
                </L>
                <L label="To">
                  <input
                    type="date"
                    className={field}
                    value={text(r.to)}
                    onChange={(e) => setRent(i, { to: orNull(e.target.value) })}
                  />
                </L>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <L label="Rent per month ($)">
                  <input
                    className={field}
                    inputMode="decimal"
                    value={r.rent_cents != null ? r.rent_cents / 100 : ""}
                    onChange={(e) => {
                      const n = num(e.target.value);
                      setRent(i, {
                        rent_cents:
                          n == null || Number.isNaN(n)
                            ? null
                            : Math.round(n * 100),
                      });
                    }}
                  />
                </L>
                <L label="Why you left">
                  <input
                    className={field}
                    value={text(r.reason_for_leaving)}
                    onChange={(e) =>
                      setRent(i, { reason_for_leaving: orNull(e.target.value) })
                    }
                  />
                </L>
              </div>
            </RowCard>
          ))}
        </div>
      </Panel>

      {error && <p className="text-[13px] text-bad">{error}</p>}
      <div className="flex justify-end">
        <Button type="submit" disabled={busy}>
          {busy ? "Saving..." : saveLabel}
        </Button>
      </div>
    </form>
  );
}
