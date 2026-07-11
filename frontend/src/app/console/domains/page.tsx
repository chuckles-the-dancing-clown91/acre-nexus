"use client";

import { useForm } from "react-hook-form";
import {
  useDomains,
  useCreateDomain,
  useVerifyDomain,
  useDeleteDomain,
  useVerifyDomainEmail,
} from "@/lib/queries";
import { Badge, Card, statusTone } from "@/components/ui";

const AUDIENCES = ["admin", "owner", "renter"];

export default function DomainsPage() {
  const domainsQuery = useDomains();
  const domains = domainsQuery.data;
  const create = useCreateDomain();
  const verifyMut = useVerifyDomain();
  const removeMut = useDeleteDomain();
  const verifyEmailMut = useVerifyDomainEmail();

  const error =
    domainsQuery.error ||
    create.error ||
    verifyMut.error ||
    removeMut.error ||
    verifyEmailMut.error;

  const { register, handleSubmit, reset } = useForm<{
    hostname: string;
    audience: string;
  }>({ defaultValues: { hostname: "", audience: "admin" } });

  const onAdd = handleSubmit(({ hostname, audience }) => {
    if (!hostname.trim()) return;
    create.mutate(
      { hostname: hostname.trim(), audience },
      { onSuccess: () => reset() }
    );
  });

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

      {error && <p className="text-bad">{error.message}</p>}

      <Card className="p-5">
        <h2 className="mb-3 font-display text-lg font-bold">
          Add a custom domain
        </h2>
        <form onSubmit={onAdd} className="flex flex-wrap items-end gap-3">
          <label className="flex-1 min-w-[220px] text-sm">
            <span className="mb-1 block text-ink-3">Hostname</span>
            <input
              {...register("hostname")}
              placeholder="portal.yourfirm.com"
              className="w-full rounded-lg border border-line bg-surface px-3 py-2"
            />
          </label>
          <label className="text-sm">
            <span className="mb-1 block text-ink-3">Audience</span>
            <select
              {...register("audience")}
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
            disabled={create.isPending}
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
                  onClick={() => verifyMut.mutate(d.id)}
                  className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold"
                >
                  Verify DNS
                </button>
              )}
              <button
                onClick={() => removeMut.mutate(d.id)}
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

            {d.email_dns_records && (
              <div className="mt-4 rounded-lg border border-line bg-surface-2 p-4 text-sm">
                <div className="mb-2 flex flex-wrap items-center gap-3">
                  <h3 className="font-semibold">Branded email</h3>
                  <Badge tone={d.email_verified ? "good" : "warn"}>
                    {d.email_verified ? "email verified" : "email unverified"}
                  </Badge>
                  <button
                    onClick={() => verifyEmailMut.mutate(d.id)}
                    disabled={
                      verifyEmailMut.isPending &&
                      verifyEmailMut.variables === d.id
                    }
                    className="ml-auto rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
                  >
                    {verifyEmailMut.isPending &&
                    verifyEmailMut.variables === d.id
                      ? "Verifying…"
                      : "Verify email DNS"}
                  </button>
                </div>
                <p className="mb-3 text-ink-3">
                  Publish these records so mail from this domain passes
                  SPF/DKIM/DMARC. Simulated verification passes in sandbox.
                </p>
                <div className="space-y-2">
                  {d.email_dns_records.map((rec) => {
                    const found = d.email_dns_status[rec.key];
                    return (
                      <div
                        key={rec.key}
                        className="flex flex-wrap items-start gap-x-3 gap-y-1"
                      >
                        <span className="w-16 shrink-0 pt-0.5 text-xs font-bold uppercase tracking-wide">
                          {rec.key}
                        </span>
                        <Badge
                          tone={
                            found === true
                              ? "good"
                              : found === false
                                ? "bad"
                                : "neutral"
                          }
                        >
                          {found === true
                            ? "found"
                            : found === false
                              ? "missing"
                              : "unchecked"}
                        </Badge>
                        <div className="min-w-0 flex-1">
                          <div className="break-all font-mono text-xs">
                            {rec.name}
                          </div>
                          <div className="break-all font-mono text-xs text-ink-3">
                            {rec.value}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
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
