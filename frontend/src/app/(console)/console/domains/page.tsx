"use client";

// Domains: point your own hosts at this workspace. Each host serves one
// audience (the staff app, the owner portal or the resident portal) and wears
// your brand once its DNS checks out. Branded email needs its own records.

import { useState } from "react";
import Link from "next/link";
import {
  Globe,
  Mail,
  Plus,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import type { DomainInfo } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import {
  useCreateDomain,
  useDeleteDomain,
  useDomains,
  useVerifyDomain,
  useVerifyDomainEmail,
} from "@/lib/queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, fieldClass, Input, Label } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import {
  CopyButton,
  errorText,
} from "@/app/(console)/console/integrations/bits";

const AUDIENCES = [
  { value: "admin", label: "Staff app" },
  { value: "owner", label: "Owner portal" },
  { value: "renter", label: "Resident portal" },
];

function audienceLabel(a: string) {
  return AUDIENCES.find((x) => x.value === a)?.label ?? a;
}

const HOST = /^(?=.{3,253}$)([a-z0-9-]+\.)+[a-z]{2,}$/i;

export default function DomainsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("domain:read");
  const manage = can("domain:manage");
  const domains = useDomains({ enabled: scoped && allowed });
  const list = domains.data ?? [];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="Domains"
        description="Give the staff app, owner portal and resident portal their own addresses. Verified hosts show your brand."
        actions={
          can("theme:write") && (
            <Button asChild variant="secondary">
              <Link href="/console/branding">Branding</Link>
            </Button>
          )
        }
      />

      {!allowed && (
        <Panel>
          <EmptyState
            icon={<Globe />}
            title="You can't see domains"
            description="Ask a company owner for the domain:read permission."
          />
        </Panel>
      )}

      {allowed && manage && <AddDomain />}

      {domains.isLoading && <Skeleton className="h-48 rounded-2xl" />}
      {domains.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load domains: {domains.error.message}
        </Panel>
      )}

      {domains.data && list.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Globe />}
            title="No domains yet"
            description="Your workspace subdomain is reserved when it's set up. Add your own host above to use your address instead."
          />
        </Panel>
      )}

      {list.map((d) => (
        <DomainCard key={d.id} d={d} manage={manage} />
      ))}
    </div>
  );
}

function AddDomain() {
  const create = useCreateDomain();
  const [hostname, setHostname] = useState("");
  const [audience, setAudience] = useState("admin");
  const [error, setError] = useState<string | null>(null);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const host = hostname.trim().toLowerCase().replace(/\.$/, "");
    if (!HOST.test(host)) {
      setError("Enter a host like portal.yourcompany.com");
      return;
    }
    setError(null);
    create.mutate(
      { hostname: host, audience },
      {
        onSuccess: () => {
          setHostname("");
          toast.success(`${host} added. Publish its DNS records next.`);
        },
        onError: (err) => toast.error(errorText(err, "Couldn't add it")),
      }
    );
  }

  return (
    <Panel>
      <PanelHeader
        title="Add a domain"
        description="Use a subdomain you control. You'll get the DNS records to publish."
      />
      <form
        onSubmit={submit}
        className="flex flex-col gap-3 p-5 sm:flex-row sm:items-start"
      >
        <div className="min-w-0 flex-1">
          <Field label="Host" error={error}>
            {(p) => (
              <Input
                {...p}
                placeholder="portal.yourcompany.com"
                autoComplete="off"
                spellCheck={false}
                value={hostname}
                onChange={(e) => setHostname(e.target.value)}
              />
            )}
          </Field>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="domain-audience">Serves</Label>
          <select
            id="domain-audience"
            className={cn(fieldClass, "block h-11 w-full sm:w-48")}
            value={audience}
            onChange={(e) => setAudience(e.target.value)}
          >
            {AUDIENCES.map((a) => (
              <option key={a.value} value={a.value}>
                {a.label}
              </option>
            ))}
          </select>
        </div>
        <Button
          type="submit"
          className="sm:mt-[26px]"
          size="lg"
          loading={create.isPending}
        >
          {!create.isPending && <Plus />}
          Add domain
        </Button>
      </form>
    </Panel>
  );
}

