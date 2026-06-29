"use client";

// Platform user detail (Acre staff console): identity + status, a profile tab
// with masked PII and a gated raw-PII reveal, a personas/memberships tab, and a
// roles tab. All reads go through TanStack Query hooks; all mutations are gated
// behind useAuth().can(...). Raw PII is fetched only on an explicit click and is
// dropped from memory as soon as its dialog closes.

import { useState } from "react";
import { useParams } from "next/navigation";
import {
  IdCard,
  KeyRound,
  Mail,
  Pencil,
  Plus,
  ShieldCheck,
  Trash2,
  UserCog,
  UserRound,
  Users,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import { iam } from "@/lib/api";
import type {
  MembershipInput,
  ProfileDto,
  ProfileInput,
  UserDetail,
  UserPii,
} from "@/lib/api";
import {
  useAddMembership,
  useAssignRole,
  usePutProfile,
  useProfileTypes,
  useRemoveMembership,
  useRevokeRole,
  useRoles,
  useUpdateUser,
  useUser,
} from "@/lib/queries";

import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { PageHeader, EmptyState } from "@/components/ui/page";
import { Breadcrumbs } from "@/components/ui/breadcrumbs";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Field,
  Input,
  TextField,
  SelectField,
} from "@/components/ui/form-field";
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
import { titleCase } from "@/lib/format";

const STATUSES = ["active", "invited", "suspended", "disabled"] as const;

// ---- page --------------------------------------------------------------------

