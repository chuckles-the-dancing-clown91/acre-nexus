"use client";

// Platform-admin: Roles & permissions management. List system + custom roles,
// create custom roles, and edit a role's granted permissions through a matrix
// grouped by permission category. System roles are read-only (locked); only
// non-system roles can be edited or deleted. Gated by `role:manage`.

import { useMemo, useState } from "react";
import { KeyRound, Plus, Shield, ShieldCheck, Trash2 } from "lucide-react";

import { useAuth } from "@/lib/auth";
import type {
  CreateRoleInput,
  PermissionDef,
  Role,
  UpdateRoleInput,
} from "@/lib/api";
import {
  useCreateRole,
  useDeleteRole,
  usePermissionsCatalog,
  useRoles,
  useUpdateRole,
} from "@/lib/queries";
import { titleCase } from "@/lib/format";
import { cn } from "@/lib/utils";

import { Badge } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { TextField, TextareaField, SelectField } from "@/components/ui/form-field";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** Scope → Badge tone. */
function scopeTone(scope: string) {
  return scope === "platform" ? ("info" as const) : ("accent" as const);
}

/** Group a flat permission catalog into ordered { category, permissions[] }. */
function groupByCategory(catalog: PermissionDef[]) {
  const map = new Map<string, PermissionDef[]>();
  for (const p of catalog) {
    const list = map.get(p.category);
    if (list) list.push(p);
    else map.set(p.category, [p]);
  }
  return [...map.entries()].map(([category, permissions]) => ({
    category,
    permissions,
  }));
}

/** Roles directory + permission-matrix editor for platform staff. */
export default function RolesPage() {
  const { can, user } = useAuth();
  const roles = useRoles();
  const canManage = can("role:manage");
  const [selected, setSelected] = useState<Role | null>(null);
  const [createOpen, setCreateOpen] = useState(false);

  const rows = roles.data ?? [];
  const systemCount = rows.filter((r) => r.is_system).length;
  const customCount = rows.length - systemCount;

  const columns = useMemo<ColumnDef<Role>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Role",
        cell: ({ row }) => {
          const r = row.original;
          return (
            <div className="flex items-center gap-3 min-w-0">
              <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent-2">
                {r.is_system ? (
                  <ShieldCheck className="h-4 w-4" />
                ) : (
                  <Shield className="h-4 w-4" />
                )}
              </span>
              <div className="min-w-0">
                <div className="truncate font-medium text-ink">{r.name}</div>
                <code className="truncate text-xs text-ink-3">{r.key}</code>
              </div>
            </div>
          );
        },
      },
      {
        accessorKey: "scope",
        header: "Scope",
        cell: ({ row }) => (
          <Badge tone={scopeTone(row.original.scope)}>
            {titleCase(row.original.scope)}
          </Badge>
        ),
      },
      {
        accessorKey: "is_system",
        header: "Type",
        cell: ({ row }) =>
          row.original.is_system ? (
            <Badge tone="info">System</Badge>
          ) : (
            <Badge tone="neutral">Custom</Badge>
          ),
      },
      {
        id: "permissions",
        accessorFn: (r) => r.permissions.length,
        header: () => <div className="text-right">Permissions</div>,
        cell: ({ row }) => (
          <div data-numeric className="text-right font-medium text-ink-2">
            {row.original.permissions.length}
          </div>
        ),
      },
    ],
    []
  );

  if (!user?.is_platform_staff) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Platform"
          title="Roles & permissions"
          description="Named permission bundles for platform and tenant access."
        />
        <EmptyState
          icon={ShieldCheck}
          title="Staff only"
          description="Roles administration is available to Acre platform staff."
        />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Platform"
        title="Roles & permissions"
        description="Named permission bundles. System roles are locked; create custom roles for fine-grained access."
        actions={
          canManage ? (
            <Button onClick={() => setCreateOpen(true)}>
              <Plus className="h-4 w-4" />
              New role
            </Button>
          ) : undefined
        }
      />

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
        {roles.isLoading ? (
          Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Roles"
              value={rows.length}
              sub="Across all scopes"
              icon={KeyRound}
            />
            <StatCard
              label="System"
              value={systemCount}
              sub="Built-in & locked"
              icon={ShieldCheck}
            />
            <StatCard
              label="Custom"
              value={customCount}
              sub="Editable bundles"
              icon={Shield}
              tone="accent"
            />
          </>
        )}
      </div>

      <DataTable<Role>
        columns={columns}
        data={rows}
        isLoading={roles.isLoading}
        searchPlaceholder="Search roles…"
        onRowClick={(r) => setSelected(r)}
        emptyState={
          <EmptyState
            className="border-0"
            icon={KeyRound}
            title="No roles yet"
            description="Create a custom role to bundle permissions for fine-grained access."
            action={
              canManage ? (
                <Button onClick={() => setCreateOpen(true)}>New role</Button>
              ) : undefined
            }
          />
        }
      />

      {canManage && (
        <NewRoleDialog open={createOpen} onOpenChange={setCreateOpen} />
      )}

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

