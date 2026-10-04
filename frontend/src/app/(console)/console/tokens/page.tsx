"use client";

// API tokens: scoped, revocable keys for the vendor API. A new token's secret
// is shown once, with a copy button; after that only its prefix shows.

import { useState } from "react";
import { KeyRound, Plus, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { API_BASE, type CreateTokenResponse } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import {
  useApiTokens,
  useCreateApiToken,
  useRevokeApiToken,
} from "@/lib/queries";
import { createTokenSchema, TOKEN_SCOPES } from "@/lib/schemas";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/ui/data-table";
import { Field, Input, Label } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import {
  CopyButton,
  errorText,
  SecretOnce,
} from "@/app/(console)/console/integrations/bits";

type Scope = (typeof TOKEN_SCOPES)[number];

const SCOPE_WORDS: Record<Scope, string> = {
  "listing:read": "Read listings",
  "property:read": "Read properties",
  "application:read": "Read applications",
};

function when(iso: string | null) {
  return iso ? new Date(iso).toLocaleDateString() : "Never";
}

export default function TokensPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("apitoken:manage");
  const tokens = useApiTokens({ enabled: scoped && allowed });
  const revoke = useRevokeApiToken();
  const [open, setOpen] = useState(false);
  const [created, setCreated] = useState<CreateTokenResponse | null>(null);
  const [showRevoked, setShowRevoked] = useState(false);
  const endpoint = `${API_BASE}/api/v1`;

  const all = tokens.data ?? [];
  const revokedCount = all.filter((t) => t.revoked).length;
  const rows = all.filter((t) => showRevoked || !t.revoked);

  function onRevoke(id: string, name: string) {
    if (
      !window.confirm(
        `Revoke “${name}”? Anything using it stops working right away.`
      )
    )
      return;
    revoke.mutate(id, {
      onSuccess: () => toast.success("Token revoked"),
      onError: (e) => toast.error(errorText(e, "Couldn't revoke the token")),
    });
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="API tokens"
        description="Scoped keys for the vendor API. Revoke one and it stops working at once."
        actions={
          allowed && (
            <Button onClick={() => setOpen(true)}>
              <Plus />
              New token
            </Button>
          )
        }
      />

      {!allowed && (
        <Panel>
          <EmptyState
            icon={<KeyRound />}
            title="You can't manage API tokens"
            description="Ask a company owner for the apitoken:manage permission."
          />
        </Panel>
      )}

      {allowed && (
        <>
          <Panel className="flex flex-col gap-3 p-5 sm:flex-row sm:items-center">
            <div className="min-w-0 flex-1">
              <div className="eyebrow">Endpoint</div>
              <code className="mt-1 block font-mono text-[13px] break-all text-fg">
                {endpoint}
              </code>
              <p className="mt-1 text-[12px] text-fg-3">
                Send the token as{" "}
                <code className="font-mono">Authorization: Bearer …</code>. A
                token without the scope a call needs gets a 403.
              </p>
            </div>
            <CopyButton value={endpoint} />
          </Panel>

          {created && (
            <SecretOnce
              title={`“${created.name}” is ready. Copy the token now; it is not shown again.`}
              secret={created.token}
              onDismiss={() => setCreated(null)}
            />
          )}

          <Panel>
            <PanelHeader
              title="Tokens"
              description="Only each token's first characters show after it's made."
              action={
                revokedCount > 0 && (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setShowRevoked((v) => !v)}
                  >
                    {showRevoked
                      ? "Hide revoked"
                      : `Show revoked · ${revokedCount}`}
                  </Button>
                )
              }
            />
            <div className="pt-3">
              {tokens.isLoading ? (
                <div className="px-5 pb-5">
                  <Skeleton className="h-32" />
                </div>
              ) : tokens.error ? (
                <p className="px-5 pb-5 text-[13px] text-bad">
                  Couldn&apos;t load tokens: {tokens.error.message}
                </p>
              ) : rows.length === 0 ? (
                <EmptyState
                  icon={<KeyRound />}
                  title="No tokens yet"
                  description="Make one for each system that reads your listings or applications, so you can revoke them one at a time."
                  action={
                    <Button onClick={() => setOpen(true)}>
                      <Plus />
                      New token
                    </Button>
                  }
                />
              ) : (
                <DataTable
                  columns={[
                    "Name",
                    "Token",
                    "Scopes",
                    "Last used",
                    "Created",
                    "",
                  ]}
                  rows={rows.map((t) => [
                    t.name,
                    <code key="p" className="font-mono text-xs text-fg-3">
                      {t.prefix}…
                    </code>,
                    <span key="s" className="flex flex-wrap gap-1">
                      {t.scopes.map((s) => (
                        <Badge key={s} tone="neutral">
                          {s}
                        </Badge>
                      ))}
                    </span>,
                    when(t.last_used_at),
                    when(t.created_at),
                    t.revoked ? (
                      <Badge key="r" tone="bad">
                        Revoked
                      </Badge>
                    ) : (
                      <Button
                        key="r"
                        size="sm"
                        variant="ghost"
                        disabled={revoke.isPending}
                        onClick={() => onRevoke(t.id, t.name)}
                      >
                        <Trash2 />
                        Revoke
                      </Button>
                    ),
                  ])}
                />
              )}
            </div>
          </Panel>
        </>
      )}

      <NewTokenDialog
        open={open}
        onOpenChange={setOpen}
        onCreated={(t) => {
          setCreated(t);
          setOpen(false);
        }}
      />
    </div>
  );
}

function NewTokenDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: (t: CreateTokenResponse) => void;
}) {
  const create = useCreateApiToken();
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState<Scope[]>(["listing:read"]);
  const [errors, setErrors] = useState<{ name?: string; scopes?: string }>({});

  function reset() {
    setName("");
    setScopes(["listing:read"]);
    setErrors({});
  }

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const parsed = createTokenSchema.safeParse({ name, scopes });
    if (!parsed.success) {
      const f = parsed.error.flatten().fieldErrors;
      setErrors({ name: f.name?.[0], scopes: f.scopes?.[0] });
      return;
    }
    setErrors({});
    create.mutate(parsed.data, {
      onSuccess: (res) => {
        toast.success("Token created");
        reset();
        onCreated(res);
      },
      onError: (err) =>
        toast.error(errorText(err, "Couldn't create the token")),
    });
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) reset();
        onOpenChange(o);
      }}
    >
      <DialogContent>
        <form onSubmit={submit} className="space-y-5">
          <div>
            <DialogTitle className="text-[17px] font-semibold text-fg">
              New API token
            </DialogTitle>
            <DialogDescription className="mt-1 text-[13px] text-fg-3">
              Name it after the system that will use it, and give it only the
              scopes it needs.
            </DialogDescription>
          </div>
          <Field label="Name" error={errors.name}>
            {(p) => (
              <Input
                {...p}
                autoFocus
                placeholder="Zillow sync"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            )}
          </Field>
          <div className="space-y-1.5">
            <Label>Scopes</Label>
            <div className="flex flex-wrap gap-2">
              {TOKEN_SCOPES.map((s) => {
                const on = scopes.includes(s);
                return (
                  <button
                    key={s}
                    type="button"
                    aria-pressed={on}
                    onClick={() =>
                      setScopes(
                        on ? scopes.filter((x) => x !== s) : [...scopes, s]
                      )
                    }
                    className={cn(
                      "rounded-full border px-3 py-1 text-[13px] transition",
                      on
                        ? "border-accent bg-accent/10 text-accent"
                        : "border-line text-fg-3 hover:text-fg"
                    )}
                  >
                    {SCOPE_WORDS[s]}
                    <span className="ml-1.5 font-mono text-[11px] opacity-70">
                      {s}
                    </span>
                  </button>
                );
              })}
            </div>
            {errors.scopes && (
              <p className="text-xs text-bad" role="alert">
                {errors.scopes}
              </p>
            )}
          </div>
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              onClick={() => {
                reset();
                onOpenChange(false);
              }}
            >
              Cancel
            </Button>
            <Button type="submit" loading={create.isPending}>
              Create token
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
