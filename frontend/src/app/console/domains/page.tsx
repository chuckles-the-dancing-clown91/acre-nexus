"use client";

import { useEffect, useState } from "react";
import { api, type DomainInfo } from "@/lib/api";
import { Badge, Card, statusTone } from "@/components/ui";

const AUDIENCES = ["admin", "owner", "renter"];

export default function DomainsPage() {
  const [domains, setDomains] = useState<DomainInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hostname, setHostname] = useState("");
  const [audience, setAudience] = useState("admin");
  const [busy, setBusy] = useState(false);

  const load = () =>
    api
      .domains()
      .then(setDomains)
      .catch((e) => setError(e.message));

  useEffect(() => {
    load();
  }, []);

  async function addDomain(e: React.FormEvent) {
    e.preventDefault();
    if (!hostname.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await api.createDomain(hostname.trim(), audience);
      setHostname("");
      await load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function verify(id: string) {
    try {
      await api.verifyDomain(id);
      await load();
    } catch (err) {
      setError((err as Error).message);
    }
  }

  async function remove(id: string) {
    try {
      await api.deleteDomain(id);
      await load();
    } catch (err) {
      setError((err as Error).message);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Domains &amp; routing
        </h1>
        <p className="text-ink-3">
          Map hosts to this workspace and an audience — an admin app, an owner
          portal, and a renter portal can each have their own domain.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <Card className="p-5">
        <h2 className="mb-3 font-display text-lg font-bold">
          Add a custom domain
        </h2>
        <form onSubmit={addDomain} className="flex flex-wrap items-end gap-3">
          <label className="flex-1 min-w-[220px] text-sm">
            <span className="mb-1 block text-ink-3">Hostname</span>
            <input
              value={hostname}
              onChange={(e) => setHostname(e.target.value)}
              placeholder="portal.yourfirm.com"
              className="w-full rounded-lg border border-line bg-surface px-3 py-2"
            />
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-3">Audience</span>
            <select
              value={audience}
              onChange={(e) => setAudience(e.target.value)}
              className="rounded-lg border border-line bg-surface px-3 py-2 capitalize"
            >
              {AUDIENCES.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
          </label>
          <button
            type="submit"
            disabled={busy}
            className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
          >
            Add domain
          </button>
        </form>
      </Card>

      <div className="space-y-4">
        {domains?.map((d) => (
          <Card key={d.id} className="p-5">
            <div className="flex flex-wrap items-center gap-3">
              <div className="flex-1">
                <div className="font-mono text-base font-bold">
                  {d.hostname}
                </div>
                <div className="text-sm text-ink-3 capitalize">
                  {d.kind} · {d.audience} portal
                </div>
              </div>
              <Badge tone={d.verified ? "good" : "warn"}>
                {d.verified ? "verified" : "unverified"}
              </Badge>
              <Badge tone={statusTone(d.tls_status)}>TLS {d.tls_status}</Badge>
              {!d.verified && (
                <button
                  onClick={() => verify(d.id)}
                  className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold"
                >
                  Verify DNS
                </button>
              )}
              <button
                onClick={() => remove(d.id)}
                className="rounded-lg border border-line px-3 py-1.5 text-sm text-ink-3"
              >
                Remove
              </button>
            </div>

            {d.dns_instructions && !d.verified && (
              <div className="mt-4 rounded-lg border border-line bg-surface-2 p-4 text-sm">
                <p className="mb-2 text-ink-3">
                  Publish these DNS records, then click <b>Verify DNS</b>:
                </p>
                <pre className="overflow-x-auto whitespace-pre-wrap font-mono text-xs leading-relaxed">
                  {`CNAME  ${d.hostname}  →  ${d.dns_instructions.cname_target}\n`}
                  {`TXT    ${d.dns_instructions.txt_name}  →  ${d.dns_instructions.txt_value}`}
                </pre>
              </div>
            )}
          </Card>
        ))}
        {domains?.length === 0 && (
          <p className="text-ink-3">
            No domains yet — your {`{slug}`}.acrenexus.com subdomain is reserved
            automatically at provisioning.
          </p>
        )}
      </div>
    </div>
  );
}
