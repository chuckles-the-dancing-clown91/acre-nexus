"use client";

// Per-user security (issue #63): TOTP MFA enrolment + linking social identities.

import { useEffect, useState } from "react";
import { toast } from "sonner";
import { api, DEFAULT_TENANT } from "@/lib/api";
import type { TotpSetupResult } from "@/lib/api";
import { Badge, Card } from "@/components/ui";
import { Button } from "@/components/ui/button";

const PROVIDERS = [
  { key: "google", label: "Google" },
  { key: "microsoft", label: "Microsoft" },
  { key: "apple", label: "Apple" },
];

export default function SecurityPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Security
        </h1>
        <p className="text-ink-3">
          Two-factor authentication and connected sign-in accounts.
        </p>
      </div>
      <MfaCard />
      <LinkedAccountsCard />
    </div>
  );
}

function MfaCard() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [setup, setSetup] = useState<TotpSetupResult | null>(null);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .mfaStatus()
      .then((s) => setEnabled(s.enabled))
      .catch(() => setEnabled(false));
  }, []);

  async function begin() {
    setError(null);
    setBusy(true);
    try {
      setSetup(await api.mfaSetup());
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't start enrolment");
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    setError(null);
    setBusy(true);
    try {
      await api.mfaConfirm(code.trim());
      setEnabled(true);
      setSetup(null);
      setCode("");
      toast.success("Two-factor authentication enabled");
    } catch {
      setError("That code isn't valid — try again.");
    } finally {
      setBusy(false);
    }
  }

  async function disable() {
    setError(null);
    setBusy(true);
    try {
      await api.mfaDisable(code.trim());
      setEnabled(false);
      setCode("");
      toast.success("Two-factor authentication disabled");
    } catch {
      setError("That code isn't valid.");
    } finally {
      setBusy(false);
    }
  }

  const field =
    "w-40 rounded-lg border border-line bg-surface px-3 py-2 text-center font-mono tracking-widest";

  return (
    <Card className="overflow-hidden">
      <div className="flex items-center gap-3 border-b border-line px-5 py-4">
        <h2 className="flex-1 font-display text-lg font-bold">
          Authenticator app (TOTP)
        </h2>
        {enabled !== null && (
          <Badge tone={enabled ? "good" : "neutral"}>
            {enabled ? "Enabled" : "Off"}
          </Badge>
        )}
      </div>
      <div className="space-y-4 p-5">
        {error && <p className="text-sm text-bad">{error}</p>}

        {enabled === false && !setup && (
          <>
            <p className="text-sm text-ink-2">
              Protect your account with a time-based code from an app like
              Google Authenticator, 1Password, or Authy.
            </p>
            <Button onClick={begin} disabled={busy}>
              {busy ? "Starting…" : "Enable two-factor"}
            </Button>
          </>
        )}

        {setup && (
          <div className="space-y-3">
            <p className="text-sm text-ink-2">
              Add this secret to your authenticator app, then enter the current
              code to confirm.
            </p>
            <div className="rounded-lg border border-line bg-surface-2 p-3 text-sm">
              <div className="mb-1 text-xs font-semibold uppercase text-ink-3">
                Secret
              </div>
              <code className="break-all font-mono">{setup.secret}</code>
              <div className="mt-2 break-all text-xs text-ink-3">
                {setup.otpauth_uri}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <input
                className={field}
                inputMode="numeric"
                placeholder="000000"
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
              <Button
                onClick={confirm}
                disabled={busy || code.trim().length < 6}
              >
                {busy ? "Confirming…" : "Confirm"}
              </Button>
              <Button
                variant="outline"
                onClick={() => {
                  setSetup(null);
                  setCode("");
                }}
              >
                Cancel
              </Button>
            </div>
          </div>
        )}

        {enabled === true && (
          <div className="space-y-3">
            <p className="text-sm text-ink-2">
              Two-factor is on. Enter a current code to turn it off.
            </p>
            <div className="flex items-center gap-2">
              <input
                className={field}
                inputMode="numeric"
                placeholder="000000"
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
              <Button
                variant="outline"
                onClick={disable}
                disabled={busy || code.trim().length < 6}
              >
                {busy ? "Disabling…" : "Disable"}
              </Button>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
}

function LinkedAccountsCard() {
  const [sandbox, setSandbox] = useState<{
    provider: string;
    url: string;
  } | null>(null);
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | null>(null);

  async function link(provider: string) {
    setError(null);
    try {
      const res = await api.oauthStart(provider, {
        intent: "link",
        tenant: DEFAULT_TENANT,
      });
      if (res.sandbox) {
        setEmail(`me@${provider}.example`);
        setSandbox({ provider, url: res.authorize_url });
      } else {
        window.location.assign(res.authorize_url);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't start linking");
    }
  }

  return (
    <Card className="overflow-hidden">
      <div className="border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">Connected accounts</h2>
      </div>
      <div className="space-y-4 p-5">
        {error && <p className="text-sm text-bad">{error}</p>}
        {sandbox ? (
          <div className="space-y-3">
            <p className="text-sm text-ink-2">
              Simulated {sandbox.provider} link — enter the email of the account
              to connect.
            </p>
            <div className="flex items-center gap-2">
              <input
                className="w-64 rounded-lg border border-line bg-surface px-3 py-2 text-sm"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
              <Button
                onClick={() => {
                  window.location.assign(
                    sandbox.url + "&email=" + encodeURIComponent(email)
                  );
                }}
              >
                Continue
              </Button>
              <Button variant="outline" onClick={() => setSandbox(null)}>
                Cancel
              </Button>
            </div>
          </div>
        ) : (
          <>
            <p className="text-sm text-ink-2">
              Link a social account so you can &quot;Log in with&quot; it next
              time.
            </p>
            <div className="flex flex-wrap gap-2">
              {PROVIDERS.map((p) => (
                <Button
                  key={p.key}
                  variant="outline"
                  onClick={() => link(p.key)}
                >
                  Link {p.label}
                </Button>
              ))}
            </div>
          </>
        )}
      </div>
    </Card>
  );
}
