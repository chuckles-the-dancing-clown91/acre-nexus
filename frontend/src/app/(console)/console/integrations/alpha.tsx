"use client";

// Alpha: single sign-on between the two products, and vendors who run Alpha
// linked so work orders land on their board and status, photos and bills
// come back.

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ChevronRight,
  ExternalLink,
  HardHat,
  KeyRound,
  Link2,
  Link2Off,
  Mail,
  Power,
  RefreshCw,
  Search,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { partner, type LinkResult } from "@/lib/parts";
import { desk } from "@/lib/servicedesk";
import { sso, type SsoStatus } from "@/lib/sso";
import type { Counterparty } from "@/lib/types";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/input";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { errorText, Row, SecretOnce } from "./bits";

const SSO_KEY = ["sso-alpha"];

// ---- Single sign-on ----

export function SsoTab() {
  const qc = useQueryClient();
  const status = useQuery({ queryKey: SSO_KEY, queryFn: sso.status });
  const [secret, setSecret] = useState<string | null>(null);
  const put = (s: SsoStatus) => qc.setQueryData(SSO_KEY, s);

  const enable = useMutation({
    mutationFn: (rotate: boolean) => sso.enable(rotate),
    onSuccess: ({ secret: s, ...rest }, rotate) => {
      setSecret(s);
      put(rest);
      toast.success(rotate ? "New secret made" : "Single sign-on is on");
    },
    onError: (e) => toast.error(errorText(e)),
  });
  const disable = useMutation({
    mutationFn: sso.disable,
    onSuccess: (s) => {
      put(s);
      setSecret(null);
      toast.success("Single sign-on is off");
    },
    onError: (e) => toast.error(errorText(e)),
  });
  const busy = enable.isPending || disable.isPending;

  if (status.isLoading) return <Skeleton className="h-64 rounded-2xl" />;
  if (status.error)
    return (
      <Panel className="border-bad/30 p-4 text-[13px] text-bad">
        Couldn&apos;t load single sign-on: {status.error.message}
      </Panel>
    );
  const s = status.data;
  if (!s) return null;

  return (
    <div className="space-y-6">
      <Panel>
        <PanelHeader
          title="Single sign-on with Alpha"
          description="People who already have an account on both sides move between them without a password. A sign-in link never creates anyone, and two-step sign-in still applies."
          action={
            <Badge tone={s.enabled ? "good" : "neutral"} dot>
              {s.enabled ? "On" : "Off"}
            </Badge>
          }
        />
        <div className="space-y-4 p-5">
          {secret && (
            <SecretOnce
              title="Copy this secret into Alpha now. It is not shown again."
              secret={secret}
              onDismiss={() => setSecret(null)}
            />
          )}
          {!s.enabled ? (
            <Button loading={busy} onClick={() => enable.mutate(false)}>
              {!busy && <Power />}
              Turn on
            </Button>
          ) : (
            <>
              <dl>
                <Row
                  label="Workspace (tenant claim)"
                  value={s.tenant}
                  copy={s.tenant}
                />
                <Row
                  label="Issuer Alpha sends"
                  value={s.issuer}
                  copy={s.issuer}
                />
                <Row
                  label="Audience Alpha sends"
                  value={s.audience}
                  copy={s.audience}
                />
                <Row
                  label="Send the browser to"
                  value={`${s.login_url}&token=…`}
                  copy={s.login_url}
                />
              </dl>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="secondary"
                  disabled={busy}
                  onClick={() => {
                    if (
                      window.confirm(
                        "Make a new secret? Alpha stops working until you paste it in."
                      )
                    )
                      enable.mutate(true);
                  }}
                >
                  <RefreshCw />
                  New secret
                </Button>
                <Button
                  variant="ghost"
                  disabled={busy}
                  onClick={() => {
                    if (window.confirm("Turn single sign-on off?"))
                      disable.mutate();
                  }}
                >
                  <Power />
                  Turn off
                </Button>
              </div>
            </>
          )}
        </div>
      </Panel>
      <Panel className="p-5 text-[13px] text-fg-2">
        <div className="text-[15px] font-semibold text-fg">Setting it up</div>
        <ol className="mt-2 list-decimal space-y-1 pl-5">
          <li>Turn it on here and copy the secret. It shows once.</li>
          <li>
            In Alpha, add a Vantedge connection with this workspace, the secret
            and this site&apos;s address.
          </li>
          <li>
            Alpha then shows an Open Vantedge link, and each linked vendor here
            gets an Open in Alpha button.
          </li>
          <li>
            A new secret stops Alpha working until it&apos;s pasted in there.
          </li>
        </ol>
      </Panel>
    </div>
  );
}

// ---- Vendors on Alpha ----