/** Dialog to create a new custom role (permissions added afterwards). */
function NewRoleDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const create = useCreateRole();
  const [scope, setScope] = useState("tenant");
  const [tenantId, setTenantId] = useState("");
  const [key, setKey] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);

  function reset() {
    setScope("tenant");
    setTenantId("");
    setKey("");
    setName("");
    setDescription("");
    setError(null);
  }

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!key.trim() || !name.trim()) {
      setError("Key and name are required.");
      return;
    }
    setError(null);
    const body: CreateRoleInput = {
      scope,
      key: key.trim(),
      name: name.trim(),
      description: description.trim(),
      permissions: [],
    };
    if (scope === "tenant" && tenantId.trim()) body.tenant_id = tenantId.trim();
    try {
      await create.mutateAsync(body);
      reset();
      onOpenChange(false);
    } catch {
      // toast handled by the mutation hook
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        onOpenChange(next);
        if (!next) reset();
      }}
    >
      <DialogContent>
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>New role</DialogTitle>
            <DialogDescription>
              Create a custom role. Add its permissions after it&apos;s created.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <SelectField
              label="Scope"
              value={scope}
              onChange={(e) => setScope(e.target.value)}
            >
              <option value="tenant">Tenant</option>
              <option value="platform">Platform</option>
            </SelectField>
            {scope === "tenant" && (
              <TextField
                label="Tenant ID"
                hint="Optional — leave blank for a template role."
                placeholder="tenant uuid"
                value={tenantId}
                onChange={(e) => setTenantId(e.target.value)}
              />
            )}
            <TextField
              label="Key"
              required
              placeholder="e.g. regional_manager"
              value={key}
              onChange={(e) => setKey(e.target.value)}
            />
            <TextField
              label="Name"
              required
              placeholder="Regional manager"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <TextareaField
              label="Description"
              hint="Optional."
              placeholder="What this role is for."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
            {error && <p className="text-xs text-bad">{error}</p>}
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={create.isPending}>
              {create.isPending ? "Creating…" : "Create role"}
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
  const catalog = usePermissionsCatalog();
  const update = useUpdateRole();
  const remove = useDeleteRole();
  const [granted, setGranted] = useState<Set<string>>(
    () => new Set(role.permissions)
  );
  const locked = role.is_system || !canManage;
  const groups = useMemo(
    () => groupByCategory(catalog.data ?? []),
    [catalog.data]
  );

  function toggle(key: string) {
    if (locked) return;
    setGranted((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  async function save() {
    const body: UpdateRoleInput = { permissions: [...granted] };
    try {
      await update.mutateAsync({ id: role.id, body });
      onClose();
    } catch {
      // toast handled by the mutation hook
    }
  }

  async function del() {
    try {
      await remove.mutateAsync(role.id);
      onClose();
    } catch {
      // toast handled by the mutation hook
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[88vh] max-w-2xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex flex-wrap items-center gap-2">
            {role.is_system ? (
              <ShieldCheck className="h-4 w-4 text-ink-3" />
            ) : (
              <Shield className="h-4 w-4 text-ink-3" />
            )}
            {role.name}
            <Badge tone={scopeTone(role.scope)}>{titleCase(role.scope)}</Badge>
            {role.is_system ? (
              <Badge tone="info">System · locked</Badge>
            ) : (
              <Badge tone="neutral">Custom</Badge>
            )}
          </DialogTitle>
          <DialogDescription>
            {role.description || "Toggle the permissions this role grants."}
            {locked && role.is_system && " System roles can't be edited."}
          </DialogDescription>
        </DialogHeader>

        <div className="my-2 space-y-5">
          {catalog.isLoading && (
            <p className="text-sm text-ink-3">Loading permissions…</p>
          )}
          {!catalog.isLoading && groups.length === 0 && (
            <p className="text-sm text-ink-3">No permissions available.</p>
          )}
          {groups.map((g) => (
            <div key={g.category}>
              <h3 className="mb-2 text-xs font-bold uppercase tracking-wide text-ink-3">
                {g.category}
              </h3>
              <div className="space-y-1.5">
                {g.permissions.map((perm) => {
                  const on = granted.has(perm.key);
                  return (
                    <label
                      key={perm.key}
                      className={cn(
                        "flex items-start gap-3 rounded-lg border px-3 py-2.5 transition",
                        on
                          ? "border-accent bg-accent-soft"
                          : "border-line bg-surface",
                        locked
                          ? "cursor-not-allowed opacity-80"
                          : "cursor-pointer hover:border-line-2"
                      )}
                    >
                      <input
                        type="checkbox"
                        className="mt-1 accent-accent"
                        checked={on}
                        disabled={locked}
                        onChange={() => toggle(perm.key)}
                      />
                      <div className="min-w-0">
                        <div className="font-medium text-ink">{perm.label}</div>
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
                variant="destructive"
                onClick={del}
                disabled={remove.isPending}
              >
                <Trash2 className="h-4 w-4" />
                {remove.isPending ? "Deleting…" : "Delete role"}
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
