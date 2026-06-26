"use client";

// Acre platform user directory: searchable table of every user across tenants,
// with a "New user" dialog that can seed an initial membership + profile basics.

import { useState } from "react";
import Link from "next/link";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";

import { useAuth } from "@/lib/auth";
import type { CreateUserInput, ProfileInput } from "@/lib/api";
import { useCreateUser, useProfileTypes, useUsers } from "@/lib/queries";
import { createUserSchema, type CreateUserInputForm } from "@/lib/schemas";
import { Badge, Card, statusTone } from "@/components/ui";
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

/** Platform-staff directory of every Acre user, with creation. */
export default function PlatformUsersPage() {
  const { can, user } = useAuth();
  const [q, setQ] = useState("");
  const { data: users, error, isLoading } = useUsers(q);
  const canManage = can("user:manage");

  if (!user?.is_platform_staff) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">This directory is platform-staff only.</p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Users
          </h1>
          <p className="text-ink-3">
            Every person across all client workspaces and Acre HQ.
          </p>
        </div>
        {canManage && <NewUserDialog />}
      </div>

      <div className="max-w-sm">
        <Input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search by name or email…"
          aria-label="Search users"
        />
      </div>

      {error && <p className="text-bad">{error.message}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.5fr_1.5fr_.7fr_.6fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Name</span>
          <span>Email</span>
          <span>Role</span>
          <span className="text-right">Status</span>
        </div>
        <div className="divide-y divide-line">
          {users?.map((u) => (
            <Link
              key={u.id}
              href={`/console/platform/users/${u.id}`}
              className="grid grid-cols-[1.5fr_1.5fr_.7fr_.6fr] items-center gap-4 px-5 py-3.5 hover:bg-surface-2"
            >
              <div className="min-w-0">
                <div className="truncate font-semibold">{u.name}</div>
                {u.username && (
                  <div className="truncate text-sm text-ink-3">
                    @{u.username}
                  </div>
                )}
              </div>
              <span className="truncate text-sm text-ink-2">{u.email}</span>
              <span className="text-sm">
                {u.is_platform_staff ? (
                  <Badge tone="info">Staff</Badge>
                ) : (
                  <span className="text-ink-3">Client</span>
                )}
              </span>
              <span className="flex justify-end">
                <Badge tone={statusTone(u.status)}>{u.status}</Badge>
              </span>
            </Link>
          ))}
          {isLoading && (
            <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
          )}
          {users && users.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No users match “{q}”.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

/** Dialog to create a user with an optional membership + profile basics. */
function NewUserDialog() {
  const [open, setOpen] = useState(false);
  const createUser = useCreateUser();
  const { data: profileTypes } = useProfileTypes();

  const {
    register,
    handleSubmit,
    watch,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateUserInputForm>({
    resolver: zodResolver(createUserSchema),
    defaultValues: { scope: "tenant", email: "", name: "" },
  });

  const scope = watch("scope");
  const scopedTypes = (profileTypes ?? []).filter((t) => t.scope === scope);

  const onSubmit = handleSubmit(async (values) => {
    // Assemble the typed API payload, dropping empty optional fields.
    const profile: ProfileInput = {};
    if (values.legal_first_name)
      profile.legal_first_name = values.legal_first_name;
    if (values.legal_last_name)
      profile.legal_last_name = values.legal_last_name;
    if (values.phone) profile.phone = values.phone;

    const body: CreateUserInput = {
      email: values.email,
      name: values.name,
    };
    if (values.username) body.username = values.username;
    if (values.password) body.password = values.password;
    if (values.profile_type) {
      body.membership = {
        scope: values.scope,
        profile_type: values.profile_type,
      };
      if (values.scope === "tenant" && values.tenant_id)
        body.membership.tenant_id = values.tenant_id;
      if (values.title) body.membership.title = values.title;
    }
    if (Object.keys(profile).length > 0) body.profile = profile;

    await createUser.mutateAsync(body, {
      onSuccess: () => {
        reset({ scope: "tenant", email: "", name: "" });
        setOpen(false);
      },
    });
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>New user</Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>New user</DialogTitle>
            <DialogDescription>
              Create a user, optionally with a starting membership and profile.
            </DialogDescription>
          </DialogHeader>

          <div className="my-5 space-y-4">
            <Field label="Email" error={errors.email?.message}>
              <Input
                type="email"
                placeholder="person@example.com"
                aria-invalid={!!errors.email}
                {...register("email")}
              />
            </Field>
            <Field label="Full name" error={errors.name?.message}>
              <Input
                placeholder="Jane Doe"
                aria-invalid={!!errors.name}
                {...register("name")}
              />
            </Field>
            <div className="grid grid-cols-2 gap-3">
              <Field label="Username (optional)">
                <Input placeholder="jdoe" {...register("username")} />
              </Field>
              <Field label="Password (optional)">
                <Input
                  type="password"
                  placeholder="Auto if blank"
                  {...register("password")}
                />
              </Field>
            </div>

            <div className="border-t border-line pt-4">
              <p className="mb-3 text-sm font-bold text-ink-2">
                Membership (optional)
              </p>
              <div className="grid grid-cols-2 gap-3">
                <Field label="Scope">
                  <select
                    className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                    {...register("scope")}
                  >
                    <option value="tenant">Tenant</option>
                    <option value="platform">Platform</option>
                  </select>
                </Field>
                <Field label="Persona">
                  <select
                    className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                    {...register("profile_type")}
                  >
                    <option value="">— None —</option>
                    {scopedTypes.map((t) => (
                      <option key={t.key} value={t.key}>
                        {t.label}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
              {scope === "tenant" && (
                <div className="mt-3 grid grid-cols-2 gap-3">
                  <Field label="Tenant ID">
                    <Input
                      placeholder="tenant uuid"
                      {...register("tenant_id")}
                    />
                  </Field>
                  <Field label="Title (optional)">
                    <Input placeholder="e.g. Owner" {...register("title")} />
                  </Field>
                </div>
              )}
            </div>

            <div className="border-t border-line pt-4">
              <p className="mb-3 text-sm font-bold text-ink-2">
                Profile basics (optional)
              </p>
              <div className="grid grid-cols-2 gap-3">
                <Field label="Legal first name">
                  <Input {...register("legal_first_name")} />
                </Field>
                <Field label="Legal last name">
                  <Input {...register("legal_last_name")} />
                </Field>
              </div>
              <div className="mt-3">
                <Field label="Phone">
                  <Input {...register("phone")} />
                </Field>
              </div>
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
              {isSubmitting ? "Creating…" : "Create user"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Labelled form field with an optional inline error. */
function Field({
  label,
  error,
  children,
}: {
  label: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      {children}
      {error && (
        <p className="text-sm text-bad" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
