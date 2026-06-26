"use client";

// Client-admin member directory: people in the current tenant workspace, with
// an "Invite member" dialog. Tenant is implied by the current JWT.

import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";

import { useAuth } from "@/lib/auth";
import { useInviteMember, useMembers, useProfileTypes } from "@/lib/queries";
import { inviteMemberSchema, type InviteMemberInputForm } from "@/lib/schemas";
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

/** Tenant-scoped member directory for client admins. */
export default function MembersPage() {
  const { can } = useAuth();
  const { data: members, error, isLoading } = useMembers();
  const canManage = can("member:manage");

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Members
          </h1>
          <p className="text-ink-3">People with access to your workspace.</p>
        </div>
        {canManage && <InviteMemberDialog />}
      </div>

      {error && <p className="text-bad">{error.message}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.4fr_1.4fr_.9fr_.5fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Name</span>
          <span>Email</span>
          <span>Persona</span>
          <span className="text-right">Status</span>
        </div>
        <div className="divide-y divide-line">
          {members?.map((m) => (
            <div
              key={m.membership_id}
              className="grid grid-cols-[1.4fr_1.4fr_.9fr_.5fr] items-center gap-4 px-5 py-3.5"
            >
              <div className="min-w-0 truncate font-semibold">{m.name}</div>
              <span className="truncate text-sm text-ink-2">{m.email}</span>
              <span className="text-sm text-ink-2">
                {m.profile_type}
                {m.title && <span className="text-ink-3"> · {m.title}</span>}
              </span>
              <span className="flex justify-end">
                <Badge tone={statusTone(m.status)}>{m.status}</Badge>
              </span>
            </div>
          ))}
          {isLoading && (
            <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
          )}
          {members && members.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No members yet.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

/** Dialog to invite a member into the current tenant. */
function InviteMemberDialog() {
  const [open, setOpen] = useState(false);
  const invite = useInviteMember();
  const { data: profileTypes } = useProfileTypes();
  const tenantTypes = (profileTypes ?? []).filter((t) => t.scope === "tenant");

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<InviteMemberInputForm>({
    resolver: zodResolver(inviteMemberSchema),
    defaultValues: { email: "", name: "", profile_type: "" },
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
          reset({ email: "", name: "", profile_type: "" });
          setOpen(false);
        },
      }
    );
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>Invite member</Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>Invite member</DialogTitle>
            <DialogDescription>
              Add someone to your workspace under a persona.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="space-y-1.5">
              <Label>Email</Label>
              <Input
                type="email"
                placeholder="person@example.com"
                aria-invalid={!!errors.email}
                {...register("email")}
              />
              {errors.email && (
                <p className="text-sm text-bad" role="alert">
                  {errors.email.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Full name</Label>
              <Input
                placeholder="Jane Doe"
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
              <Label>Persona</Label>
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                aria-invalid={!!errors.profile_type}
                {...register("profile_type")}
              >
                <option value="">— Select —</option>
                {tenantTypes.map((t) => (
                  <option key={t.key} value={t.key}>
                    {t.label}
                  </option>
                ))}
              </select>
              {errors.profile_type && (
                <p className="text-sm text-bad" role="alert">
                  {errors.profile_type.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Title (optional)</Label>
              <Input
                placeholder="e.g. Property manager"
                {...register("title")}
              />
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
              {isSubmitting ? "Inviting…" : "Invite member"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
