"use client";

// Owners: the people whose properties you manage. A searchable directory,
// a form to add one, and the side-panel details: contact, LLCs, portal
// access, and their own approval limit.

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  BadgeDollarSign,
  ChevronRight,
  DoorOpen,
  Handshake,
  Plus,
  Search,
  Send,
} from "lucide-react";
import { toast } from "sonner";
import { crm, toCents, type OwnerRow } from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { ownersExtra, type OwnerInviteResult } from "@/lib/comms-extra";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, Input, fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import {
  ago,
  errMsg,
  humanize,
  MiniStat,
  OWNER_KINDS,
  type Opened,
} from "./shared";

export function OwnersTab({
  owners,
  loading,
  error,
  manage,
  onOpen,
}: {
  owners: OwnerRow[] | undefined;
  loading: boolean;
  error: Error | null;
  manage: boolean;
  onOpen: (o: Opened) => void;
}) {
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const q = query.trim().toLowerCase();
  const rows = (owners ?? []).filter(
    (o) =>
      !q ||
      o.name.toLowerCase().includes(q) ||
      (o.email ?? "").toLowerCase().includes(q) ||
      o.entities.some((e) => e.toLowerCase().includes(q))
  );

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="relative sm:w-72">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search owners, emails or LLCs"
            aria-label="Search owners"
            className="h-10 pl-9"
          />
        </div>
        {manage && (
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus />
            New owner
          </Button>
        )}
      </div>

      {loading && (
        <div className="space-y-2 p-3">
          {Array.from({ length: 5 }, (_, i) => (
            <Skeleton key={i} className="h-14" />
          ))}
        </div>
      )}
      {error && (
        <p className="p-4 text-[13px] text-bad">
          Couldn&apos;t load owners: {error.message}
        </p>
      )}
      {owners && rows.length === 0 && (
        <EmptyState
          icon={<Handshake />}
          title={q ? "No owners match that search" : "No owners yet"}
          description={
            q ? undefined : "Add one, or win a lead in the pipeline."
          }
        />
      )}
      {rows.length > 0 && (
        <div className="hidden grid-cols-[1.4fr_1.3fr_1.2fr_.5fr_.5fr_.9fr_.8fr_16px] gap-4 border-b border-line px-5 py-2.5 lg:grid">
          {[
            "Owner",
            "Contact",
            "Entities",
            "Props",
            "Doors",
            "Last contact",
            "Follow-ups",
          ].map((h) => (
            <span
              key={h}
              className={cn(
                "eyebrow",
                (h === "Props" || h === "Doors" || h === "Follow-ups") &&
                  "text-right"
              )}
            >
              {h}
            </span>
          ))}
          <span />
        </div>
      )}
      <ul className="divide-y divide-line">
        {rows.map((o) => (
          <li key={o.id}>
            <button
              type="button"
              onClick={() => onOpen({ type: "owner", id: o.id, name: o.name })}
              className="flex w-full items-center gap-3 px-5 py-3 text-left transition hover:bg-fill-2 lg:grid lg:grid-cols-[1.4fr_1.3fr_1.2fr_.5fr_.5fr_.9fr_.8fr_16px] lg:gap-4"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate text-[14px] font-medium text-fg">
                  {o.name}
                </div>
                <div className="truncate text-xs text-fg-3">
                  {humanize(o.kind)}
                  <span className="lg:hidden">
                    {" "}
                    · {o.properties} props · {o.doors} doors · last contact{" "}
                    {ago(o.last_contact_at).toLowerCase()}
                  </span>
                </div>
              </div>
              <div className="hidden min-w-0 text-[13px] text-fg-2 lg:block">
                <div className="truncate">{o.email ?? "-"}</div>
                {o.phone && (
                  <div className="truncate text-xs text-fg-3">{o.phone}</div>
                )}
              </div>
              <div className="hidden min-w-0 truncate text-[13px] text-fg-2 lg:block">
                {o.entities.length ? o.entities.join(", ") : "-"}
              </div>
              <span className="figure hidden text-right text-[13px] text-fg lg:block">
                {o.properties}
              </span>
              <span className="figure hidden text-right text-[13px] text-fg lg:block">
                {o.doors}
              </span>
              <span className="hidden text-[13px] text-fg-2 lg:block">
                {ago(o.last_contact_at)}
              </span>
              <span className="flex shrink-0 justify-end gap-1.5">
                {o.follow_ups_due > 0 && (
                  <Badge tone="bad">{o.follow_ups_due} due</Badge>
                )}
                {o.open_follow_ups > o.follow_ups_due && (
                  <Badge>{o.open_follow_ups - o.follow_ups_due} open</Badge>
                )}
                {o.open_follow_ups === 0 && (
                  <span className="hidden text-[13px] text-fg-4 lg:inline">
                    -
                  </span>
                )}
              </span>
              <ChevronRight className="size-4 shrink-0 text-fg-4" />
            </button>
          </li>
        ))}
      </ul>

      {creating && (
        <NewOwnerDialog
          onClose={() => setCreating(false)}
          onCreated={(o) => {
            setCreating(false);
            onOpen({ type: "owner", id: o.id, name: o.name });
          }}
        />
      )}
    </Panel>
  );
}

