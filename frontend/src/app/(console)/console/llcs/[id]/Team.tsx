"use client";

// The people assigned to an LLC. Assigning someone also gives them access to
// the entity and every property it holds.

import { useState } from "react";
import { UserPlus, Users, X } from "lucide-react";
import { useAuth } from "@/lib/auth";
import {
  useAssignments,
  useCreateAssignment,
  useDeleteAssignment,
  useMembers,
} from "@/lib/queries";
import { ASSIGNABLE_RELATIONSHIPS } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass, Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F } from "@/components/property/bits";

export function Team({ entityId }: { entityId: string }) {
  const { can } = useAuth();
  const manage = can("entity:manage");
  const team = useAssignments("entity", entityId);
  const remove = useDeleteAssignment("entity", entityId);
  const [open, setOpen] = useState(false);

  return (
    <Panel>
      <PanelHeader
        title="Team"
        description="Assigning someone also gives them access to this entity and every property it holds."
        action={
          manage && (
            <Button size="sm" variant="secondary" onClick={() => setOpen(true)}>
              <UserPlus />
              Assign
            </Button>
          )
        }
      />
      <div className="p-2 pt-3">
        {team.isLoading && <Skeleton className="m-3 h-14" />}
        {team.error && (
          <p className="px-3 py-2 text-[13px] text-bad">{team.error.message}</p>
        )}
        {team.data?.length === 0 && (
          <EmptyState
            icon={<Users />}
            title="Nobody assigned"
            className="py-8"
          />
        )}
        <ul className="divide-y divide-line">
          {team.data?.map((a) => (
            <li key={a.id} className="flex items-center gap-3 px-3 py-2.5">
              <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[12px] font-semibold text-accent">
                {a.user_name
                  .split(" ")
                  .map((w) => w[0])
                  .slice(0, 2)
                  .join("")}
              </span>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-[13px] font-medium text-fg">
                    {a.user_name}
                  </span>
                  {a.is_primary && <Badge tone="accent">primary</Badge>}
                </div>
                <div className="truncate text-xs text-fg-3">
                  {a.relationship_label}
                  {a.title ? ` · ${a.title}` : ""}
                  {a.user_email ? ` · ${a.user_email}` : ""}
                </div>
              </div>
              {manage && (
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={`Unassign ${a.user_name}`}
                  disabled={remove.isPending}
                  onClick={() => {
                    if (confirm(`Unassign ${a.user_name}?`))
                      remove.mutate(a.id);
                  }}
                >
                  <X />
                </Button>
              )}
            </li>
          ))}
        </ul>
      </div>
      {manage && (
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogContent>
            {open && (
              <AssignForm entityId={entityId} onDone={() => setOpen(false)} />
            )}
          </DialogContent>
        </Dialog>
      )}
    </Panel>
  );
}

function AssignForm({
  entityId,
  onDone,
}: {
  entityId: string;
  onDone: () => void;
}) {
  const { can } = useAuth();
  const members = useMembers({ enabled: can("member:read") });
  const create = useCreateAssignment("entity", entityId);
  const [userId, setUserId] = useState("");
  const [relationship, setRelationship] = useState(
    ASSIGNABLE_RELATIONSHIPS[0].key
  );
  const [title, setTitle] = useState("");
  const [primary, setPrimary] = useState(false);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (!userId) return;
        create.mutate(
          {
            user_id: userId,
            relationship,
            is_primary: primary,
            ...(title.trim() ? { title: title.trim() } : {}),
          },
          { onSuccess: onDone }
        );
      }}
    >
      <DialogTitle className="text-[17px] font-semibold">
        Assign someone
      </DialogTitle>
      <DialogDescription className="mt-1 text-[13px] text-fg-3">
        Give a teammate a role on this entity.
      </DialogDescription>
      <div className="mt-4 space-y-3">
        <F label="Person">
          <select
            className={`${fieldClass} w-full`}
            value={userId}
            onChange={(e) => setUserId(e.target.value)}
            required
          >
            <option value="">Choose a teammate</option>
            {(members.data ?? [])
              .filter((m) => m.status === "active")
              .map((m) => (
                <option key={m.user_id} value={m.user_id}>
                  {m.name} · {m.email}
                </option>
              ))}
          </select>
        </F>
        <F label="As">
          <select
            className={`${fieldClass} w-full`}
            value={relationship}
            onChange={(e) => setRelationship(e.target.value)}
          >
            {ASSIGNABLE_RELATIONSHIPS.map((r) => (
              <option key={r.key} value={r.key}>
                {r.label}
              </option>
            ))}
          </select>
        </F>
        <F label="Title (optional)">
          <Input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Lead PM"
          />
        </F>
        <label className="flex items-center gap-2 text-[13px] text-fg-2">
          <input
            type="checkbox"
            className="accent-[var(--accent)]"
            checked={primary}
            onChange={(e) => setPrimary(e.target.checked)}
          />
          Primary contact for this role
        </label>
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" disabled={!userId} loading={create.isPending}>
          Assign
        </Button>
      </div>
    </form>
  );
}
