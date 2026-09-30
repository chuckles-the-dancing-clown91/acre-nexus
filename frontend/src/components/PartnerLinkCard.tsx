"use client";

// Link a vendor (counterparty) to their Alpha Power Wash account: base URL +
// an Alpha API key with write:jobs. Shows the callback URL and, once, the
// signing secret the vendor pastes into their Alpha API client.

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { partner, type PartnerLink } from "@/lib/parts";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

export function PartnerLinkCard({
  counterpartyId,
  manage,
}: {
  counterpartyId: string;
  manage: boolean;
}) {
  const [link, setLink] = useState<PartnerLink | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState(false);

  const load = useCallback(() => {
    partner
      .link(counterpartyId)
      .then(setLink)
      .catch(() => setLink(null));
  }, [counterpartyId]);
  useEffect(load, [load]);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  if (!link) return null;

  return (
    <Card className="p-5">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="font-display text-lg font-bold">
          Alpha Power Wash link
        </h2>
        {link.linked ? (
          <Badge tone={link.status === "ok" ? "good" : "bad"}>
            {link.status === "ok" ? "linked" : "needs attention"}
          </Badge>
        ) : (
          <Badge>not linked</Badge>
        )}
      </div>
      <p className="mb-3 text-sm text-ink-3">
        When this vendor runs Alpha, work orders you dispatch to them land on
        their board, and their status, photos and bill come back to the work
        order.
      </p>
      {link.linked && !editing && (
        <dl className="mb-3 space-y-2 text-sm">
          <div className="flex justify-between gap-3 border-b border-line pb-2">
            <dt className="text-ink-3">Their server</dt>
            <dd className="font-mono text-xs">{link.base_url}</dd>
          </div>
          <div className="flex justify-between gap-3 border-b border-line pb-2">
            <dt className="text-ink-3">Linked</dt>
            <dd>{link.linked_at?.slice(0, 10)}</dd>
          </div>
          {link.error && (
            <div className="flex justify-between gap-3 border-b border-line pb-2">
              <dt className="text-ink-3">Last error</dt>
              <dd className="text-bad">{link.error}</dd>
            </div>
          )}
        </dl>
      )}
      {manage && (!link.linked || editing) && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void run(async () => {
              const r = await partner.connect(counterpartyId, {
                base_url: baseUrl,
                api_key: apiKey,
              });
              setSecret(r.callback_secret);
              setApiKey("");
              setEditing(false);
            }, "Linked.");
          }}
          className="mb-3 flex flex-wrap items-center gap-2"
        >
          <input
            className={`${field} w-72`}
            placeholder="https://api.their-alpha.com"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
          />
          <input
            className={`${field} w-72`}
            placeholder="Alpha API key (apw_live_…) with write:jobs"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
          <Button type="submit" disabled={busy || !baseUrl || !apiKey}>
            {link.linked ? "Update link" : "Link vendor"}
          </Button>
          {editing && (
            <Button
              variant="ghost"
              type="button"
              onClick={() => setEditing(false)}
            >
              Cancel
            </Button>
          )}
        </form>
      )}
      <div className="rounded-xl bg-surface-2 p-3 text-sm">
        <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
          For their Alpha API client
        </div>
        <div className="mt-1">
          Callback URL:{" "}
          <span className="font-mono text-xs">{link.callback_url}</span>
        </div>
        {secret ? (
          <div className="mt-1">
            Secret (shown once):{" "}
            <span className="font-mono text-xs">{secret}</span>
          </div>
        ) : (
          <div className="mt-1 text-ink-3">
            Secret:{" "}
            {link.callback_secret_set
              ? "set — rotate to see a new one"
              : "created when you link"}
          </div>
        )}
      </div>
      {manage && link.linked && !editing && (
        <div className="mt-3 flex flex-wrap gap-2">
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => setEditing(true)}
          >
            Change key
          </Button>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const r = await partner.rotateSecret(counterpartyId);
                setSecret(r.callback_secret);
              }, "New secret — give it to the vendor.")
            }
          >
            Rotate secret
          </Button>
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() =>
              void run(() => partner.disconnect(counterpartyId), "Unlinked.")
            }
          >
            Unlink
          </Button>
        </div>
      )}
    </Card>
  );
}
