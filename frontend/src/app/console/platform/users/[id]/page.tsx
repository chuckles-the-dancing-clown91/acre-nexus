"use client";

// Platform user detail: identity + status, profile (with gated PII reveal),
// memberships, and role assignments. All mutations go through TanStack Query
// hooks; raw PII is only fetched on an explicit click and never stored beyond
// the open reveal dialog.

import { useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";

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
  useRemoveMembership,
  useRevokeRole,
  useRoles,
  useProfileTypes,
  useUpdateUser,
  useUser,
} from "@/lib/queries";
import { profileFormSchema, type ProfileFormInput } from "@/lib/schemas";
import { Badge, Card, statusTone } from "@/components/ui";
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

const STATUSES = ["active", "invited", "suspended", "disabled"] as const;

/** Full user detail page for platform staff. */
export default function UserDetailPage() {
  const params = useParams<{ id: string }>();
  const { can, user: me } = useAuth();
  const { data: user, error, isLoading } = useUser(params.id);
  const canManage = can("user:manage");

  if (!me?.is_platform_staff) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">This page is platform-staff only.</p>
      </Card>
    );
  }
  if (error)
    return <p className="text-bad">Couldn&apos;t load user: {error.message}</p>;
  if (isLoading || !user) return <p className="text-ink-3">Loading…</p>;

  return (
    <div className="space-y-6">
      <Link
        href="/console/platform/users"
        className="inline-flex items-center gap-2 text-sm font-semibold text-ink-2"
      >
        <Icon name="back" size={16} /> All users
      </Link>

      <IdentityCard user={user} canManage={canManage} />

      <div className="grid gap-6 lg:grid-cols-[1.3fr_1fr]">
        <ProfileSection
          user={user}
          canEdit={can("profile:write")}
          canReveal={can("profile:read_pii")}
        />
        <div className="space-y-6">
          <MembershipsSection user={user} canManage={canManage} />
          <RolesSection
            user={user}
            canManage={can("role:manage") || canManage}
          />
        </div>
      </div>
    </div>
  );
}

/** Identity header card with an inline status editor. */
function IdentityCard({
  user,
  canManage,
}: {
  user: UserDetail;
  canManage: boolean;
}) {
  const update = useUpdateUser(user.id);

  return (
    <Card className="p-5">
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          {user.name}
        </h1>
        {user.is_platform_staff && <Badge tone="info">Staff</Badge>}
        <Badge tone={statusTone(user.status)}>{user.status}</Badge>
      </div>
      <p className="mt-1 text-ink-3">
        {user.email}
        {user.username && ` · @${user.username}`}
      </p>

      {canManage && (
        <div className="mt-4 flex items-center gap-2">
          <Label htmlFor="status-select">Status</Label>
          <select
            id="status-select"
            className="h-9 rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
            value={user.status}
            onChange={(e) => update.mutate({ status: e.target.value })}
            disabled={update.isPending}
          >
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
            {!STATUSES.includes(user.status as (typeof STATUSES)[number]) && (
              <option value={user.status}>{user.status}</option>
            )}
          </select>
        </div>
      )}
    </Card>
  );
}