export function PartnersTab() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const vendors = useQuery({
    queryKey: ["entities", "all"],
    queryFn: () => api.entities(),
    enabled: scoped && can("entity:read"),
  });
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [everyone, setEveryone] = useState(false);
  // Contractors first, linked ones on top; other entities on request.
  const list = (vendors.data ?? [])
    .filter((v) => everyone || v.kind === "contractor" || v.partner_kind)
    .filter((v) => v.name.toLowerCase().includes(q.trim().toLowerCase()))
    .sort(
      (a, b) =>
        Number(!!b.partner_kind) - Number(!!a.partner_kind) ||
        a.name.localeCompare(b.name)
    );
  const linked = (vendors.data ?? []).filter((v) => v.partner_kind).length;

  return (
    <Panel>
      <PanelHeader
        title="Vendors on Alpha"
        description="When a vendor runs Alpha, work orders you send them land on their board, and status, photos and the bill come back."
        action={
          vendors.data && (
            <Badge tone={linked ? "good" : "neutral"}>{linked} linked</Badge>
          )
        }
      />
      {(vendors.data?.length ?? 0) > 0 && (
        <div className="flex flex-col gap-2 px-5 pt-4 sm:flex-row sm:items-center">
          <div className="relative max-w-sm flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              aria-label="Filter vendors"
              className="pl-9"
              placeholder="Filter by name"
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
          </div>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setEveryone((v) => !v)}
          >
            {everyone ? "Contractors only" : "Show every entity"}
          </Button>
        </div>
      )}
      <div className="pt-3">
        {vendors.isLoading && (
          <div className="px-5 pb-5">
            <Skeleton className="h-32" />
          </div>
        )}
        {vendors.error && (
          <p className="px-5 pb-5 text-[13px] text-bad">
            Couldn&apos;t load vendors: {vendors.error.message}
          </p>
        )}
        {vendors.data && vendors.data.length === 0 && (
          <EmptyState
            icon={<HardHat />}
            title="No vendors yet"
            description="Add vendors in Entities, then link the ones who run Alpha here."
          />
        )}
        {vendors.data && vendors.data.length > 0 && list.length === 0 && (
          <p className="px-5 py-6 text-center text-[13px] text-fg-3">
            No matches. Try showing every entity.
          </p>
        )}
        <ul className="divide-y divide-line">
          {list.map((v) => (
            <li key={v.id}>
              <button
                type="button"
                aria-expanded={open === v.id}
                onClick={() => setOpen(open === v.id ? null : v.id)}
                className="flex w-full items-center gap-3 px-5 py-3 text-left transition hover:bg-fill"
              >
                <span className="flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-fill text-fg-2">
                  <HardHat className="size-4" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[14px] font-medium text-fg">
                    {v.name}
                  </span>
                  <span className="block truncate text-xs text-fg-3">
                    {[v.kind.replace(/_/g, " "), v.email]
                      .filter(Boolean)
                      .join(" · ")}
                  </span>
                </span>
                <VendorBadge v={v} />
                <ChevronRight
                  className={cn(
                    "size-4 shrink-0 text-fg-4 transition",
                    open === v.id && "rotate-90"
                  )}
                />
              </button>
              {open === v.id && <PartnerLinkPanel vendor={v} />}
            </li>
          ))}
        </ul>
      </div>
    </Panel>
  );
}

function VendorBadge({ v }: { v: Counterparty }) {
  if (!v.partner_kind) return <Badge tone="neutral">Not linked</Badge>;
  return (
    <Badge tone={v.partner_status === "ok" ? "good" : "bad"}>
      {v.partner_status === "ok" ? "Linked" : "Needs attention"}
    </Badge>
  );
}

