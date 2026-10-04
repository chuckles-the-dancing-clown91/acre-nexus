"use client";

// Mark a counterparty as one of the family's own: an entity the family owns
// or a family member. Its bills then wait for a related-party review.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Scale } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { crm } from "@/lib/backoffice";
import { family } from "@/lib/family";
import type { Counterparty } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, input, why } from "@/components/property/bits";

export function RelatedParty({
  cp,
  manage,
}: {
  cp: Counterparty;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const llcs = useQuery({
    queryKey: ["legal-entities"],
    queryFn: api.legalEntities,
  });
  const owners = useQuery({
    queryKey: ["crm-owners"],
    queryFn: crm.owners,
    enabled: manage,
  });
  const [llc, setLlc] = useState(cp.related_llc_id ?? "");
  const [owner, setOwner] = useState(cp.related_owner_id ?? "");
  const [busy, setBusy] = useState(false);
  const related = !!(cp.related_llc_id || cp.related_owner_id);
  const llcName = llcs.data?.find((l) => l.id === cp.related_llc_id)?.name;
  const ownerName = owners.data?.find(
    (o) => o.id === cp.related_owner_id
  )?.name;

  async function save() {
    setBusy(true);
    try {
      await family.setRelated(cp.id, {
        related_llc_id: llc || null,
        related_owner_id: owner || null,
      });
      toast.success(
        llc || owner ? "Marked as a related party" : "No longer a related party"
      );
      void qc.invalidateQueries({ queryKey: ["entities", "one", cp.id] });
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel>
      <PanelHeader
        title="Related party"
        description="Is this one of the family's own companies or people? Its bills then need a market-rate note and a yes from someone outside the deal."
        action={
          related ? (
            <Badge tone="warn">Related</Badge>
          ) : (
            <Scale className="size-4 text-fg-3" />
          )
        }
      />
      <div className="space-y-3 p-5 pt-2">
        {!manage && (
          <p className="text-[13px] text-fg-3">
            {related
              ? `Linked to ${llcName ?? ownerName ?? "the family"}.`
              : "Not a related party."}
          </p>
        )}
        {manage && (
          <>
            <F label="The family's company">
              <select
                className={input}
                value={llc}
                onChange={(e) => setLlc(e.target.value)}
              >
                <option value="">None</option>
                {llcs.data?.map((l) => (
                  <option key={l.id} value={l.id}>
                    {l.name}
                  </option>
                ))}
              </select>
            </F>
            <F label="A family member">
              <select
                className={input}
                value={owner}
                onChange={(e) => setOwner(e.target.value)}
              >
                <option value="">None</option>
                {owners.data?.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.name}
                  </option>
                ))}
              </select>
            </F>
            <Button
              size="sm"
              variant="secondary"
              loading={busy}
              onClick={() => void save()}
            >
              Save
            </Button>
          </>
        )}
      </div>
    </Panel>
  );
}