/** Full user-detail page for Acre platform staff. */
export default function PlatformUserDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { can, user: me } = useAuth();
  const { data: user, error, isLoading } = useUser(id);

  const canManage = can("user:manage");

  if (!me?.is_platform_staff) {
    return (
      <Card>
        <CardContent>
          <EmptyState
            icon={ShieldCheck}
            title="Platform staff only"
            description="This page is restricted to Acre platform staff."
          />
        </CardContent>
      </Card>
    );
  }

  if (isLoading) return <UserDetailSkeleton />;

  if (error || !user) {
    return (
      <div className="space-y-6">
        <Breadcrumbs
          items={[
            { label: "Users", href: "/console/platform/users" },
            { label: "User" },
          ]}
        />
        <Card>
          <CardContent>
            <EmptyState
              icon={UserRound}
              title="Couldn't load user"
              description={error?.message ?? "User not found."}
            />
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <Breadcrumbs
        items={[
          { label: "Users", href: "/console/platform/users" },
          { label: user.name },
        ]}
      />

      <PageHeader
        eyebrow="Platform user"
        title={
          <span className="flex flex-wrap items-center gap-3">
            {user.name}
            {user.is_platform_staff && <Badge tone="info">Staff</Badge>}
            <Badge tone={statusTone(user.status)}>
              {titleCase(user.status)}
            </Badge>
          </span>
        }
        description={
          <span className="inline-flex items-center gap-1.5">
            <Mail className="h-3.5 w-3.5 text-ink-3" />
            {user.email}
            {user.username && (
              <span className="text-ink-3"> · @{user.username}</span>
            )}
          </span>
        }
        actions={
          canManage ? <EditIdentityDialog user={user} /> : undefined
        }
      />

      <Tabs defaultValue="profile">
        <TabsList>
          <TabsTrigger value="profile">Profile</TabsTrigger>
          <TabsTrigger value="memberships">
            Personas{user.memberships.length ? ` (${user.memberships.length})` : ""}
          </TabsTrigger>
          <TabsTrigger value="roles">
            Roles{user.roles.length ? ` (${user.roles.length})` : ""}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="profile">
          <ProfileTab user={user} canManage={canManage} />
        </TabsContent>
        <TabsContent value="memberships">
          <MembershipsTab user={user} canManage={canManage} />
        </TabsContent>
        <TabsContent value="roles">
          <RolesTab user={user} canManage={canManage} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

// ---- profile tab -------------------------------------------------------------

/** Masked profile view + gated PII reveal + gated edit form. */
function ProfileTab({
  user,
  canManage,
}: {
  user: UserDetail;
  canManage: boolean;
}) {
  const p = user.profile;

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Profile</CardTitle>
          <CardDescription>
            Identity and contact details. Sensitive values are masked.
          </CardDescription>
        </div>
        <div className="flex items-center gap-2">
          {canManage && p && (p.has_ssn || p.has_gov_id) && (
            <RevealPiiButton userId={user.id} />
          )}
          {canManage && <EditProfileDialog user={user} />}
        </div>
      </CardHeader>
      <CardContent>
        {!p ? (
          <EmptyState
            className="border-0"
            icon={IdCard}
            title="No profile on file"
            description={
              canManage
                ? "Add identity and contact details for this user."
                : "This user has no profile details yet."
            }
            action={canManage ? <EditProfileDialog user={user} /> : undefined}
          />
        ) : (
          <dl className="grid gap-x-8 gap-y-3 sm:grid-cols-2">
            <Row label="Legal name" value={legalName(p)} />
            <Row label="Preferred name" value={p.preferred_name} />
            <Row label="Date of birth" value={p.date_of_birth} />
            <Row label="Phone" value={p.phone} />
            <Row label="Address" value={address(p)} className="sm:col-span-2" />
            <Row
              label="SSN"
              value={p.has_ssn ? `••• •• ${p.ssn_last4 ?? "••••"}` : null}
              mono
            />
            <Row
              label="Government ID"
              value={
                p.has_gov_id
                  ? `${p.gov_id_type ? titleCase(p.gov_id_type) : "ID"} ••••${p.gov_id_last4 ?? ""}`
                  : null
              }
              mono
            />
          </dl>
        )}
      </CardContent>
    </Card>
  );
}

/** A definition-list row; renders an em-dash for empty values. */
function Row({
  label,
  value,
  mono,
  className,
}: {
  label: string;
  value: string | null | undefined;
  mono?: boolean;
  className?: string;
}) {
  return (
    <div className={className}>
      <dt className="text-xs font-semibold uppercase tracking-wide text-ink-3">
        {label}
      </dt>
      <dd
        className={
          mono
            ? "mt-0.5 font-mono text-sm font-medium text-ink"
            : "mt-0.5 text-sm font-medium text-ink"
        }
        data-numeric={mono ? "" : undefined}
      >
        {value || <span className="text-ink-3">—</span>}
      </dd>
    </div>
  );
}

/** Gated button that fetches raw PII on click and shows it transiently. */
function RevealPiiButton({ userId }: { userId: string }) {
  const [open, setOpen] = useState(false);
  const [pii, setPii] = useState<UserPii | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  /** Fetch PII only when the dialog is explicitly opened. */
  async function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      // Drop sensitive values from memory as soon as the dialog closes.
      setPii(null);
      setErr(null);
      return;
    }
    setLoading(true);
    setErr(null);
    try {
      setPii(await iam.pii(userId));
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          <KeyRound className="h-4 w-4" />
          Reveal PII
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Sensitive identifiers</DialogTitle>
          <DialogDescription>
            Shown only while this dialog is open. Closing it clears the values
            from memory. This reveal is recorded in the audit log.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          {loading && (
            <>
              <div className="skeleton h-10 rounded-lg" />
              <div className="skeleton h-10 rounded-lg" />
            </>
          )}
          {err && <p className="text-sm text-bad">{err}</p>}
          {pii && (
            <dl className="space-y-3">
              <Row label="SSN" value={pii.ssn} mono />
              <Row label="Government ID number" value={pii.gov_id_number} mono />
            </dl>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Edit-profile dialog (PUT). Sensitive fields are write-only. */
function EditProfileDialog({ user }: { user: UserDetail }) {
  const [open, setOpen] = useState(false);
  const putProfile = usePutProfile(user.id);
  const p = user.profile;

  const [form, setForm] = useState(() => initialProfileForm(p));

  function set<K extends keyof typeof form>(key: K, value: string) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  function onOpenChange(next: boolean) {
    setOpen(next);
    if (next) setForm(initialProfileForm(p));
  }

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    // Only forward non-empty fields so blanks don't clobber existing data.
    const body: ProfileInput = {};
    for (const [key, val] of Object.entries(form)) {
      const trimmed = val.trim();
      if (trimmed) (body as Record<string, string>)[key] = trimmed;
    }
    await putProfile.mutateAsync(body, {
      onSuccess: () => setOpen(false),
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          <Pencil className="h-4 w-4" />
          {p ? "Edit profile" : "Add profile"}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>{p ? "Edit profile" : "Add profile"}</DialogTitle>
            <DialogDescription>
              Update identity and contact details. Sensitive values are
              write-only — leave them blank to keep the existing value.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <TextField
                label="Legal first"
                value={form.legal_first_name}
                onChange={(e) => set("legal_first_name", e.target.value)}
              />
              <TextField
                label="Legal middle"
                value={form.legal_middle_name}
                onChange={(e) => set("legal_middle_name", e.target.value)}
              />
              <TextField
                label="Legal last"
                value={form.legal_last_name}
                onChange={(e) => set("legal_last_name", e.target.value)}
              />
            </div>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <TextField
                label="Preferred name"
                value={form.preferred_name}
                onChange={(e) => set("preferred_name", e.target.value)}
              />
              <TextField
                label="Date of birth"
                placeholder="1990-01-31"
                hint="YYYY-MM-DD"
                value={form.date_of_birth}
                onChange={(e) => set("date_of_birth", e.target.value)}
              />
            </div>
            <TextField
              label="Phone"
              value={form.phone}
              onChange={(e) => set("phone", e.target.value)}
            />
            <TextField
              label="Address line 1"
              value={form.address_line1}
              onChange={(e) => set("address_line1", e.target.value)}
            />
            <TextField
              label="Address line 2"
              value={form.address_line2}
              onChange={(e) => set("address_line2", e.target.value)}
            />
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <TextField
                label="City"
                value={form.city}
                onChange={(e) => set("city", e.target.value)}
              />
              <TextField
                label="Region / state"
                value={form.region}
                onChange={(e) => set("region", e.target.value)}
              />
            </div>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <TextField
                label="Postal code"
                value={form.postal_code}
                onChange={(e) => set("postal_code", e.target.value)}
              />
              <TextField
                label="Country"
                value={form.country}
                onChange={(e) => set("country", e.target.value)}
              />
            </div>

            <div className="space-y-4 border-t border-line pt-4">
              <p className="text-xs font-semibold uppercase tracking-wide text-ink-3">
                Sensitive (write-only)
              </p>
              <TextField
                label="SSN"
                placeholder="Leave blank to keep"
                value={form.ssn}
                onChange={(e) => set("ssn", e.target.value)}
              />
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <TextField
                  label="Gov ID type"
                  placeholder="passport"
                  value={form.gov_id_type}
                  onChange={(e) => set("gov_id_type", e.target.value)}
                />
                <TextField
                  label="Gov ID number"
                  placeholder="Leave blank to keep"
                  value={form.gov_id_number}
                  onChange={(e) => set("gov_id_number", e.target.value)}
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
            <Button type="submit" disabled={putProfile.isPending}>
              {putProfile.isPending ? "Saving…" : "Save profile"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ---- memberships tab ---------------------------------------------------------

/** Personas / memberships list with gated add + remove. */
function MembershipsTab({
  user,
  canManage,
}: {
  user: UserDetail;
  canManage: boolean;
}) {
  const remove = useRemoveMembership(user.id);

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Personas &amp; memberships</CardTitle>
          <CardDescription>
            The scopes and personas this user can act under.
          </CardDescription>
        </div>
        {canManage && <AddMembershipDialog user={user} />}
      </CardHeader>
      <CardContent>
        {user.memberships.length === 0 ? (
          <EmptyState
            className="border-0"
            icon={Users}
            title="No memberships"
            description="This user has no personas yet."
            action={canManage ? <AddMembershipDialog user={user} /> : undefined}
          />
        ) : (
          <ul className="space-y-2.5">
            {user.memberships.map((m) => (
              <li
                key={m.id}
                className="flex items-center gap-3 rounded-xl border border-line px-3.5 py-3"
              >
                <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-ink-3">
                  <UserRound className="h-4 w-4" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium text-ink">
                    {titleCase(m.profile_type)}
                    {m.title && (
                      <span className="font-normal text-ink-3">
                        {" "}
                        · {m.title}
                      </span>
                    )}
                  </div>
                  <div className="truncate text-xs text-ink-3">
                    {titleCase(m.scope)}
                    {m.tenant_id && <> · {m.tenant_id}</>}
                    {m.is_primary && <> · primary</>}
                  </div>
                </div>
                <Badge tone={statusTone(m.status)}>{titleCase(m.status)}</Badge>
                {canManage && (
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label="Remove persona"
                    disabled={remove.isPending}
                    onClick={() => remove.mutate(m.id)}
                  >
                    <Trash2 className="h-4 w-4 text-bad" />
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

/** Dialog to add a persona membership to the user. */
function AddMembershipDialog({ user }: { user: UserDetail }) {
  const [open, setOpen] = useState(false);
  const add = useAddMembership(user.id);
  const { data: profileTypes } = useProfileTypes();

  const [scope, setScope] = useState<"tenant" | "platform">("tenant");
  const [profileType, setProfileType] = useState("");
  const [tenantId, setTenantId] = useState("");
  const [title, setTitle] = useState("");

  const scopedTypes = (profileTypes ?? []).filter((t) => t.scope === scope);

  function reset() {
    setScope("tenant");
    setProfileType("");
    setTenantId("");
    setTitle("");
  }

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!profileType) return;
    const body: MembershipInput = { scope, profile_type: profileType };
    if (scope === "tenant" && tenantId.trim()) body.tenant_id = tenantId.trim();
    if (title.trim()) body.title = title.trim();
    add.mutate(body, {
      onSuccess: () => {
        setOpen(false);
        reset();
      },
    });
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus className="h-4 w-4" />
          Add persona
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>Add persona</DialogTitle>
            <DialogDescription>
              Grant {user.name} a membership under a persona.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <Field label="Scope">
              <Select
                value={scope}
                onValueChange={(v) => {
                  setScope(v as "tenant" | "platform");
                  setProfileType("");
                }}
              >
                <SelectTrigger className="h-10">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="tenant">Tenant</SelectItem>
                  <SelectItem value="platform">Platform</SelectItem>
                </SelectContent>
              </Select>
            </Field>

            <Field label="Persona" required>
              <Select value={profileType} onValueChange={setProfileType}>
                <SelectTrigger className="h-10">
                  <SelectValue placeholder="Select a persona" />
                </SelectTrigger>
                <SelectContent>
                  {scopedTypes.map((t) => (
                    <SelectItem key={t.key} value={t.key}>
                      {t.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>

            {scope === "tenant" && (
              <Field label="Tenant ID" hint="Optional.">
                <Input
                  value={tenantId}
                  placeholder="tenant uuid"
                  onChange={(e) => setTenantId(e.target.value)}
                />
              </Field>
            )}

            <TextField
              label="Title"
              hint="Optional."
              value={title}
              onChange={(e) => setTitle(e.target.value)}
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
            <Button type="submit" disabled={add.isPending || !profileType}>
              {add.isPending ? "Adding…" : "Add persona"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ---- roles tab ---------------------------------------------------------------

/** Role-assignments list with gated assign + revoke. */
function RolesTab({
  user,
  canManage,
}: {
  user: UserDetail;
  canManage: boolean;
}) {
  const revoke = useRevokeRole(user.id);
  const canAssign = useAuth().can("role:manage") || canManage;

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Roles</CardTitle>
          <CardDescription>
            Roles granted to this user across scopes.
          </CardDescription>
        </div>
        {canAssign && <AssignRoleDialog user={user} />}
      </CardHeader>
      <CardContent>
        {user.roles.length === 0 ? (
          <EmptyState
            className="border-0"
            icon={ShieldCheck}
            title="No roles assigned"
            description="This user has no roles yet."
            action={canAssign ? <AssignRoleDialog user={user} /> : undefined}
          />
        ) : (
          <ul className="space-y-2.5">
            {user.roles.map((r) => (
              <li
                key={r.id}
                className="flex items-center gap-3 rounded-xl border border-line px-3.5 py-3"
              >
                <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-ink-3">
                  <ShieldCheck className="h-4 w-4" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium text-ink">
                    {r.role_name}
                  </div>
                  <code className="text-xs text-ink-3">{r.role_key}</code>
                </div>
                {r.tenant_id && <Badge tone="neutral">Tenant</Badge>}
                {canAssign && (
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label="Revoke role"
                    disabled={revoke.isPending}
                    onClick={() => revoke.mutate(r.id)}
                  >
                    <Trash2 className="h-4 w-4 text-bad" />
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

/** Dialog to assign one of the available roles to the user. */
function AssignRoleDialog({ user }: { user: UserDetail }) {
  const [open, setOpen] = useState(false);
  const assign = useAssignRole(user.id);
  const { data: roles } = useRoles();
  const [roleId, setRoleId] = useState("");

  const assignedIds = new Set(user.roles.map((r) => r.role_id));
  const available = (roles ?? []).filter((r) => !assignedIds.has(r.id));

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!roleId) return;
    const role = available.find((r) => r.id === roleId);
    assign.mutate(
      {
        role_id: roleId,
        ...(role?.tenant_id ? { tenant_id: role.tenant_id } : {}),
      },
      {
        onSuccess: () => {
          setOpen(false);
          setRoleId("");
        },
      }
    );
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) setRoleId("");
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus className="h-4 w-4" />
          Assign role
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>Assign role</DialogTitle>
            <DialogDescription>
              Grant {user.name} an additional role.
            </DialogDescription>
          </DialogHeader>

          <Field label="Role" required>
            <Select value={roleId} onValueChange={setRoleId}>
              <SelectTrigger className="h-10">
                <SelectValue placeholder="Select a role" />
              </SelectTrigger>
              <SelectContent>
                {available.map((r) => (
                  <SelectItem key={r.id} value={r.id}>
                    {r.name} ({r.scope})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>

          {available.length === 0 && (
            <p className="text-sm text-ink-3">
              All available roles are already assigned.
            </p>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setOpen(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={assign.isPending || !roleId}>
              {assign.isPending ? "Assigning…" : "Assign role"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ---- identity edit -----------------------------------------------------------

/** Dialog to edit the user's identity fields (name / username / status). */
function EditIdentityDialog({ user }: { user: UserDetail }) {
  const [open, setOpen] = useState(false);
  const update = useUpdateUser(user.id);

  const [name, setName] = useState(user.name);
  const [username, setUsername] = useState(user.username ?? "");
  const [status, setStatus] = useState(user.status);

  function reset() {
    setName(user.name);
    setUsername(user.username ?? "");
    setStatus(user.status);
  }

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    update.mutate(
      {
        name: name.trim(),
        username: username.trim() || undefined,
        status,
      },
      { onSuccess: () => setOpen(false) }
    );
  }

  const statusOptions = STATUSES.includes(
    user.status as (typeof STATUSES)[number]
  )
    ? STATUSES
    : ([...STATUSES, user.status] as readonly string[]);

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (next) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button variant="outline">
          <UserCog className="h-4 w-4" />
          Edit identity
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>Edit identity</DialogTitle>
            <DialogDescription>
              Update {user.name}&apos;s display name, username, and account
              status.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <TextField
              label="Full name"
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <TextField
              label="Username"
              hint="Optional."
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
            <SelectField
              label="Status"
              value={status}
              onChange={(e) => setStatus(e.target.value)}
            >
              {statusOptions.map((s) => (
                <option key={s} value={s}>
                  {titleCase(s)}
                </option>
              ))}
            </SelectField>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setOpen(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={update.isPending || !name.trim()}>
              {update.isPending ? "Saving…" : "Save changes"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ---- helpers -----------------------------------------------------------------

type ProfileForm = {
  legal_first_name: string;
  legal_middle_name: string;
  legal_last_name: string;
  preferred_name: string;
  date_of_birth: string;
  phone: string;
  address_line1: string;
  address_line2: string;
  city: string;
  region: string;
  postal_code: string;
  country: string;
  gov_id_type: string;
  ssn: string;
  gov_id_number: string;
};

/** Seed the edit form from the masked profile (sensitive fields stay blank). */
function initialProfileForm(p: ProfileDto | null): ProfileForm {
  return {
    legal_first_name: p?.legal_first_name ?? "",
    legal_middle_name: p?.legal_middle_name ?? "",
    legal_last_name: p?.legal_last_name ?? "",
    preferred_name: p?.preferred_name ?? "",
    date_of_birth: p?.date_of_birth ?? "",
    phone: p?.phone ?? "",
    address_line1: p?.address_line1 ?? "",
    address_line2: p?.address_line2 ?? "",
    city: p?.city ?? "",
    region: p?.region ?? "",
    postal_code: p?.postal_code ?? "",
    country: p?.country ?? "",
    gov_id_type: p?.gov_id_type ?? "",
    ssn: "",
    gov_id_number: "",
  };
}

/** Assemble a legal name string from profile parts. */
function legalName(p: ProfileDto): string {
  return (
    [p.legal_first_name, p.legal_middle_name, p.legal_last_name]
      .filter(Boolean)
      .join(" ") || ""
  );
}

/** Assemble a single-line address from profile parts. */
function address(p: ProfileDto): string {
  return (
    [
      p.address_line1,
      p.address_line2,
      p.city,
      p.region,
      p.postal_code,
      p.country,
    ]
      .filter(Boolean)
      .join(", ") || ""
  );
}

/** Loading placeholder mirroring the page layout. */
function UserDetailSkeleton() {
  return (
    <div className="space-y-6">
      <div className="skeleton h-4 w-40 rounded" />
      <div className="space-y-2">
        <div className="skeleton h-8 w-64 rounded-lg" />
        <div className="skeleton h-4 w-48 rounded" />
      </div>
      <div className="skeleton h-9 w-72 rounded-lg" />
      <div className="skeleton h-64 rounded-xl" />
    </div>
  );
}
