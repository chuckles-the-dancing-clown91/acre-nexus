"use client";

// Canonical form pattern for the app: react-hook-form + zod validation
// (@hookform/resolvers), submitted inside a shadcn Dialog, with a sonner toast
// on success. Server reads/writes go through TanStack Query hooks (useApiTokens
// / useCreateApiToken / useRevokeApiToken).

import { useState } from "react";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";

import { API_BASE, type CreateTokenResponse } from "@/lib/api";
import { Badge, Card } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  createTokenSchema,
  type CreateTokenInput,
  TOKEN_SCOPES,
} from "@/lib/schemas";
import {
  useApiTokens,
  useCreateApiToken,
  useRevokeApiToken,
} from "@/lib/queries";

export default function TokensPage() {
  const { data: tokens, error } = useApiTokens();
  const revoke = useRevokeApiToken();
  const createToken = useCreateApiToken();

  const [open, setOpen] = useState(false);
  const [created, setCreated] = useState<CreateTokenResponse | null>(null);

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
      setCreated(res);
      toast.success("API token created", {
        description: `“${res.name}” is ready — copy it now, it won't be shown again.`,
      });
      reset({ name: "", scopes: ["listing:read"] });
      setOpen(false);
    } catch (e) {
      const message = e instanceof Error ? e.message : "Failed to create token";
      toast.error("Couldn't create token", { description: message });
    }
  });

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            API tokens
          </h1>
          <p className="text-ink-3">
            Scoped, revocable keys for the vendor API (
            <code>{API_BASE}/api/v1</code>).
          </p>
        </div>

        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild>
            <Button>Create token</Button>
          </DialogTrigger>
          <DialogContent>
            <form onSubmit={onSubmit}>
              <DialogHeader>
                <DialogTitle>Create API token</DialogTitle>
                <DialogDescription>
                  Name the token and pick the scopes it may use.
                </DialogDescription>
              </DialogHeader>

              <div className="my-5 space-y-4">
                <div className="space-y-1.5">
                  <Label htmlFor="token-name">Name</Label>
                  <Input
                    id="token-name"
                    placeholder="e.g. Zillow sync"
                    aria-invalid={!!errors.name}
                    {...register("name")}
                  />
                  {errors.name && (
                    <p className="text-sm text-bad" role="alert">
                      {errors.name.message}
                    </p>
                  )}
                </div>

                <div className="space-y-1.5">
                  <Label>Scopes</Label>
                  <Controller
                    control={control}
                    name="scopes"
                    render={({ field }) => (
                      <div className="flex flex-wrap gap-2">
                        {TOKEN_SCOPES.map((s) => {
                          const on = field.value.includes(s);
                          return (
                            <button
                              type="button"
                              key={s}
                              onClick={() =>
                                field.onChange(
                                  on
                                    ? field.value.filter((x) => x !== s)
                                    : [...field.value, s]
                                )
                              }
                              className={`rounded-full border px-3 py-1.5 font-mono text-xs font-bold transition ${
                                on
                                  ? "border-accent bg-accent-soft text-accent-2"
                                  : "border-line-2 text-ink-3"
                              }`}
                            >
                              {s}
                            </button>
                          );
                        })}
                      </div>
                    )}
                  />
                  {errors.scopes && (
                    <p className="text-sm text-bad" role="alert">
                      {errors.scopes.message}
                    </p>
                  )}
                </div>
              </div>

              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setOpen(false)}
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
      </div>

      {error && <p className="text-bad">{error.message}</p>}

      {created && (
        <Card className="border-good-soft bg-good-soft p-5">
          <p className="mb-2 font-bold text-good">
            Token created — copy it now, it won&apos;t be shown again:
          </p>
          <code className="block break-all rounded-lg bg-surface px-3 py-2 font-mono text-sm">
            {created.token}
          </code>
        </Card>
      )}

      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Active tokens
        </div>
        <div className="divide-y divide-line">
          {tokens?.map((t) => (
            <div key={t.id} className="flex items-center gap-4 px-5 py-3.5">
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{t.name}</div>
                <code className="font-mono text-xs text-ink-3">
                  {t.prefix}…
                </code>
              </div>
              <div className="hidden gap-1 sm:flex">
                {t.scopes.map((s) => (
                  <Badge key={s} tone="neutral">
                    {s}
                  </Badge>
                ))}
              </div>
              {t.revoked ? (
                <Badge tone="bad">Revoked</Badge>
              ) : (
                <button
                  onClick={() =>
                    revoke.mutate(t.id, {
                      onSuccess: () => toast.success("Token revoked"),
                      onError: (e) =>
                        toast.error("Couldn't revoke token", {
                          description: e.message,
                        }),
                    })
                  }
                  className="text-sm font-semibold text-bad hover:underline"
                >
                  Revoke
                </button>
              )}
            </div>
          ))}
          {tokens && tokens.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No tokens yet.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
