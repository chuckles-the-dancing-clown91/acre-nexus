"use client";

// Link a vendor to their Alpha Power Wash account with their server address
// and an Alpha API key that can write jobs. Shows the callback address and,
// once, the signing secret the vendor pastes into their Alpha API client.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, KeyRound, Link2, RefreshCw, Unlink } from "lucide-react";
import { toast } from "sonner";
import { partner } from "@/lib/parts";
import { sso } from "@/lib/sso";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { Fact } from "@/components/property/bits";

export function PartnerLink({
  counterpartyId,
  manage,
}: {
  counterpartyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const key = ["partner-link", counterpartyId];
  const link = useQuery({
    queryKey: key,
    queryFn: () => partner.link(counterpartyId),
    retry: false,
  });
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState(false);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      void qc.invalidateQueries({ queryKey: key });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    } finally {
      setBusy(false);
    }
  }

  const l = link.data;
  if (!l) return null;

  return (
    <Panel>
      <PanelHeader
        title="Alpha Power Wash link"
        description="When this vendor runs Alpha, work orders you send them land on their board, and their status, photos and bill come back to the work order."
        action={
          l.linked ? (
            <Badge tone={l.status === "ok" ? "good" : "bad"}>
              {l.status === "ok" ? "linked" : "needs attention"}
            </Badge>
          ) : (
            <Badge>not linked</Badge>
          )
        }
      />
      <div className="space-y-4 p-5 pt-4">
        {l.linked && !editing && (
          <dl className="divide-y divide-line/60">
            <Fact label="Their server">
              <span className="font-mono text-xs">{l.base_url}</span>
            </Fact>
            <Fact label="Linked">{l.linked_at?.slice(0, 10)}</Fact>
            {l.error && (
              <Fact label="Last error">
                <span className="text-bad">{l.error}</span>
              </Fact>
            )}
          </dl>
        )}

        {manage && (!l.linked || editing) && (
          <form
            className="flex flex-wrap items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                const r = await partner.connect(counterpartyId, {
                  base_url: baseUrl.trim(),
                  api_key: apiKey.trim(),
                });
                setSecret(r.callback_secret);
                setApiKey("");
                setEditing(false);
              }, "Linked");
            }}
          >
            <Input
              className="h-9 min-w-56 flex-1"
              placeholder="https://api.their-alpha.com"
              aria-label="Their Alpha server"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
            />
            <Input
              className="h-9 min-w-56 flex-1"
              placeholder="Alpha API key with write:jobs"
              aria-label="Alpha API key"
              autoComplete="off"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
            />
            <Button
              type="submit"
              size="sm"
              className="h-9"
              disabled={!baseUrl.trim() || !apiKey.trim()}
              loading={busy}
            >
              <Link2 />
              {l.linked ? "Update link" : "Link vendor"}
            </Button>
            {editing && (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                className="h-9"
                onClick={() => setEditing(false)}
              >
                Cancel
              </Button>
            )}
          </form>
        )}

        {l.linked && <OpenInAlpha counterpartyId={counterpartyId} />}

        <div className="rounded-xl border border-line bg-fill/50 p-3 text-[13px]">
          <div className="eyebrow">For their Alpha API client</div>
          <div className="mt-1.5 text-fg-2">
            Callback address:{" "}
            <span className="font-mono text-xs break-all text-fg">
              {l.callback_url}
            </span>
          </div>
          {secret ? (
            <div className="mt-1 text-fg-2">
              Secret, shown once:{" "}
              <span className="font-mono text-xs break-all text-fg">
                {secret}
              </span>
            </div>
          ) : (
            <div className="mt-1 text-fg-3">
              Secret:{" "}
              {l.callback_secret_set
                ? "set. Rotate it to see a new one."
                : "made when you link."}
            </div>
          )}
        </div>

        {manage && l.linked && !editing && (
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => {
                setBaseUrl(l.base_url ?? "");
                setEditing(true);
              }}
            >
              <KeyRound />
              Change key
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const r = await partner.rotateSecret(counterpartyId);
                  setSecret(r.callback_secret);
                }, "New secret made. Give it to the vendor.")
              }
            >
              <RefreshCw />
              Rotate secret
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => {
                if (confirm("Unlink this vendor's Alpha account?"))
                  void run(
                    () => partner.disconnect(counterpartyId),
                    "Unlinked"
                  );
              }}
            >
              <Unlink />
              Unlink
            </Button>
          </div>
        )}
      </div>
    </Panel>
  );
}

/** Opens the vendor's Alpha signed in as you, when single sign-on is on. */
function OpenInAlpha({ counterpartyId }: { counterpartyId: string }) {
  const status = useQuery({
    queryKey: ["sso", "alpha"],
    queryFn: sso.status,
    retry: false,
  });
  const [web, setWeb] = useState("");
  const [busy, setBusy] = useState(false);
  if (!status.data?.enabled) return null;

  async function go() {
    setBusy(true);
    try {
      const r = await sso.launch({
        counterparty_id: counterpartyId,
        web_url: web.trim() || undefined,
      });
      window.open(r.url, "_blank", "noopener,noreferrer");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't open Alpha");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Input
        className="h-9 min-w-56 flex-1"
        placeholder="Their Alpha web address (first time only)"
        aria-label="Their Alpha web address"
        value={web}
        onChange={(e) => setWeb(e.target.value)}
      />
      <Button
        size="sm"
        variant="secondary"
        className="h-9"
        onClick={go}
        loading={busy}
      >
        <ExternalLink />
        Open in Alpha
      </Button>
    </div>
  );
}
