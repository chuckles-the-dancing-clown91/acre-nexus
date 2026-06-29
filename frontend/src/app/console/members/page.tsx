"use client";

// Tenant-scoped member directory (client-admin view): everyone with access to
// the current workspace, rebuilt on the Acre design system. Tenant is implied
// by the active session's JWT. Admins can invite new members under a persona.

import { useMemo, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Mail, Plus, UserPlus, Users } from "lucide-react";
import type { ColumnDef } from "@/components/ui/data-table";

import { useAuth } from "@/lib/auth";
import { useInviteMember, useMembers, useProfileTypes } from "@/lib/queries";
import type { Member } from "@/lib/api";
import { inviteMemberSchema, type InviteMemberInputForm } from "@/lib/schemas";
import { initials, titleCase } from "@/lib/format";

import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable } from "@/components/ui/data-table";
import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { Field, TextField } from "@/components/ui/form-field";
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

/** Tenant-scoped member directory for client admins. */
export default function MembersPage() {
  const { can } = useAuth();
  const members = useMembers();
  const canManage = can("member:manage");

  const rows = members.data ?? [];
  const activeCount = rows.filter((m) => m.status === "active").length;

  const columns = useMemo<ColumnDef<Member, unknown>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => {
          const m = row.original;
          return (
            <div className="flex items-center gap-3">
              <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-accent-soft text-xs font-bold text-accent-2">
                {initials(m.name)}
              </span>
              <span className="font-medium text-ink">{m.name}</span>
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
        accessorKey: "profile_type",
        header: "Persona",
        cell: ({ row }) => (
          <Badge tone="neutral">{titleCase(row.original.profile_type)}</Badge>
        ),
      },
      {
        accessorKey: "title",
        header: "Title",
        cell: ({ row }) =>
          row.original.title ? (
            <span className="text-ink-2">{row.original.title}</span>
          ) : (
            <span className="text-ink-3">—</span>
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
    ],
    []
  );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="Team members"
        description="People with access to your workspace and the persona they act under."
        actions={canManage ? <InviteMemberDialog /> : undefined}
      />

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
        {members.isLoading ? (
          Array.from({ length: 2 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Members"
              value={rows.length}
              sub="With workspace access"
              icon={Users}
            />
            <StatCard
              label="Active"
              value={activeCount}
              sub={`of ${rows.length} total`}
              icon={UserPlus}
              tone="good"
            />
          </>
        )}
      </div>

      <DataTable
        columns={columns}
        data={rows}
        isLoading={members.isLoading}
        searchPlaceholder="Search members…"
        emptyState={
          <EmptyState
            className="border-0"
            icon={Users}
            title="No members yet"
            description="Invite a teammate to give them access to this workspace under a persona."
            action={canManage ? <InviteMemberDialog /> : undefined}
          />
        }
      />
    </div>
  );
}

/** Dialog to invite a member into the current tenant under a persona. */
function InviteMemberDialog() {
  const [open, setOpen] = useState(false);
  const invite = useInviteMember();
  const { data: profileTypes } = useProfileTypes();
  const tenantTypes = (profileTypes ?? []).filter((t) => t.scope === "tenant");

  const {
    register,
    handleSubmit,
    control,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<InviteMemberInputForm>({
    resolver: zodResolver(inviteMemberSchema),
    defaultValues: { email: "", name: "", profile_type: "", title: "" },
  });

  const onSubmit = handleSubmit(async (values) => {
    await invite.mutateAsync(
      {
        email: values.email,
        name: values.name,
        profile_type: values.profile_type,
        ...(values.title ? { title: values.title } : {}),
      },
      {
        onSuccess: () => {
          reset({ email: "", name: "", profile_type: "", title: "" });
          setOpen(false);
        },
      }
    );
  });

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) reset({ email: "", name: "", profile_type: "", title: "" });
      }}
    >
      <DialogTrigger asChild>
        <Button>
          <Plus className="h-4 w-4" />
          Invite member
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>Invite member</DialogTitle>
            <DialogDescription>
              Add someone to your workspace under a persona.
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
            <Field
              label="Persona"
              required
              error={errors.profile_type?.message}
            >
              <Controller
                control={control}
                name="profile_type"
                render={({ field }) => (
                  <Select value={field.value} onValueChange={field.onChange}>
                    <SelectTrigger
                      aria-invalid={!!errors.profile_type}
                      className="h-10"
                    >
                      <SelectValue placeholder="Select a persona" />
                    </SelectTrigger>
                    <SelectContent>
                      {tenantTypes.map((t) => (
                        <SelectItem key={t.key} value={t.key}>
                          {t.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              />
            </Field>
            <TextField
              label="Title"
              placeholder="e.g. Property manager"
              hint="Optional."
              {...register("title")}
            />
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
              <Mail className="h-4 w-4" />
              {isSubmitting ? "Inviting…" : "Invite member"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
