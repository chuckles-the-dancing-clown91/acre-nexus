"use client";

// The property record's suggestions for a property and its unit: review,
// tick what is right, apply. Nothing is overwritten without a click.

import { useEffect, useState } from "react";
import { toast } from "sonner";
import { tours, type Autofill } from "@/lib/tours";
import { Button, Card } from "@/components/ui";

export function AutofillCard({ propertyId }: { propertyId: string }) {
  const [data, setData] = useState<Autofill | null>(null);
  const [pick, setPick] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    tours
      .autofill(propertyId)
      .then((d) => {
        setData(d);
        setPick(new Set(d.proposals.map((p) => p.field)));
      })
      .catch(() => setData(null));
  }, [propertyId]);

  if (!data || data.proposals.length === 0) return null;

  const apply = async () => {
    setBusy(true);
    try {
      const d = await tours.applyAutofill(propertyId, [...pick]);
      setData(d);
      setPick(new Set(d.proposals.map((p) => p.field)));
      toast.success("Applied");
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="mb-4 space-y-3 border-accent p-4">
      <div>
        <h3 className="font-display text-base font-bold">
          The property record suggests changes
        </h3>
        <p className="text-xs text-ink-3">
          Review each one. Only the ticked values are applied.
        </p>
      </div>
      <ul className="divide-y divide-line">
        {data.proposals.map((p) => (
          <li key={p.field} className="flex items-center gap-3 py-2 text-sm">
            <input
              type="checkbox"
              checked={pick.has(p.field)}
              onChange={(e) => {
                const n = new Set(pick);
                if (e.target.checked) n.add(p.field);
                else n.delete(p.field);
                setPick(n);
              }}
              aria-label={p.label}
            />
            <span className="w-32 font-semibold">{p.label}</span>
            <span className="text-ink-3">{p.current ?? "not set"}</span>
            <span aria-hidden="true">to</span>
            <strong>{p.proposed.replace(/_/g, " ")}</strong>
          </li>
        ))}
      </ul>
      <Button onClick={apply} disabled={busy || pick.size === 0}>
        Apply {pick.size} change{pick.size === 1 ? "" : "s"}
      </Button>
    </Card>
  );
}
