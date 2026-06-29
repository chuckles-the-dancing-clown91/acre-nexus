"use client";

import { useMemo, useState } from "react";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";
import { Check, Copy, KeyRound, Plus, ShieldOff, Trash2 } from "lucide-react";

import { useAuth } from "@/lib/auth";
import {
  useApiTokens,
  useCreateApiToken,
  useRevokeApiToken,
} from "@/lib/queries";
import { API_BASE, type CreateTokenResponse, type TokenSummary } from "@/lib/api";
import {
  createTokenSchema,
  type CreateTokenInput,
  TOKEN_SCOPES,
} from "@/lib/schemas";
import { relativeDate } from "@/lib/format";
import { cn } from "@/lib/utils";

import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Field, Input } from "@/components/ui/form-field";
import { Badge } from "@/components/ui";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

export default function TokensPage() {
  const { can } = useAuth();
  const canManage = can("apitoken:manage");

  const tokens = useApiTokens();
  const createToken = useCreateApiToken();
  const revokeToken = useRevokeApiToken();

  const [createOpen, setCreateOpen] = useState(false);
  // The plaintext token, shown once after creation in a separate reveal dialog.
  const [revealed, setRevealed] = useState<CreateTokenResponse | null>(null);
  const [copied, setCopied] = useState(false);
  // Pending revoke target (row), confirmed in a dialog before mutating.
  const [revokeTarget, setRevokeTarget] = useState<TokenSummary | null>(null);

  const rows = useMemo(() => tokens.data ?? [], [tokens.data]);

  const { activeCount, revokedCount } = useMemo(() => {
    let active = 0;
    let revoked = 0;
    for (const t of rows) {
      if (t.revoked) revoked += 1;
      else active += 1;
    }
    return { activeCount: active, revokedCount: revoked };
  }, [rows]);

  const {
    control,
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateTokenInput>({
    resolver: zodResolver(createTokenSchema),
    defaultValues: { name: "", scopes: ["listing:read"] },
  });

  const onSubmit = handleSubmit(async (values) => {
    try {
      const res = await createToken.mutateAsync(values);
      reset({ name: "", scopes: ["listing:read"] });
      setCreateOpen(false);
      setCopied(false);
      setRevealed(res);
      toast.success("API token created", {
        description: "Copy the secret now — it won't be shown again.",
      });
    } catch (e) {
      toast.error("Couldn't create token", {
        description: e instanceof Error ? e.message : "Please try again.",
      });
    }
  });

  async function copyToken() {
    if (!revealed) return;
    try {
      await navigator.clipboard.writeText(revealed.token);
      setCopied(true);
      toast.success("Token copied to clipboard");
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast.error("Couldn't copy", {
        description: "Select the token and copy it manually.",
      });
    }
  }

  function confirmRevoke() {
    if (!revokeTarget) return;
    const target = revokeTarget;
    revokeToken.mutate(target.id, {
      onSuccess: () => {
        toast.success("Token revoked", {
          description: `"${target.name}" can no longer access the API.`,
        });
        setRevokeTarget(null);
      },
      onError: (e) =>
        toast.error("Couldn't revoke token", { description: e.message }),
    });
  }

  const columns: ColumnDef<TokenSummary>[] = [
    {
      accessorKey: "name",
      header: "Name",
      cell: ({ row }) => (
        <span className="font-medium text-ink">{row.original.name}</span>
      ),
    },
    {
      accessorKey: "prefix",
      header: "Prefix",
      cell: ({ row }) => (
        <code className="font-mono text-xs text-ink-2">
          {row.original.prefix}…
        </code>
      ),
    },
    {
      id: "scopes",
      header: "Scopes",
      enableSorting: false,
      cell: ({ row }) => (
        <div className="flex flex-wrap gap-1.5">
          {row.original.scopes.length === 0 ? (
            <span className="text-xs text-ink-3">—</span>
          ) : (
            row.original.scopes.map((s) => (
              <Badge key={s} tone="neutral" className="font-mono">
                {s}
              </Badge>
            ))
          )}
        </div>
      ),
    },
    {
      accessorKey: "last_used_at",
      header: "Last used",
      cell: ({ row }) =>
        row.original.last_used_at ? (
          <span className="text-ink-2">
            {relativeDate(row.original.last_used_at)}
          </span>
        ) : (
          <span className="text-ink-3">Never</span>
        ),
    },
    {
      id: "status",
      header: "Status",
      enableSorting: false,
      cell: ({ row }) =>
        row.original.revoked ? (
          <Badge tone="bad">Revoked</Badge>
        ) : (
          <Badge tone="good">Active</Badge>
        ),
    },
    {
      id: "actions",
      header: "",
      enableSorting: false,
      cell: ({ row }) =>
        canManage && !row.original.revoked ? (
          <div className="flex justify-end">
            <Button
              variant="ghost"
              size="sm"
              className="text-bad hover:bg-bad-soft hover:text-bad"
              onClick={() => setRevokeTarget(row.original)}
            >
              <Trash2 className="h-4 w-4" />
              Revoke
            </Button>
          </div>
        ) : null,
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Developers"
        title="API tokens"
        description={
          <>
            Scoped, revocable keys for the vendor API at{" "}
            <code className="font-mono text-ink-2">{API_BASE}/api/v1</code>.
          </>
        }
        actions={
          canManage ? (
            <Dialog open={createOpen} onOpenChange={setCreateOpen}>
              <DialogTrigger asChild>
                <Button>
                  <Plus className="h-4 w-4" />
                  Create token
                </Button>
              </DialogTrigger>
              <DialogContent>
                <form onSubmit={onSubmit}>
                  <DialogHeader>
                    <DialogTitle>Create API token</DialogTitle>
                    <DialogDescription>
                      Name the token and choose the scopes it may use. The secret
                      is shown once after creation.
                    </DialogDescription>
                  </DialogHeader>

                  <div className="my-5 space-y-4">
                    <Field
                      label="Name"
                      htmlFor="token-name"
                      required
                      error={errors.name?.message}
                    >
                      <Input
                        id="token-name"
                        placeholder="e.g. Zillow sync"
                        error={!!errors.name}
                        {...register("name")}
                      />
                    </Field>

                    <Field
                      label="Scopes"
                      required
                      error={errors.scopes?.message}
                    >
                      <Controller
                        control={control}
                        name="scopes"
                        render={({ field }) => (
                          <div className="space-y-2">
                            {TOKEN_SCOPES.map((s) => {
                              const on = field.value.includes(s);
                              return (
                                <label
                                  key={s}
                                  className={cn(
                                    "flex cursor-pointer items-center gap-3 rounded-lg border px-3 py-2.5 transition",
                                    on
                                      ? "border-accent bg-accent-soft"
                                      : "border-line bg-surface hover:bg-surface-2"
                                  )}
                                >
                                  <span
                                    className={cn(
                                      "flex h-4 w-4 items-center justify-center rounded border transition",
                                      on
                                        ? "border-accent bg-accent text-on-accent"
                                        : "border-line-2 bg-surface"
                                    )}
                                  >
                                    {on && <Check className="h-3 w-3" />}
                                  </span>
                                  <input
                                    type="checkbox"
                                    className="sr-only"
                                    checked={on}
                                    onChange={() =>
                                      field.onChange(
                                        on
                                          ? field.value.filter((x) => x !== s)
                                          : [...field.value, s]
                                      )
                                    }
                                  />
                                  <code className="font-mono text-xs font-bold text-ink">
                                    {s}
                                  </code>
                                </label>
                              );
                            })}
                          </div>
                        )}
                      />
                    </Field>
                  </div>

                  <DialogFooter>
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => setCreateOpen(false)}
                    >
                      Cancel
                    </Button>
                    <Button type="submit" disabled={isSubmitting}>
                      {isSubmitting ? "Creating…" : "Create token"}
                    </Button>
                  </DialogFooter>
                </form>
              </DialogContent>
            </Dialog>
          ) : undefined
        }
      />

      {/* Stats */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-3">
        {tokens.isLoading ? (
          Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Total tokens"
              value={rows.length}
              sub="Issued for the vendor API"
              icon={KeyRound}
            />
            <StatCard
              label="Active"
              value={activeCount}
              sub="Currently usable"
              icon={Check}
              tone="good"
            />
            <StatCard
              label="Revoked"
              value={revokedCount}
              sub="No longer usable"
              icon={ShieldOff}
              tone={revokedCount > 0 ? "bad" : "neutral"}
            />
          </>
        )}
      </div>

      {/* Token list */}
      <DataTable
        columns={columns}
        data={rows}
        isLoading={tokens.isLoading}
        searchPlaceholder="Search tokens…"
        enableSearch={rows.length > 0}
        emptyState={
          <EmptyState
            className="border-0"
            icon={KeyRound}
            title="No API tokens yet"
            description={
              canManage
                ? "Create a scoped token to give a vendor programmatic access to the API."
                : "No tokens have been issued for the vendor API."
            }
            action={
              canManage ? (
                <Button onClick={() => setCreateOpen(true)}>
                  <Plus className="h-4 w-4" />
                  Create token
                </Button>
              ) : undefined
            }
          />
        }
      />

      {/* One-time token reveal */}
      <Dialog
        open={!!revealed}
        onOpenChange={(open) => {
          if (!open) setRevealed(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Copy your API token</DialogTitle>
            <DialogDescription>
              This is the only time the full secret is shown. Store it somewhere
              safe — you can&apos;t retrieve it again.
            </DialogDescription>
          </DialogHeader>

          {revealed && (
            <div className="my-4 space-y-3">
              <div className="rounded-lg border border-line bg-surface-2 p-3">
                <code className="block break-all font-mono text-sm text-ink">
                  {revealed.token}
                </code>
              </div>
              <Button
                type="button"
                variant="outline"
                className="w-full"
                onClick={copyToken}
              >
                {copied ? (
                  <>
                    <Check className="h-4 w-4 text-good" />
                    Copied
                  </>
                ) : (
                  <>
                    <Copy className="h-4 w-4" />
                    Copy to clipboard
                  </>
                )}
              </Button>
            </div>
          )}

          <DialogFooter>
            <Button onClick={() => setRevealed(null)}>Done</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Revoke confirmation */}
      <Dialog
        open={!!revokeTarget}
        onOpenChange={(open) => {
          if (!open && !revokeToken.isPending) setRevokeTarget(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Revoke API token</DialogTitle>
            <DialogDescription>
              {revokeTarget ? (
                <>
                  Revoking <span className="font-semibold text-ink">{revokeTarget.name}</span>{" "}
                  immediately blocks any client using it. This can&apos;t be
                  undone.
                </>
              ) : null}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setRevokeTarget(null)}
              disabled={revokeToken.isPending}
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={confirmRevoke}
              disabled={revokeToken.isPending}
            >
              {revokeToken.isPending ? "Revoking…" : "Revoke token"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