/** Name, type, email, phone and notes: shared by the new and edit forms. */
function OwnerFields({
  name,
  setName,
  kind,
  setKind,
  email,
  setEmail,
  phone,
  setPhone,
  notes,
  setNotes,
}: {
  name: string;
  setName: (v: string) => void;
  kind: string;
  setKind: (v: string) => void;
  email: string;
  setEmail: (v: string) => void;
  phone: string;
  setPhone: (v: string) => void;
  notes: string;
  setNotes: (v: string) => void;
}) {
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-[1fr_auto] gap-3">
        <Field label="Name">
          {(f) => (
            <Input
              {...f}
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          )}
        </Field>
        <Field label="Type">
          {(f) => (
            <select
              {...f}
              className={cn(fieldClass, "h-11")}
              value={kind}
              onChange={(e) => setKind(e.target.value)}
            >
              {OWNER_KINDS.map((k) => (
                <option key={k} value={k}>
                  {humanize(k)}
                </option>
              ))}
            </select>
          )}
        </Field>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Email">
          {(f) => (
            <Input
              {...f}
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          )}
        </Field>
        <Field label="Phone">
          {(f) => (
            <Input
              {...f}
              type="tel"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
            />
          )}
        </Field>
      </div>
      <Field label="Notes">
        {(f) => (
          <textarea
            {...f}
            rows={2}
            className={cn(fieldClass, "w-full")}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        )}
      </Field>
    </div>
  );
}

