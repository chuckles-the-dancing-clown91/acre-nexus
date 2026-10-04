"use client";

// Give a vendor a login to the vendor portal: every job sent to them in one
// place, instead of one link per job.

import { useState } from "react";
import { HardHat, Send } from "lucide-react";
import { toast } from "sonner";
import { vendorPortal } from "@/lib/vendorLink";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";

export function PortalInvite({
  counterpartyId,
  hasEmail,
  allowed,
}: {
  counterpartyId: string;
  hasEmail: boolean;
  allowed: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<"invited" | "linked" | null>(null);
  async function invite() {
    setBusy(true);
    try {
      const r = await vendorPortal.invite(counterpartyId);
      setDone(r.outcome);
      toast.success(
        r.outcome === "invited"
          ? "Sent them a link to set a password"
          : "Their account now opens the vendor portal"
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't invite them");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Panel>
      <PanelHeader
        title="Vendor portal"
        description={
          done === "invited"
            ? "Invited. They set a password, then see every job sent to them."
            : done === "linked"
              ? "Linked. Their existing account now opens the vendor portal."
              : "A login that shows every job sent to them, to accept, update and invoice in one place. Links in emails keep working."
        }
      />
      <div className="flex flex-wrap items-center gap-2 px-5 pb-5">
        <Button
          size="sm"
          variant="secondary"
          loading={busy}
          disabled={!allowed || !hasEmail}
          onClick={invite}
        >
          {!busy && (done ? <Send /> : <HardHat />)}
          {done ? "Send the link again" : "Invite to the portal"}
        </Button>
        {!hasEmail && (
          <span className="text-[12px] text-fg-4">Add an email first.</span>
        )}
        {hasEmail && !allowed && (
          <span className="text-[12px] text-fg-4">
            Needs the member:manage permission.
          </span>
        )}
      </div>
    </Panel>
  );
}
