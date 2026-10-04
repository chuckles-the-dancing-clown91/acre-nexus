"use client";

// Foundation mode for an entity: its leases get income certifications and
// housing vouchers, and its management fee can be charged at cost (staff
// time at pay rate plus burden) instead of a percent of rent.

import { useState } from "react";
import Link from "next/link";
import { useQueryClient } from "@tanstack/react-query";
import { HandHeart } from "lucide-react";
import { toast } from "sonner";
import type { LegalEntity } from "@/lib/api";
import { family } from "@/lib/family";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, input, why } from "@/components/property/bits";

export function Foundation({
  entity,
  manage,
}: {
  entity: LegalEntity;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const [on, setOn] = useState(!!entity.foundation);
  const [basis, setBasis] = useState<"percent" | "at_cost">(
    entity.fee_basis ?? "percent"
  );
  const [busy, setBusy] = useState(false);
  async function save() {
    setBusy(true);
    try {
      await family.setFoundation(entity.id, {
        foundation: on,
        fee_basis: basis,
      });
      toast.success("Saved");
      void qc.invalidateQueries({ queryKey: ["legal-entities"] });
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Panel>
      <PanelHeader
        title="Foundation"
        description="Income certifications and housing vouchers on its leases, and how its management fee is charged."
        action={
          entity.foundation ? (
            <Badge tone="info">Foundation</Badge>
          ) : (
            <HandHeart className="size-4 text-fg-3" />
          )
        }
      />
      <div className="space-y-3 p-5 pt-2">
        <label className="flex items-center gap-2 text-[14px] text-fg">
          <input
            type="checkbox"
            className="size-4 accent-[var(--accent)]"
            checked={on}
            disabled={!manage}
            onChange={(e) => setOn(e.target.checked)}
          />
          Run as a foundation
        </label>
        <F label="Management fee">
          <select
            className={input}
            value={basis}
            disabled={!manage}
            onChange={(e) => setBasis(e.target.value as "percent" | "at_cost")}
          >
            <option value="percent">A percent of rent collected</option>
            <option value="at_cost">
              At cost: staff time at pay rate plus burden
            </option>
          </select>
        </F>
        <div className="flex items-center gap-3">
          {manage && (
            <Button
              size="sm"
              variant="secondary"
              loading={busy}
              onClick={() => void save()}
            >
              Save
            </Button>
          )}
          {entity.foundation && (
            <Link
              href="/console/foundation"
              className="text-[13px] text-accent hover:underline"
            >
              Certifications and vouchers
            </Link>
          )}
        </div>
      </div>
    </Panel>
  );
}