function DomainCard({ d, manage }: { d: DomainInfo; manage: boolean }) {
  const verify = useVerifyDomain();
  const remove = useDeleteDomain();
  const verifyEmail = useVerifyDomainEmail();

  return (
    <Panel>
      <div className="flex flex-col gap-3 px-5 pt-5 sm:flex-row sm:items-start">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-[15px] font-medium break-all text-fg">
              {d.hostname}
            </span>
            <Badge tone={d.verified ? "good" : "warn"} dot>
              {d.verified ? "Verified" : "Not verified"}
            </Badge>
            <Badge tone={tlsTone(d.tls_status)}>
              <ShieldCheck className="size-3" />
              TLS {d.tls_status.replace(/_/g, " ")}
            </Badge>
          </div>
          <p className="mt-0.5 text-[13px] text-fg-3">
            {audienceLabel(d.audience)} · {d.kind.replace(/_/g, " ")}
            {d.verified_at &&
              ` · verified ${new Date(d.verified_at).toLocaleDateString()}`}
          </p>
        </div>
        {manage && (
          <div className="flex shrink-0 gap-2">
            {!d.verified && (
              <Button
                size="sm"
                loading={verify.isPending}
                onClick={() =>
                  verify.mutate(d.id, {
                    onSuccess: (r) =>
                      (r as DomainInfo | undefined)?.verified
                        ? toast.success(`${d.hostname} is verified`)
                        : toast.message(
                            "Not found yet. DNS can take a while to spread."
                          ),
                    onError: (e) => toast.error(errorText(e)),
                  })
                }
              >
                {!verify.isPending && <RefreshCw />}
                Check DNS
              </Button>
            )}
            <Button
              size="sm"
              variant="ghost"
              disabled={remove.isPending}
              onClick={() => {
                if (
                  !window.confirm(
                    `Remove ${d.hostname}? People using it lose access through that address.`
                  )
                )
                  return;
                remove.mutate(d.id, {
                  onSuccess: () => toast.success(`${d.hostname} removed`),
                  onError: (e) => toast.error(errorText(e)),
                });
              }}
            >
              <Trash2 />
              Remove
            </Button>
          </div>
        )}
      </div>

      <div className="space-y-4 p-5">
        {d.dns_instructions && !d.verified && (
          <Records
            title="Publish these records, then check DNS"
            records={[
              {
                type: "CNAME",
                name: d.hostname,
                value: d.dns_instructions.cname_target,
              },
              {
                type: "TXT",
                name: d.dns_instructions.txt_name,
                value: d.dns_instructions.txt_value,
              },
            ]}
          />
        )}

        {d.email_dns_records && (
          <div className="rounded-xl border border-line bg-fill/50">
            <div className="flex flex-col gap-2 px-4 pt-4 sm:flex-row sm:items-center">
              <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
                <Mail className="size-4 text-fg-3" />
                <span className="text-[14px] font-medium text-fg">
                  Branded email
                </span>
                <Badge tone={d.email_verified ? "good" : "warn"}>
                  {d.email_verified ? "Verified" : "Not verified"}
                </Badge>
              </div>
              {manage && (
                <Button
                  size="sm"
                  variant="secondary"
                  loading={verifyEmail.isPending}
                  onClick={() =>
                    verifyEmail.mutate(d.id, {
                      onSuccess: (r) =>
                        (r as DomainInfo | undefined)?.email_verified
                          ? toast.success("Email records verified")
                          : toast.message("Some records aren't there yet"),
                      onError: (e) => toast.error(errorText(e)),
                    })
                  }
                >
                  {!verifyEmail.isPending && <RefreshCw />}
                  Check email DNS
                </Button>
              )}
            </div>
            <p className="px-4 pt-1 text-[13px] text-fg-3">
              Publish these TXT records so mail sent from this domain passes
              SPF, DKIM and DMARC. In the sandbox the check always passes.
            </p>
            <ul className="mt-2 divide-y divide-line">
              {d.email_dns_records.map((rec) => {
                const found = d.email_dns_status[rec.key];
                const [tone, word]: [Tone, string] =
                  found === true
                    ? ["good", "Found"]
                    : found === false
                      ? ["bad", "Missing"]
                      : ["neutral", "Not checked"];
                return (
                  <li
                    key={rec.key}
                    className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:items-start"
                  >
                    <div className="flex w-32 shrink-0 items-center gap-2">
                      <span className="text-xs font-medium text-fg uppercase">
                        {rec.key}
                      </span>
                      <Badge tone={tone}>{word}</Badge>
                    </div>
                    <div className="min-w-0 flex-1 space-y-1">
                      <RecordValue label="Name" value={rec.name} />
                      <RecordValue label="Value" value={rec.value} />
                    </div>
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        {d.verified && !d.email_dns_records && (
          <p className="text-[13px] text-fg-3">
            This host is live and shows your brand.
          </p>
        )}
      </div>
    </Panel>
  );
}

function tlsTone(s: string): Tone {
  const v = s.toLowerCase();
  if (v === "issued" || v === "active" || v === "ok") return "good";
  if (v === "pending" || v === "provisioning") return "warn";
  return statusTone(v);
}

function Records({
  title,
  records,
}: {
  title: string;
  records: { type: string; name: string; value: string }[];
}) {
  return (
    <div className="rounded-xl border border-line bg-fill/50">
      <div className="px-4 pt-4 text-[14px] font-medium text-fg">{title}</div>
      <ul className="mt-2 divide-y divide-line">
        {records.map((r) => (
          <li
            key={r.type}
            className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:items-start"
          >
            <span className="w-32 shrink-0 font-mono text-xs font-medium text-fg">
              {r.type}
            </span>
            <div className="min-w-0 flex-1 space-y-1">
              <RecordValue label="Name" value={r.name} />
              <RecordValue label="Value" value={r.value} />
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

function RecordValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-2">
      <span className="w-12 shrink-0 text-[12px] text-fg-3">{label}</span>
      <code className="min-w-0 flex-1 font-mono text-xs break-all text-fg-2">
        {value}
      </code>
      <CopyButton
        value={value}
        size="icon"
        variant="ghost"
        label={`Copy ${label.toLowerCase()}`}
      />
    </div>
  );
}
