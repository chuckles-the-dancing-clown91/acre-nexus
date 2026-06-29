"use client";

// Acre PLATFORM-ADMIN user directory: every user across the whole platform.
// A debounced search box drives the server-side `useUsers(q)` query; the table
// surfaces scope (Platform vs Tenant), status, and tenant, and links each row
// to the user's detail page. Staff with "user:manage" can create new users.

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Search, ShieldCheck, UserPlus, Users } from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useCreateUser, useProfileTypes, useUsers } from "@/lib/queries";
import type { CreateUserInput, MembershipInput, UserSummary } from "@/lib/api";
import { createUserSchema, type CreateUserInputForm } from "@/lib/schemas";
import { initials, titleCase } from "@/lib/format";

import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { Field, Input, TextField } from "@/components/ui/form-field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

/** Platform-staff directory of every Acre user across all tenants. */
export default function PlatformUsersPage() {
  const { can } = useAuth();
  const router = useRouter();
  const canManage = can("user:manage");

  // Debounce the search box: typing updates `term` immediately, but the server
  // query `q` only follows ~300ms later so we don't refetch on every keystroke.
  const [term, setTerm] = useState("");
  const [q, setQ] = useState("");
  useEffect(() => {
    const id = setTimeout(() => setQ(term.trim()), 300);
    return () => clearTimeout(id);
  }, [term]);

  const users = useUsers(q);
  const rows = users.data ?? [];
  const staffCount = rows.filter((u) => u.is_platform_staff).length;

  const columns = useMemo<ColumnDef<UserSummary, unknown>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => {
          const u = row.original;
          return (
            <div className="flex items-center gap-3">
              <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-accent-soft text-xs font-bold text-accent-2">
                {initials(u.name)}
              </span>
              <div className="min-w-0">
                <div className="truncate font-medium text-ink">{u.name}</div>
                {u.username && (
                  <div className="truncate text-xs text-ink-3">
                    @{u.username}
                  </div>
                )}
              </div>
            </div>
          );
        },
      },
      {
        accessorKey: "email",
        header: "Email",
        cell: ({ row }) => (
          <span className="text-ink-2">{row.original.email}</span>
        ),
      },
      {
        id: "scope",
        accessorKey: "is_platform_staff",
        header: "Scope",
        cell: ({ row }) =>
          row.original.is_platform_staff ? (
            <Badge tone="info">Platform</Badge>
          ) : (
            <Badge tone="neutral">Tenant</Badge>
          ),
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => (
          <Badge tone={statusTone(row.original.status)}>
            {titleCase(row.original.status)}
          </Badge>
        ),
      },
      {
        accessorKey: "tenant_id",
        header: "Tenant",
        cell: ({ row }) =>
          row.original.tenant_id ? (
            <span className="font-mono text-xs text-ink-2">
              {row.original.tenant_id}
            </span>
          ) : (
            <span className="text-ink-3">—</span>
          ),
      },
    ],
    []
  );

  const searching = q.length > 0;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Platform admin"
        title="Users"
        description="Every person across all client workspaces and Acre HQ."
        actions={canManage ? <NewUserDialog /> : undefined}
      />

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
        {users.isLoading ? (
          Array.from({ length: 2 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Users"
              value={rows.length}
              sub={searching ? `Matching “${q}”` : "Across the platform"}
              icon={Users}
            />
            <StatCard
              label="Platform staff"
              value={staffCount}
              sub="Acre HQ accounts"
              icon={ShieldCheck}
              tone="accent"
            />
          </>
        )}
      </div>

      <div className="relative max-w-xs">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-3" />
        <Input
          value={term}
          onChange={(e) => setTerm(e.target.value)}
          placeholder="Search by name or email…"
          aria-label="Search users"
          className="pl-9"
        />
      </div>

      <DataTable<UserSummary>
        columns={columns}
        data={rows}
        isLoading={users.isLoading}
        enableSearch={false}
        onRowClick={(u) => router.push(`/console/platform/users/${u.id}`)}
        emptyState={
          <EmptyState
            className="border-0"
            icon={Users}
            title={searching ? "No matching users" : "No users yet"}
            description={
              searching
                ? `No users match “${q}”. Try a different name or email.`
                : "Users created across the platform will appear here."
            }
            action={!searching && canManage ? <NewUserDialog /> : undefined}
          />
        }
      />
    </div>
  );
}

const EMPTY_FORM: CreateUserInputForm = {
  email: "",
  name: "",
  username: "",
  password: "",
  scope: "tenant",
  tenant_id: "",
  profile_type: "",
  title: "",
  legal_first_name: "",
  legal_last_name: "",
  phone: "",
};

