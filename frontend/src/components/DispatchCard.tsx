"use client";

// Send a work order to a vendor's own system (Alpha Power Wash): pick a
// linked vendor, a requested date and a note. Their job id and status show
// here as they come back through the signed callback.

import { useEffect, useState } from "react";
import { toast } from "sonner";
import type { TicketDetail } from "@/lib/types";
import { partner, type LinkedVendor } from "@/lib/parts";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

export function DispatchCard({
  ticket,
  manage,
  reload,
}: {
  ticket: TicketDetail;
  manage: boolean;
  reload: () => void;
}) {
  const [vendors, setVendors] = useState<LinkedVendor[]>([]);
  const [vendorId, setVendorId] = useState("");
  const [when, setWhen] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    partner
      .vendors()
      .then(setVendors)
      .catch(() => setVendors([]));
  }, []);

  const vendor = vendors.find((v) => v.id === ticket.partner_counterparty_id);
  const sent = ticket.partner_job_id || ticket.partner_status;
  if (!sent && (vendors.length === 0 || !manage)) return null;

  async function send(e: React.FormEvent) {
    e.preventDefault();
    if (!vendorId) {
      toast.error("Pick a vendor.");
      return;
    }
    setBusy(true);
    try {
      await partner.dispatch(ticket.id, {
        counterparty_id: vendorId,
        requested_for: when ? `${when}:00` : undefined,
        note: note.trim() || undefined,
      });
      toast.success("Sent to the vendor.");
      reload();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not send");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="p-5">
      <h2 className="mb-2 font-display text-lg font-bold">Vendor (Alpha)</h2>
      {sent ? (
        <div className="flex flex-wrap items-center gap-3 text-sm">
          <span>
            Sent to{" "}
            <span className="font-semibold">{vendor?.name ?? "vendor"}</span>
            {ticket.partner_job_id
              ? ` — their job #${ticket.partner_job_id}`
              : ""}
          </span>
          <Badge
            tone={
              ticket.partner_status === "complete"
                ? "good"
                : ticket.partner_status === "failed"
                  ? "bad"
                  : "info"
            }
          >
            {ticket.partner_status ?? "sent"}
          </Badge>
          {ticket.partner_synced_at && (
            <span className="text-xs text-ink-3">
              updated {ticket.partner_synced_at.slice(0, 16).replace("T", " ")}
            </span>
          )}
        </div>
      ) : (
        <form
          onSubmit={send}
          className="flex flex-wrap items-center gap-2 text-sm"
        >
          <select
            className={field}
            value={vendorId}
            onChange={(e) => setVendorId(e.target.value)}
          >
            <option value="">Send to…</option>
            {vendors.map((v) => (
              <option key={v.id} value={v.id}>
                {v.name}
              </option>
            ))}
          </select>
          <input
            type="datetime-local"
            className={field}
            value={when}
            onChange={(e) => setWhen(e.target.value)}
          />
          <input
            className={`${field} w-64`}
            placeholder="Note for their crew"
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <Button type="submit" disabled={busy || !vendorId}>
            Send work request
          </Button>
          <span className="text-xs text-ink-3">
            The vendor gets the address, access notes and what to do; status,
            photos and the bill come back here.
          </span>
        </form>
      )}
    </Card>
  );
}
