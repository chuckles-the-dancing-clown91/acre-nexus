"use client";

// Roles administration: list system + custom roles, create custom roles, and
// edit a role's permissions through a category-grouped matrix. System roles are
// read-only (locked).

import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";

import { useAuth } from "@/lib/auth";
import type { CreateRoleInput, Role } from "@/lib/api";
import { groupPermissions } from "@/lib/iam";
import {
  useCreateRole,
  useDeleteRole,
  usePermissionsCatalog,
  useRoles,
  useUpdateRole,
} from "@/lib/queries";
import { createRoleSchema, type CreateRoleInputForm } from "@/lib/schemas";
import { Badge, Card } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Icon } from "@/components/Icon";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

/** Roles directory + permission-matrix editor for platform staff. */
export default function RolesPage() {
  const { can, user } = useAuth();
  const { data: roles, error, isLoading } = useRoles();
  const canManage = can("role:manage");
  const [selected, setSelected] = useState<Role | null>(null);

  if (!user?.is_platform_staff) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">Roles administration is staff only.</p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Roles
          </h1>
          <p className="text-ink-3">
            Named permission bundles. System roles are locked; create custom
            roles for fine-grained access.
          </p>
        </div>
        {canManage && <NewRoleDialog />}
      </div>

      {error && <p className="text-bad">{error.message}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.4fr_.8fr_.7fr_.5fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Role</span>
          <span>Scope</span>
          <span>Type</span>
          <span className="text-right">Perms</span>
        </div>
        <div className="divide-y divide-line">
          {roles?.map((r) => (
            <button
              key={r.id}
              onClick={() => setSelected(r)}
              className="grid w-full grid-cols-[1.4fr_.8fr_.7fr_.5fr] items-center gap-4 px-5 py-3.5 text-left hover:bg-surface-2"
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2 truncate font-semibold">
                  {r.is_system && (
                    <Icon name="key" size={14} className="text-ink-3" />
                  )}
                  {r.name}
                </div>
                <code className="text-xs text-ink-3">{r.key}</code>
              </div>
              <span className="text-sm text-ink-2">{r.scope}</span>
              <span>
                {r.is_system ? (
                  <Badge tone="info">System</Badge>
                ) : (
                  <Badge tone="accent">Custom</Badge>
                )}
              </span>
              <span className="text-right font-mono text-sm text-ink-2">
                {r.permissions.length}
              </span>
            </button>
          ))}
          {isLoading && (
            <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
          )}
          {roles && roles.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No roles yet.
            </div>
          )}
        </div>
      </Card>

      {selected && (
        <RoleEditorDialog
          role={selected}
          canManage={canManage}
          onClose={() => setSelected(null)}
        />
      )}
    </div>
  );
}

