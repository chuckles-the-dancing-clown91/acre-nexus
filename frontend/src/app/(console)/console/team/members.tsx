"use client";

// Members: the people who can sign in to this workspace, their persona (which
// sets their default role) and title, and whether they've chosen a password.
// `member:read` to see; `member:manage` to invite and send sign-in links.
//
// `MembersPage` is the same directory as a page of its own, for the Admin
// nav's /console/members.

import { useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { KeyRound, MailPlus, Search, UserPlus, Users } from "lucide-react";
import { toast } from "sonner";
import { iam, type Member } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { queryKeys, useMembers, useProfileTypes } from "@/lib/queries";
import { inviteMemberSchema } from "@/lib/schemas";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, Input, fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

export function MembersPage() {
  const { can } = useAuth();
  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Admin"
        title="Members"
        description="People with access to your workspace."
      />
      {can("member:read") ? (
        <MembersDirectory />
      ) : (
        <Panel>
          <EmptyState
            icon={<Users />}
            title="You don't have access to members"
            description="Ask an admin for the member:read permission."
          />
        </Panel>
      )}
    </div>
  );
}

export function MembersDirectory() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const manage = can("member:manage");
  const members = useMembers({ enabled: scoped && can("member:read") });
  const profileTypes = useProfileTypes({ enabled: scoped });
  const [q, setQ] = useState("");
  const [persona, setPersona] = useState("");
  const [inviting, setInviting] = useState(false);

  const labels = useMemo(
    () => new Map((profileTypes.data ?? []).map((t) => [t.key, t.label])),
    [profileTypes.data]
  );
  const personas = useMemo(
    () => [...new Set((members.data ?? []).map((m) => m.profile_type))].sort(),
    [members.data]
  );
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (members.data ?? [])
      .filter((m) => !persona || m.profile_type === persona)
      .filter(
        (m) =>
          !needle ||
          m.name.toLowerCase().includes(needle) ||
          m.email.toLowerCase().includes(needle) ||
          (m.title ?? "").toLowerCase().includes(needle)
      )
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [members.data, q, persona]);
  const invited = (members.data ?? []).filter(
    (m) => m.account_status === "invited"
  ).length;

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center">
        <div className="relative sm:w-64">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search members"
            aria-label="Search members"
            className="h-10 pl-9"
          />
        </div>
        <select
          aria-label="Persona"
          value={persona}
          onChange={(e) => setPersona(e.target.value)}
          className={fieldClass}
        >
          <option value="">Every persona</option>
          {personas.map((p) => (
            <option key={p} value={p}>
              {labels.get(p) ?? p}
            </option>
          ))}
        </select>
        {invited > 0 && (
          <span className="text-xs text-fg-3">
            {invited} still to set a password
          </span>
        )}
        {manage && (
          <Button
            size="sm"
            className="sm:ml-auto"
            onClick={() => setInviting(true)}
          >
            <UserPlus />
            Invite member
          </Button>
        )}
      </div>
      {members.isLoading && (
        <div className="space-y-2 p-3">
          {Array.from({ length: 5 }, (_, i) => (
            <Skeleton key={i} className="h-14" />
          ))}
        </div>
      )}
      {members.error && (
        <p className="p-4 text-[13px] text-bad">
          Couldn&apos;t load members: {members.error.message}
        </p>
      )}
      {members.data && rows.length === 0 && (
        <EmptyState
          icon={<Users />}
          title={members.data.length ? "Nobody matches that" : "No members yet"}
        />
      )}
      <ul className="divide-y divide-line">
        {rows.map((m) => (
          <li
            key={m.membership_id}
            className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:items-center sm:gap-4"
          >
            <div className="flex min-w-0 flex-1 items-center gap-3">
              <span className="flex size-9 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[12px] font-semibold text-accent">
                {m.name
                  .split(" ")
                  .map((w) => w[0])
                  .slice(0, 2)
                  .join("")}
              </span>
              <div className="min-w-0">
                <div className="truncate text-[14px] font-medium text-fg">
                  {m.name}
                </div>
                <div className="truncate text-xs text-fg-3">{m.email}</div>
              </div>
            </div>
            <div className="text-[13px] text-fg-2 sm:w-56">
              {labels.get(m.profile_type) ?? m.profile_type}
              {m.title && <span className="text-fg-3"> · {m.title}</span>}
            </div>
            <div className="flex items-center gap-2 sm:w-64 sm:justify-end">
              {m.account_status === "invited" ? (
                <Badge tone="warn">hasn&apos;t set a password</Badge>
              ) : (
                <Badge tone={statusTone(m.status)}>{m.status}</Badge>
              )}
              {manage && <LoginLinkButton member={m} />}
            </div>
          </li>
        ))}
      </ul>

      {inviting && <InviteDialog onClose={() => setInviting(false)} />}
    </Panel>
  );
}