/** Profile view with masked PII and a gated reveal + edit dialog. */
function ProfileSection({
  user,
  canEdit,
  canReveal,
}: {
  user: UserDetail;
  canEdit: boolean;
  canReveal: boolean;
}) {
  const p = user.profile;

  return (
    <Card className="p-5">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="font-display text-lg font-bold">Profile</h2>
        {canEdit && <EditProfileDialog user={user} />}
      </div>

      {!p ? (
        <p className="text-ink-3">No profile on file.</p>
      ) : (
        <dl className="space-y-3 text-sm">
          <Row k="Legal name" v={legalName(p)} />
          <Row k="Preferred name" v={p.preferred_name} />
          <Row k="Date of birth" v={p.date_of_birth} />
          <Row k="Phone" v={p.phone} />
          <Row k="Address" v={address(p)} />
          <Row
            k="SSN"
            v={p.has_ssn ? `••• •• ${p.ssn_last4 ?? "••••"}` : "—"}
          />
          <Row
            k="Gov ID"
            v={
              p.has_gov_id
                ? `${p.gov_id_type ?? "ID"} ••••${p.gov_id_last4 ?? ""}`
                : "—"
            }
          />
        </dl>
      )}

      {p && canReveal && (p.has_ssn || p.has_gov_id) && (
        <div className="mt-4">
          <RevealPiiButton userId={user.id} />
        </div>
      )}
    </Card>
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
        <Button variant="outline">
          <Icon name="key" size={15} /> Reveal SSN / ID
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Sensitive identifiers</DialogTitle>
          <DialogDescription>
            Shown only while this dialog is open. Close it to clear from memory.
          </DialogDescription>
        </DialogHeader>
        <div className="my-2 space-y-3">
          {loading && <p className="text-ink-3">Loading…</p>}
          {err && <p className="text-bad">{err}</p>}
          {pii && (
            <dl className="space-y-3 text-sm">
              <Row k="SSN" v={pii.ssn ?? "—"} mono />
              <Row k="Gov ID number" v={pii.gov_id_number ?? "—"} mono />
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

/** Edit-profile dialog (PUT). Sensitive fields are write-only here. */
function EditProfileDialog({ user }: { user: UserDetail }) {
  const [open, setOpen] = useState(false);
  const putProfile = usePutProfile(user.id);
  const p = user.profile;

  const {
    register,
    handleSubmit,
    formState: { isSubmitting },
  } = useForm<ProfileFormInput>({
    resolver: zodResolver(profileFormSchema),
    defaultValues: {
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
      has_pet: p?.has_pet ?? false,
      pet_details: p?.pet_details ?? "",
      is_military: p?.is_military ?? false,
      annual_income:
        p?.annual_income_cents != null
          ? String(p.annual_income_cents / 100)
          : "",
    },
  });

  const onSubmit = handleSubmit(async (values) => {
    // Only forward non-empty fields so blanks don't clobber existing data.
    const { has_pet, is_military, annual_income, ...texts } = values;
    const body: ProfileInput = {
      has_pet: has_pet ?? false,
      is_military: is_military ?? false,
      annual_income_cents: annual_income
        ? Math.round(parseFloat(annual_income) * 100)
        : undefined,
    };
    for (const [key, val] of Object.entries(texts)) {
      if (val) (body as unknown as Record<string, string>)[key] = val as string;
    }
    await putProfile.mutateAsync(body, { onSuccess: () => setOpen(false) });
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline">Edit profile</Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>Edit profile</DialogTitle>
            <DialogDescription>
              Update identity and contact details. Sensitive values are
              write-only and shown masked once saved.
            </DialogDescription>
          </DialogHeader>

          <div className="my-5 space-y-4">
            <div className="grid grid-cols-3 gap-3">
              <FormField label="Legal first">
                <Input {...register("legal_first_name")} />
              </FormField>
              <FormField label="Legal middle">
                <Input {...register("legal_middle_name")} />
              </FormField>
              <FormField label="Legal last">
                <Input {...register("legal_last_name")} />
              </FormField>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <FormField label="Preferred name">
                <Input {...register("preferred_name")} />
              </FormField>
              <FormField label="Date of birth (YYYY-MM-DD)">
                <Input
                  placeholder="1990-01-31"
                  {...register("date_of_birth")}
                />
              </FormField>
            </div>
            <FormField label="Phone">
              <Input {...register("phone")} />
            </FormField>
            <FormField label="Address line 1">
              <Input {...register("address_line1")} />
            </FormField>
            <FormField label="Address line 2">
              <Input {...register("address_line2")} />
            </FormField>
            <div className="grid grid-cols-2 gap-3">
              <FormField label="City">
                <Input {...register("city")} />
              </FormField>
              <FormField label="Region / state">
                <Input {...register("region")} />
              </FormField>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <FormField label="Postal code">
                <Input {...register("postal_code")} />
              </FormField>
              <FormField label="Country">
                <Input {...register("country")} />
              </FormField>
            </div>

            <div className="border-t border-line pt-4">
              <p className="mb-3 text-sm font-bold text-ink-2">
                Rental details
              </p>
              <div className="grid grid-cols-2 gap-3">
                <FormField label="Annual income (USD)">
                  <Input inputMode="decimal" {...register("annual_income")} />
                </FormField>
                <FormField label="Pet details">
                  <Input
                    placeholder="e.g. one 30lb corgi"
                    {...register("pet_details")}
                  />
                </FormField>
              </div>
              <div className="mt-3 flex items-center gap-6 text-sm">
                <label className="flex items-center gap-2">
                  <input type="checkbox" {...register("has_pet")} />
                  <span>Has pet(s)</span>
                </label>
                <label className="flex items-center gap-2">
                  <input type="checkbox" {...register("is_military")} />
                  <span>Military / veteran</span>
                </label>
              </div>
            </div>

            <div className="border-t border-line pt-4">
              <p className="mb-3 text-sm font-bold text-ink-2">
                Sensitive (write-only)
              </p>
              <FormField label="SSN">
                <Input placeholder="Leave blank to keep" {...register("ssn")} />
              </FormField>
              <div className="mt-3 grid grid-cols-2 gap-3">
                <FormField label="Gov ID type">
                  <Input placeholder="passport" {...register("gov_id_type")} />
                </FormField>
                <FormField label="Gov ID number">
                  <Input
                    placeholder="Leave blank to keep"
                    {...register("gov_id_number")}
                  />
                </FormField>
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
              {isSubmitting ? "Saving…" : "Save profile"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Memberships list with add / remove. */
function MembershipsSection({
  user,
  canManage,
}: {
  user: UserDetail;
  canManage: boolean;
}) {
  const remove = useRemoveMembership(user.id);

  return (
    <Card className="p-5">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="font-display text-lg font-bold">Memberships</h2>
        {canManage && <AddMembershipDialog user={user} />}
      </div>
      {user.memberships.length === 0 ? (
        <p className="text-ink-3">No memberships.</p>
      ) : (
        <div className="space-y-2.5">
          {user.memberships.map((m) => (
            <div
              key={m.id}
              className="flex items-center gap-3 rounded-xl border border-line px-3 py-2.5"
            >
              <div className="min-w-0 flex-1">
                <div className="font-semibold">
                  {m.profile_type}
                  {m.title && (
                    <span className="font-normal text-ink-3"> · {m.title}</span>
                  )}
                </div>
                <div className="text-xs text-ink-3">
                  {m.scope}
                  {m.tenant_id && ` · ${m.tenant_id}`}
                  {m.is_primary && " · primary"}
                </div>
              </div>
              <Badge tone={statusTone(m.status)}>{m.status}</Badge>
              {canManage && (
                <button
                  onClick={() => remove.mutate(m.id)}
                  className="text-sm font-semibold text-bad hover:underline"
                >
                  Remove
                </button>
              )}
            </div>
          ))}
        </div>
      )}
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

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!profileType) return;
    const body: MembershipInput = { scope, profile_type: profileType };
    if (scope === "tenant" && tenantId) body.tenant_id = tenantId;
    if (title) body.title = title;
    add.mutate(body, {
      onSuccess: () => {
        setOpen(false);
        setProfileType("");
        setTenantId("");
        setTitle("");
      },
    });
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" className="h-9 px-3">
          Add persona
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Add persona</DialogTitle>
            <DialogDescription>
              Grant {user.name} a membership under a persona.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <FormField label="Scope">
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                value={scope}
                onChange={(e) =>
                  setScope(e.target.value as "tenant" | "platform")
                }
              >
                <option value="tenant">Tenant</option>
                <option value="platform">Platform</option>
              </select>
            </FormField>
            <FormField label="Persona">
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                value={profileType}
                onChange={(e) => setProfileType(e.target.value)}
              >
                <option value="">— Select —</option>
                {scopedTypes.map((t) => (
                  <option key={t.key} value={t.key}>
                    {t.label}
                  </option>
                ))}
              </select>
            </FormField>
            {scope === "tenant" && (
              <FormField label="Tenant ID">
                <Input
                  value={tenantId}
                  onChange={(e) => setTenantId(e.target.value)}
                  placeholder="tenant uuid"
                />
              </FormField>
            )}
            <FormField label="Title (optional)">
              <Input value={title} onChange={(e) => setTitle(e.target.value)} />
            </FormField>
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

/** Roles list with assign / revoke. */
function RolesSection({
  user,
  canManage,
}: {
  user: UserDetail;
  canManage: boolean;
}) {
  const revoke = useRevokeRole(user.id);

  return (
    <Card className="p-5">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="font-display text-lg font-bold">Roles</h2>
        {canManage && <AssignRoleDialog user={user} />}
      </div>
      {user.roles.length === 0 ? (
        <p className="text-ink-3">No roles assigned.</p>
      ) : (
        <div className="space-y-2.5">
          {user.roles.map((r) => (
            <div
              key={r.id}
              className="flex items-center gap-3 rounded-xl border border-line px-3 py-2.5"
            >
              <Icon name="shield" size={16} className="text-ink-3" />
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{r.role_name}</div>
                <code className="text-xs text-ink-3">{r.role_key}</code>
              </div>
              {canManage && (
                <button
                  onClick={() => revoke.mutate(r.id)}
                  className="text-sm font-semibold text-bad hover:underline"
                >
                  Revoke
                </button>
              )}
            </div>
          ))}
        </div>
      )}
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

  function submit(e: React.FormEvent) {
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
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" className="h-9 px-3">
          Assign role
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Assign role</DialogTitle>
            <DialogDescription>
              Grant {user.name} an additional role.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5">
            <FormField label="Role">
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                value={roleId}
                onChange={(e) => setRoleId(e.target.value)}
              >
                <option value="">— Select —</option>
                {available.map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.name} ({r.scope})
                  </option>
                ))}
              </select>
            </FormField>
          </div>
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

// ---- small presentational helpers --------------------------------------------

/** A definition-list row; renders an em-dash for empty values. */
function Row({
  k,
  v,
  mono,
}: {
  k: string;
  v: string | null | undefined;
  mono?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-line pb-2.5 last:border-0">
      <dt className="text-ink-3">{k}</dt>
      <dd className={mono ? "font-mono font-semibold" : "font-semibold"}>
        {v || "—"}
      </dd>
    </div>
  );
}

/** Labelled form field wrapper. */
function FormField({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

/** Assemble a legal name string from profile parts. */
function legalName(p: ProfileDto): string {
  return (
    [p.legal_first_name, p.legal_middle_name, p.legal_last_name]
      .filter(Boolean)
      .join(" ") || "—"
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
      .join(", ") || "—"
  );
}