/** Dialog to create a new custom role. */
function NewRoleDialog() {
  const [open, setOpen] = useState(false);
  const create = useCreateRole();

  const {
    register,
    handleSubmit,
    watch,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateRoleInputForm>({
    resolver: zodResolver(createRoleSchema),
    defaultValues: { scope: "tenant", key: "", name: "" },
  });

  const scope = watch("scope");

  const onSubmit = handleSubmit(async (values) => {
    const body: CreateRoleInput = {
      scope: values.scope,
      key: values.key,
      name: values.name,
      description: values.description ?? "",
      permissions: [],
    };
    if (values.scope === "tenant" && values.tenant_id)
      body.tenant_id = values.tenant_id;
    await create.mutateAsync(body, {
      onSuccess: () => {
        reset({ scope: "tenant", key: "", name: "" });
        setOpen(false);
      },
    });
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>New role</Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>New role</DialogTitle>
            <DialogDescription>
              Create a custom role. Add permissions after it&apos;s created.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="space-y-1.5">
              <Label>Scope</Label>
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                {...register("scope")}
              >
                <option value="tenant">Tenant</option>
                <option value="platform">Platform</option>
              </select>
            </div>
            {scope === "tenant" && (
              <div className="space-y-1.5">
                <Label>Tenant ID (optional)</Label>
                <Input placeholder="tenant uuid" {...register("tenant_id")} />
              </div>
            )}
            <div className="space-y-1.5">
              <Label>Key</Label>
              <Input
                placeholder="e.g. regional_manager"
                aria-invalid={!!errors.key}
                {...register("key")}
              />
              {errors.key && (
                <p className="text-sm text-bad" role="alert">
                  {errors.key.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Name</Label>
              <Input
                placeholder="Regional manager"
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
              <Label>Description (optional)</Label>
              <Input {...register("description")} />
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
              {isSubmitting ? "Creating…" : "Create role"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Editor dialog: permission matrix grouped by category. */
function RoleEditorDialog({
  role,
  canManage,
  onClose,
}: {
  role: Role;
  canManage: boolean;
  onClose: () => void;
}) {
  const { data: permissions } = usePermissionsCatalog();
  const update = useUpdateRole();
  const remove = useDeleteRole();
  const [selected, setSelected] = useState<Set<string>>(
    new Set(role.permissions)
  );
  const locked = role.is_system || !canManage;
  const groups = groupPermissions(permissions ?? []);

  /** Toggle a permission key in the working selection. */
  function toggle(key: string) {
    if (locked) return;
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  function save() {
    update.mutate(
      { id: role.id, body: { permissions: [...selected] } },
      { onSuccess: onClose }
    );
  }

  function del() {
    remove.mutate(role.id, { onSuccess: onClose });
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] max-w-2xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {role.is_system && (
              <Icon name="key" size={16} className="text-ink-3" />
            )}
            {role.name}
            {role.is_system ? (
              <Badge tone="info">System · locked</Badge>
            ) : (
              <Badge tone="accent">Custom</Badge>
            )}
          </DialogTitle>
          <DialogDescription>
            {role.description || "Toggle the permissions this role grants."}
            {locked && role.is_system && " System roles can't be edited."}
          </DialogDescription>
        </DialogHeader>

        <div className="my-4 space-y-5">
          {groups.length === 0 && (
            <p className="text-ink-3">Loading permissions…</p>
          )}
          {groups.map((g) => (
            <div key={g.category}>
              <h3 className="mb-2 text-xs font-bold uppercase tracking-wide text-ink-3">
                {g.category}
              </h3>
              <div className="space-y-1.5">
                {g.permissions.map((perm) => {
                  const on = selected.has(perm.key);
                  return (
                    <label
                      key={perm.key}
                      className={`flex cursor-pointer items-start gap-3 rounded-xl border px-3 py-2.5 ${
                        on ? "border-accent bg-accent-soft" : "border-line"
                      } ${locked ? "cursor-not-allowed opacity-80" : ""}`}
                    >
                      <input
                        type="checkbox"
                        className="mt-1"
                        checked={on}
                        disabled={locked}
                        onChange={() => toggle(perm.key)}
                      />
                      <div className="min-w-0">
                        <div className="font-semibold">{perm.label}</div>
                        <code className="text-xs text-ink-3">{perm.key}</code>
                        {perm.description && (
                          <p className="text-sm text-ink-3">
                            {perm.description}
                          </p>
                        )}
                      </div>
                    </label>
                  );
                })}
              </div>
            </div>
          ))}
        </div>

        <DialogFooter className="sm:justify-between">
          <div>
            {!role.is_system && canManage && (
              <Button
                type="button"
                variant="outline"
                className="border-bad-soft text-bad"
                onClick={del}
                disabled={remove.isPending}
              >
                Delete role
              </Button>
            )}
          </div>
          <div className="flex gap-2">
            <Button type="button" variant="outline" onClick={onClose}>
              Close
            </Button>
            {!locked && (
              <Button type="button" onClick={save} disabled={update.isPending}>
                {update.isPending ? "Saving…" : "Save permissions"}
              </Button>
            )}
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