/** Resend the link a member uses to choose (or reset) their password. */
function LoginLinkButton({ member }: { member: Member }) {
  const [busy, setBusy] = useState(false);
  const invited = member.account_status === "invited";
  async function send() {
    setBusy(true);
    try {
      const res = await iam.sendLoginLink(member.membership_id);
      toast.success(
        res.purpose === "invite"
          ? `Sent ${member.name} a new link to set their password`
          : `Sent ${member.name} a password reset link`
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send the link");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Button
      size="sm"
      variant="ghost"
      loading={busy}
      onClick={send}
      title={
        invited ? "Resend their invite link" : "Send a password reset link"
      }
    >
      {!busy && (invited ? <MailPlus /> : <KeyRound />)}
      {invited ? "Resend invite" : "Reset link"}
    </Button>
  );
}

type FormErrors = Partial<Record<"email" | "name" | "profile_type", string>>;

function InviteDialog({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const profileTypes = useProfileTypes();
  const tenantTypes = (profileTypes.data ?? []).filter(
    (t) => t.scope === "tenant"
  );
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [profileType, setProfileType] = useState("");
  const [title, setTitle] = useState("");
  const [errors, setErrors] = useState<FormErrors>({});
  const [busy, setBusy] = useState(false);
  const chosen = tenantTypes.find((t) => t.key === profileType);

  async function submit() {
    const parsed = inviteMemberSchema.safeParse({
      email,
      name,
      profile_type: profileType,
      title,
    });
    if (!parsed.success) {
      const next: FormErrors = {};
      for (const issue of parsed.error.issues) {
        const k = issue.path[0] as keyof FormErrors;
        if (!next[k]) next[k] = issue.message;
      }
      setErrors(next);
      return;
    }
    setErrors({});
    setBusy(true);
    try {
      const v = parsed.data;
      await iam.inviteMember({
        email: v.email,
        name: v.name,
        profile_type: v.profile_type,
        ...(v.title ? { title: v.title } : {}),
      });
      toast.success(`Invited ${v.name}. They get a link to set a password.`);
      await qc.invalidateQueries({ queryKey: queryKeys.members });
      void qc.invalidateQueries({ queryKey: ["team"] });
      onClose();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't invite them");
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Invite a member
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          They get an email with a link to set their password. Their persona
          gives them its default role.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <Field label="Email" error={errors.email}>
            {(f) => (
              <Input
                {...f}
                autoFocus
                type="email"
                placeholder="person@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            )}
          </Field>
          <Field label="Full name" error={errors.name}>
            {(f) => (
              <Input
                {...f}
                placeholder="Jane Doe"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            )}
          </Field>
          <Field
            label="Persona"
            error={errors.profile_type}
            hint={chosen?.description}
          >
            {(f) => (
              <select
                {...f}
                className={cn(fieldClass, "h-11 w-full")}
                value={profileType}
                onChange={(e) => setProfileType(e.target.value)}
              >
                <option value="">Pick one</option>
                {tenantTypes.map((t) => (
                  <option key={t.key} value={t.key}>
                    {t.label}
                  </option>
                ))}
              </select>
            )}
          </Field>
          <Field label="Title (optional)">
            {(f) => (
              <Input
                {...f}
                placeholder="e.g. Property manager"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
              />
            )}
          </Field>
          <div className="flex justify-end gap-2 pt-1">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy}>
              {!busy && <UserPlus />}
              Invite
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