function PartnerLinkPanel({ vendor }: { vendor: Counterparty }) {
  const { can } = useAuth();
  const manage = can("entity:manage");
  const qc = useQueryClient();
  const key = ["partner-link", vendor.id];
  const link = useQuery({
    queryKey: key,
    queryFn: () => partner.link(vendor.id),
  });
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  const after = (ok: string) => (r?: LinkResult | unknown) => {
    toast.success(ok);
    const res = r as LinkResult | undefined;
    if (res?.callback_secret) setSecret(res.callback_secret);
    qc.invalidateQueries({ queryKey: key });
    qc.invalidateQueries({ queryKey: ["entities", "all"] });
  };
  const onError = (e: Error) => toast.error(errorText(e));

  const connect = useMutation({
    mutationFn: () =>
      partner.connect(vendor.id, {
        base_url: baseUrl.trim(),
        api_key: apiKey.trim(),
      }),
    onSuccess: (r) => {
      setApiKey("");
      setEditing(false);
      after("Linked")(r);
    },
    onError,
  });
  const rotate = useMutation({
    mutationFn: () => partner.rotateSecret(vendor.id),
    onSuccess: after("New secret made. Give it to the vendor."),
    onError,
  });
  const unlink = useMutation({
    mutationFn: () => partner.disconnect(vendor.id),
    onSuccess: () => {
      setSecret(null);
      after("Unlinked")();
    },
    onError,
  });
  const invite = useMutation({
    mutationFn: () => desk.alphaInvite(vendor.id),
    onSuccess: () => toast.success(`Invite sent to ${vendor.email}`),
    onError,
  });
  const busy = connect.isPending || rotate.isPending || unlink.isPending;

  if (link.isLoading)
    return (
      <div className="px-5 pb-4">
        <Skeleton className="h-24" />
      </div>
    );
  if (link.error)
    return (
      <p className="px-5 pb-4 text-[13px] text-bad">{link.error.message}</p>
    );
  const l = link.data;
  if (!l) return null;

  return (
    <div className="space-y-4 border-t border-line bg-fill/40 px-5 py-4">
      {l.linked && !editing && (
        <dl>
          <Row label="Their server" value={l.base_url ?? ""} />
          <Row
            label="Linked"
            value={
              l.linked_at ? new Date(l.linked_at).toLocaleDateString() : ""
            }
            mono={false}
          />
          {l.error && (
            <Row
              label="Last error"
              value={<span className="text-bad">{l.error}</span>}
              mono={false}
            />
          )}
        </dl>
      )}

      {manage && (!l.linked || editing) && (
        <form
          className="grid gap-3 sm:grid-cols-[1fr_1fr_auto] sm:items-end"
          onSubmit={(e) => {
            e.preventDefault();
            if (baseUrl.trim() && apiKey.trim()) connect.mutate();
          }}
        >
          <Field label="Their Alpha server">
            {(p) => (
              <Input
                {...p}
                placeholder="https://api.their-alpha.com"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
              />
            )}
          </Field>
          <Field label="Alpha API key (write:jobs)">
            {(p) => (
              <Input
                {...p}
                type="password"
                autoComplete="off"
                className="font-mono"
                placeholder="apw_live_…"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
              />
            )}
          </Field>
          <div className="flex gap-2">
            <Button
              type="submit"
              size="lg"
              disabled={!baseUrl.trim() || !apiKey.trim()}
              loading={connect.isPending}
            >
              {!connect.isPending && <Link2 />}
              {l.linked ? "Update link" : "Link vendor"}
            </Button>
            {editing && (
              <Button
                type="button"
                size="lg"
                variant="ghost"
                onClick={() => setEditing(false)}
              >
                Cancel
              </Button>
            )}
          </div>
        </form>
      )}

      {l.linked && <OpenInAlpha counterpartyId={vendor.id} />}

      <div className="rounded-xl border border-line bg-fill/60 p-4">
        <div className="eyebrow">For their Alpha API client</div>
        <dl className="mt-1">
          <Row
            label="Callback address"
            value={l.callback_url}
            copy={l.callback_url}
          />
          {!secret && (
            <Row
              label="Secret"
              mono={false}
              value={
                <span className="text-fg-3">
                  {l.callback_secret_set
                    ? "Set. Make a new one to see it."
                    : "Made when you link."}
                </span>
              }
            />
          )}
        </dl>
        {secret && (
          <div className="mt-3">
            <SecretOnce
              title="Give this secret to the vendor now. It is not shown again."
              secret={secret}
              onDismiss={() => setSecret(null)}
            />
          </div>
        )}
      </div>

      {manage && (
        <div className="flex flex-wrap gap-2">
          {l.linked && !editing && (
            <>
              <Button
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => setEditing(true)}
              >
                <KeyRound />
                Change key
              </Button>
              <Button
                size="sm"
                variant="secondary"
                loading={rotate.isPending}
                disabled={busy}
                onClick={() => {
                  if (
                    window.confirm(
                      "Make a new secret? Their Alpha stops sending updates until they paste it in."
                    )
                  )
                    rotate.mutate();
                }}
              >
                {!rotate.isPending && <RefreshCw />}
                New secret
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  if (window.confirm(`Unlink ${vendor.name} from Alpha?`))
                    unlink.mutate();
                }}
              >
                <Link2Off />
                Unlink
              </Button>
            </>
          )}
          {!l.linked && (
            <Button
              size="sm"
              variant="ghost"
              disabled={!vendor.email}
              loading={invite.isPending}
              onClick={() => invite.mutate()}
              title={
                vendor.email ? undefined : "Add an email for this vendor first"
              }
            >
              {!invite.isPending && <Mail />}
              Invite them to Alpha
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

/** Opens the vendor's Alpha signed in as you, when single sign-on is on. */
function OpenInAlpha({ counterpartyId }: { counterpartyId: string }) {
  const { can } = useAuth();
  const status = useQuery({
    queryKey: SSO_KEY,
    queryFn: sso.status,
    enabled: can("integrations:manage"),
    retry: false,
  });
  const [web, setWeb] = useState("");
  const launch = useMutation({
    mutationFn: () =>
      sso.launch({
        counterparty_id: counterpartyId,
        web_url: web.trim() || undefined,
      }),
    onSuccess: (r) => window.open(r.url, "_blank", "noopener,noreferrer"),
    onError: (e) => toast.error(errorText(e)),
  });
  if (!status.data?.enabled) return null;
  return (
    <form
      className="flex flex-col gap-2 sm:flex-row"
      onSubmit={(e) => {
        e.preventDefault();
        launch.mutate();
      }}
    >
      <Input
        aria-label="Their Alpha web address"
        placeholder="Their Alpha web address (first time only)"
        value={web}
        onChange={(e) => setWeb(e.target.value)}
      />
      <Button
        type="submit"
        variant="secondary"
        size="lg"
        loading={launch.isPending}
      >
        {!launch.isPending && <ExternalLink />}
        Open in Alpha
      </Button>
    </form>
  );
}
