"use client";

// Single sign-on with Alpha: turn it on, copy the secret into Alpha, and
// people signed in on one side open the other without a password.

import { useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { sso, type SsoStatus } from "@/lib/sso";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

export default function SsoSettingsPage() {
  const { can } = useAuth();
  const allowed = can("integrations:manage");
  const [status, setStatus] = useState<SsoStatus | null>(null);
  const [secret, setSecret] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!allowed) return;
    sso
      .status()
      .then(setStatus)
      .catch((e: Error) => setError(e.message));
  }, [allowed]);

  if (!allowed)
    return (
      <Card className="p-6 text-ink-2">
        You need the <span className="font-mono">integrations:manage</span>{" "}
        permission.
      </Card>
    );
  if (error) return <p className="text-sm text-bad">{error}</p>;
  if (!status) return <p className="text-sm text-ink-3">Loading…</p>;

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const row = (label: string, value: string) => (
    <div className="flex justify-between gap-3 border-b border-line py-2 text-sm">
      <dt className="text-ink-3">{label}</dt>
      <dd className="break-all font-mono text-xs">{value}</dd>
    </div>
  );

  return (
    <div className="max-w-2xl space-y-5">
      <div>
        <Link href="/console/settings" className="text-xs text-ink-3">
          ← Settings
        </Link>
        <h1 className="font-display text-2xl font-bold">
          Single sign-on with Alpha
        </h1>
        <p className="text-sm text-ink-3">
          People who already have an account here can open Alpha, and the other
          way round, without typing a password. Nobody is created by a sign-in
          link, and two-step sign-in still applies.
        </p>
      </div>

      <Card className="space-y-4 p-5">
        <div className="flex items-center gap-2">
          <h2 className="font-display text-lg font-bold">Status</h2>
          <Badge tone={status.enabled ? "good" : "neutral"}>
            {status.enabled ? "On" : "Off"}
          </Badge>
        </div>

        {!status.enabled && (
          <Button
            disabled={busy}
            onClick={() =>
              run(async () => {
                const r = await sso.enable();
                setSecret(r.secret);
                setStatus(r);
                toast.success("Single sign-on is on");
              })
            }
          >
            Turn on
          </Button>
        )}

        {secret && (
          <div className="rounded-xl bg-warn-soft p-3 text-sm">
            <div className="font-semibold text-warn">
              Copy this secret now. It is not shown again.
            </div>
            <code className="mt-1 block break-all font-mono text-xs">
              {secret}
            </code>
            <Button
              className="mt-2"
              variant="outline"
              onClick={() => navigator.clipboard.writeText(secret)}
            >
              Copy
            </Button>
          </div>
        )}

        {status.enabled && (
          <>
            <dl>
              {row("Workspace (tenant claim)", status.tenant)}
              {row("Issuer Alpha sends", status.issuer)}
              {row("Audience Alpha sends", status.audience)}
              {row("Send the browser to", `${status.login_url}&token=…`)}
            </dl>
            <p className="text-sm text-ink-3">
              In Alpha, add a Vantedge connection with this workspace, the
              secret above and this site&apos;s address. Then Alpha shows an
              Open Vantedge link, and each linked vendor here gets an Open in
              Alpha button.
            </p>
            <div className="flex gap-2">
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => {
                  if (
                    window.confirm(
                      "Make a new secret? Alpha stops working until you paste it in."
                    )
                  )
                    void run(async () => {
                      const r = await sso.enable(true);
                      setSecret(r.secret);
                      setStatus(r);
                    });
                }}
              >
                New secret
              </Button>
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  if (window.confirm("Turn single sign-on off?"))
                    void run(async () => {
                      setStatus(await sso.disable());
                      setSecret(null);
                    });
                }}
              >
                Turn off
              </Button>
            </div>
          </>
        )}
      </Card>
    </div>
  );
}