function NewOwnerDialog({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (o: { id: string; name: string }) => void;
}) {
  const qc = useQueryClient();
  const [name, setName] = useState("");
  const [kind, setKind] = useState<string>("individual");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit() {
    setBusy(true);
    try {
      const o = await crm.createOwner({
        name: name.trim(),
        kind,
        email: email.trim() || undefined,
        phone: phone.trim() || undefined,
        notes: notes.trim() || undefined,
      });
      toast.success(`Added ${o.name}`);
      await qc.invalidateQueries({ queryKey: ["crm", "owners"] });
      onCreated(o);
    } catch (e) {
      toast.error(errMsg(e, "Couldn't add the owner"));
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogTitle className="text-[17px] font-semibold">
          New owner
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Someone whose properties you manage. Link their LLCs and properties
          from Entities.
        </DialogDescription>
        <form
          className="mt-4"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <OwnerFields
            {...{ name, setName, kind, setKind, email, setEmail }}
            {...{ phone, setPhone, notes, setNotes }}
          />
          <div className="mt-5 flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy} disabled={!name.trim()}>
              {!busy && <Plus />}
              Add owner
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function OwnerDetails({
  owner,
  manage,
}: {
  owner: OwnerRow;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const [name, setName] = useState(owner.name);
  const [kind, setKind] = useState(owner.kind);
  const [email, setEmail] = useState(owner.email ?? "");
  const [phone, setPhone] = useState(owner.phone ?? "");
  const [notes, setNotes] = useState(owner.notes ?? "");
  const [busy, setBusy] = useState(false);

  const dirty =
    name !== owner.name ||
    kind !== owner.kind ||
    email !== (owner.email ?? "") ||
    phone !== (owner.phone ?? "") ||
    notes !== (owner.notes ?? "");

  async function save() {
    setBusy(true);
    try {
      await crm.updateOwner(owner.id, {
        name: name.trim(),
        kind,
        email: email.trim(),
        phone: phone.trim(),
        notes: notes.trim(),
      });
      toast.success("Owner saved");
      await qc.invalidateQueries({ queryKey: ["crm", "owners"] });
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save the owner"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-5">
      <div className="grid grid-cols-3 gap-2">
        <MiniStat label="Properties" value={String(owner.properties)} />
        <MiniStat label="Doors" value={String(owner.doors)} />
        <MiniStat label="Last contact" value={ago(owner.last_contact_at)} />
      </div>
      {owner.entities.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-fg-3">Entities</span>
          {owner.entities.map((e) => (
            <Badge key={e}>{e}</Badge>
          ))}
        </div>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <fieldset disabled={!manage}>
          <OwnerFields
            {...{ name, setName, kind, setKind, email, setEmail }}
            {...{ phone, setPhone, notes, setNotes }}
          />
        </fieldset>
        {manage && (
          <div className="mt-3 flex justify-end">
            <Button
              type="submit"
              loading={busy}
              disabled={!dirty || !name.trim()}
            >
              Save changes
            </Button>
          </div>
        )}
      </form>
      {manage && (
        <div className="grid gap-3 sm:grid-cols-2">
          <PortalAccess
            owner={owner}
            dirtyEmail={email !== (owner.email ?? "")}
          />
          <ApprovalLimit owner={owner} />
        </div>
      )}
    </section>
  );
}

/** Give the owner a login to the owner portal. */
function PortalAccess({
  owner,
  dirtyEmail,
}: {
  owner: OwnerRow;
  dirtyEmail: boolean;
}) {
  const { can } = useAuth();
  const allowed = can("member:manage");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<OwnerInviteResult | null>(null);
  const hasEmail = !!owner.email?.includes("@");

  async function invite() {
    setBusy(true);
    try {
      const r = await ownersExtra.invite(owner.id);
      setResult(r);
      toast.success(
        r.outcome === "invited"
          ? `Sent ${owner.name} a link to set a password`
          : `${owner.name}'s account now opens the owner portal`
      );
    } catch (e) {
      toast.error(errMsg(e, "Couldn't invite them"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="rounded-xl border border-line p-3">
      <div className="flex items-center gap-2 text-[13px] font-medium text-fg">
        <DoorOpen className="size-4 text-fg-3" />
        Owner portal
      </div>
      <p className="mt-1 text-xs text-fg-3">
        {result
          ? result.outcome === "invited"
            ? "Invited. They get an email to set a password, then land on their statements and approvals."
            : "Linked. Their existing account now opens the owner portal."
          : owner.user_id
            ? "Has a login. They see approvals, statements and open work for their properties."
            : "Approvals, statements and open work for their properties. Sends a link to set a password; an existing account is linked."}
      </p>
      <Button
        size="sm"
        variant="secondary"
        className="mt-2"
        loading={busy}
        disabled={!allowed || !hasEmail || dirtyEmail}
        onClick={invite}
      >
        {!busy && <Send />}
        {result || owner.user_id
          ? "Send the link again"
          : "Invite to the portal"}
      </Button>
      {!allowed ? (
        <p className="mt-1.5 text-[11px] text-fg-4">
          Needs the member:manage permission.
        </p>
      ) : !hasEmail ? (
        <p className="mt-1.5 text-[11px] text-fg-4">Add an email first.</p>
      ) : dirtyEmail ? (
        <p className="mt-1.5 text-[11px] text-fg-4">
          Save the new email first.
        </p>
      ) : null}
    </div>
  );
}

/** The owner's own limit for work before they're asked to approve it. */
function ApprovalLimit({ owner }: { owner: OwnerRow }) {
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState<number | null | undefined>(undefined);

  async function set(cents: number | null) {
    setBusy(true);
    try {
      const r = await ownersExtra.setApprovalLimit(owner.id, cents);
      setSaved(r.approval_limit_cents);
      setValue("");
      toast.success(
        r.approval_limit_cents === null
          ? `${owner.name} uses the workspace limit`
          : `${owner.name}'s limit is $${(r.approval_limit_cents / 100).toLocaleString()}`
      );
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save the limit"));
    } finally {
      setBusy(false);
    }
  }

  const cents = value.trim() ? toCents(value) : null;

  return (
    <div className="rounded-xl border border-line p-3">
      <div className="flex items-center gap-2 text-[13px] font-medium text-fg">
        <BadgeDollarSign className="size-4 text-fg-3" />
        Approval limit
      </div>
      <p className="mt-1 text-xs text-fg-3">
        {saved === undefined
          ? owner.approval_limit_cents !== null
            ? `Now $${(owner.approval_limit_cents / 100).toLocaleString()}. Work estimated at or over it waits for their OK.`
            : "Work estimated at or over this waits for their OK. Without one, the workspace setting applies."
          : saved === null
            ? "Now using the workspace setting."
            : `Now $${(saved / 100).toLocaleString()}.`}
      </p>
      <form
        className="mt-2 flex flex-wrap gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (cents !== null) void set(cents);
        }}
      >
        <input
          aria-label="Approval limit in dollars"
          inputMode="decimal"
          placeholder="$ amount"
          className={cn(fieldClass, "w-28 py-1.5")}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <Button
          type="submit"
          size="sm"
          variant="secondary"
          disabled={busy || cents === null}
        >
          Set
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={busy}
          onClick={() => set(null)}
        >
          Use workspace limit
        </Button>
      </form>
    </div>
  );
}
