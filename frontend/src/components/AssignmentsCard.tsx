"use client";

// A reusable "Team" card: lists the people assigned to a property or legal
// entity (LLC) and, for users who can manage it, an Assign dialog + remove
// controls. Assigning also grants the person scoped access to the subject
// (property:{id} / entity:{id}) — see the backend `routes/assignments`.

import { useState } from "react";

import { useAuth } from "@/lib/auth";
import {
  useAssignments,
  useCreateAssignment,
  useDeleteAssignment,
  useMembers,
} from "@/lib/queries";
import { ASSIGNABLE_RELATIONSHIPS, type AssignmentSubject } from "@/lib/types";
import { Badge, Card } from "@/components/ui";
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

const SELECT_CLS =
  "flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent";

export function AssignmentsCard({
  subjectType,
  subjectId,
  writePermission,
}: {
  subjectType: AssignmentSubject;
  subjectId: string;
  /** Permission that gates assigning/removing (e.g. "property:write"). */
  writePermission: string;
}) {
  const { can } = useAuth();
  const canWrite = can(writePermission);
  const {
    data: team,
    isLoading,
    error,
  } = useAssignments(subjectType, subjectId);
  const remove = useDeleteAssignment(subjectType, subjectId);

  const scopeBlurb =
    subjectType === "entity"
      ? "this entity and every property it holds title to"
      : "this property";

  return (
    <Card className="space-y-4 p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="font-display text-lg font-bold">Team</h2>
          <p className="text-sm text-ink-3">
            Assigning someone also grants them access to {scopeBlurb}.
          </p>
        </div>
        {canWrite && (
          <AssignDialog subjectType={subjectType} subjectId={subjectId} />
        )}
      </div>

      {error && <p className="text-sm text-bad">{error.message}</p>}

      <div className="divide-y divide-line">
        {team?.map((a) => (
          <div
            key={a.id}
            className="flex items-center justify-between gap-3 py-3"
          >
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className="truncate font-semibold">{a.user_name}</span>
                {a.is_primary && <Badge tone="accent">Primary</Badge>}
              </div>
              <div className="truncate text-sm text-ink-3">
                {a.relationship_label}
                {a.title ? ` · ${a.title}` : ""} · {a.user_email}
              </div>
            </div>
            {canWrite && (
              <Button
                variant="ghost"
                onClick={() => remove.mutate(a.id)}
                disabled={remove.isPending}
              >
                Remove
              </Button>
            )}
          </div>
        ))}
        {isLoading && (
          <div className="py-6 text-center text-ink-3">Loading…</div>
        )}
        {team && team.length === 0 && (
          <div className="py-6 text-center text-ink-3">
            No one assigned yet.
          </div>
        )}
      </div>
    </Card>
  );
}

/** Dialog to assign a workspace member to this subject with a relationship. */
function AssignDialog({
  subjectType,
  subjectId,
}: {
  subjectType: AssignmentSubject;
  subjectId: string;
}) {
  const [open, setOpen] = useState(false);
  const [userId, setUserId] = useState("");
  const [relationship, setRelationship] = useState(
    ASSIGNABLE_RELATIONSHIPS[0].key
  );
  const [isPrimary, setIsPrimary] = useState(false);
  const [title, setTitle] = useState("");
  const { data: members } = useMembers();
  const create = useCreateAssignment(subjectType, subjectId);

  const reset = () => {
    setUserId("");
    setRelationship(ASSIGNABLE_RELATIONSHIPS[0].key);
    setIsPrimary(false);
    setTitle("");
  };

  const onSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!userId) return;
    create.mutate(
      {
        user_id: userId,
        relationship,
        is_primary: isPrimary,
        ...(title ? { title } : {}),
      },
      {
        onSuccess: () => {
          reset();
          setOpen(false);
        },
      }
    );
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>Assign</Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>Assign staff</DialogTitle>
            <DialogDescription>
              Give a workspace member a role here. This grants them scoped
              access.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="space-y-1.5">
              <Label>Person</Label>
              <select
                className={SELECT_CLS}
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
              >
                <option value="">— Select member —</option>
                {(members ?? []).map((m) => (
                  <option key={m.user_id} value={m.user_id}>
                    {m.name} · {m.email}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1.5">
              <Label>Relationship</Label>
              <select
                className={SELECT_CLS}
                value={relationship}
                onChange={(e) => setRelationship(e.target.value)}
              >
                {ASSIGNABLE_RELATIONSHIPS.map((r) => (
                  <option key={r.key} value={r.key}>
                    {r.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1.5">
              <Label>Title (optional)</Label>
              <Input
                placeholder="e.g. Lead PM"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
              />
            </div>
            <label className="flex items-center gap-2 text-sm text-ink-2">
              <input
                type="checkbox"
                checked={isPrimary}
                onChange={(e) => setIsPrimary(e.target.checked)}
              />
              Primary contact for this relationship
            </label>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setOpen(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={create.isPending || !userId}>
              {create.isPending ? "Assigning…" : "Assign"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