/** Dialog to create a user, optionally with a starting membership + profile. */
function NewUserDialog() {
  const [open, setOpen] = useState(false);
  const createUser = useCreateUser();
  const { data: profileTypes } = useProfileTypes();

  const {
    register,
    handleSubmit,
    control,
    watch,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateUserInputForm>({
    resolver: zodResolver(createUserSchema),
    defaultValues: EMPTY_FORM,
  });

  const scope = watch("scope");
  const scopedTypes = (profileTypes ?? []).filter((t) => t.scope === scope);

  const onSubmit = handleSubmit(async (values) => {
    const body: CreateUserInput = {
      email: values.email,
      name: values.name,
    };
    if (values.username) body.username = values.username;
    if (values.password) body.password = values.password;

    // Only attach a membership when a persona is chosen.
    if (values.profile_type) {
      const membership: MembershipInput = {
        scope: values.scope,
        profile_type: values.profile_type,
      };
      if (values.scope === "tenant" && values.tenant_id)
        membership.tenant_id = values.tenant_id;
      if (values.title) membership.title = values.title;
      body.membership = membership;
    }

    // Drop empty optional profile fields.
    const profile: NonNullable<CreateUserInput["profile"]> = {};
    if (values.legal_first_name)
      profile.legal_first_name = values.legal_first_name;
    if (values.legal_last_name)
      profile.legal_last_name = values.legal_last_name;
    if (values.phone) profile.phone = values.phone;
    if (Object.keys(profile).length > 0) body.profile = profile;

    await createUser.mutateAsync(body, {
      onSuccess: () => {
        reset(EMPTY_FORM);
        setOpen(false);
      },
    });
  });

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) reset(EMPTY_FORM);
      }}
    >
      <DialogTrigger asChild>
        <Button>
          <UserPlus className="h-4 w-4" />
          New user
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>New user</DialogTitle>
            <DialogDescription>
              Create a user, optionally with a starting membership and profile.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <TextField
              label="Email"
              type="email"
              placeholder="person@example.com"
              required
              error={errors.email?.message}
              {...register("email")}
            />
            <TextField
              label="Full name"
              placeholder="Jane Doe"
              required
              error={errors.name?.message}
              {...register("name")}
            />
            <div className="grid grid-cols-2 gap-3">
              <TextField
                label="Username"
                hint="Optional."
                placeholder="jdoe"
                error={errors.username?.message}
                {...register("username")}
              />
              <TextField
                label="Password"
                type="password"
                hint="Auto-generated if blank."
                placeholder="••••••••"
                error={errors.password?.message}
                {...register("password")}
              />
            </div>

            <div className="border-t border-line pt-4">
              <p className="mb-3 text-xs font-semibold uppercase tracking-wide text-ink-3">
                Membership (optional)
              </p>
              <div className="grid grid-cols-2 gap-3">
                <Field label="Scope" error={errors.scope?.message}>
                  <Controller
                    control={control}
                    name="scope"
                    render={({ field }) => (
                      <Select
                        value={field.value}
                        onValueChange={field.onChange}
                      >
                        <SelectTrigger className="h-10">
                          <SelectValue placeholder="Select scope" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="tenant">Tenant</SelectItem>
                          <SelectItem value="platform">Platform</SelectItem>
                        </SelectContent>
                      </Select>
                    )}
                  />
                </Field>
                <Field label="Persona" error={errors.profile_type?.message}>
                  <Controller
                    control={control}
                    name="profile_type"
                    render={({ field }) => (
                      <Select
                        value={field.value || ""}
                        onValueChange={field.onChange}
                      >
                        <SelectTrigger
                          className="h-10"
                          disabled={scopedTypes.length === 0}
                        >
                          <SelectValue placeholder="None" />
                        </SelectTrigger>
                        <SelectContent>
                          {scopedTypes.map((t) => (
                            <SelectItem key={t.key} value={t.key}>
                              {t.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    )}
                  />
                </Field>
              </div>
              {scope === "tenant" && (
                <div className="mt-3 grid grid-cols-2 gap-3">
                  <TextField
                    label="Tenant ID"
                    hint="Required for tenant memberships."
                    placeholder="tenant uuid"
                    error={errors.tenant_id?.message}
                    {...register("tenant_id")}
                  />
                  <TextField
                    label="Title"
                    hint="Optional."
                    placeholder="e.g. Owner"
                    error={errors.title?.message}
                    {...register("title")}
                  />
                </div>
              )}
            </div>

            <div className="border-t border-line pt-4">
              <p className="mb-3 text-xs font-semibold uppercase tracking-wide text-ink-3">
                Profile basics (optional)
              </p>
              <div className="grid grid-cols-2 gap-3">
                <TextField
                  label="Legal first name"
                  error={errors.legal_first_name?.message}
                  {...register("legal_first_name")}
                />
                <TextField
                  label="Legal last name"
                  error={errors.legal_last_name?.message}
                  {...register("legal_last_name")}
                />
              </div>
              <div className="mt-3">
                <TextField
                  label="Phone"
                  placeholder="(555) 555-0100"
                  error={errors.phone?.message}
                  {...register("phone")}
                />
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
              <UserPlus className="h-4 w-4" />
              {isSubmitting ? "Creating…" : "Create user"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
