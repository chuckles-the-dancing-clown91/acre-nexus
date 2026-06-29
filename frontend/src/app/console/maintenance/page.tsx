"use client";

// Maintenance work orders — a Kanban-style board grouped by ticket status.
// Read access via `maintenance:read`; creating tickets and changing status
// requires `maintenance:manage`. Data flows through the TanStack Query hooks in
// queries.ts; the create/detail dialogs are colocated below.

import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useQuery } from "@tanstack/react-query";
import {
  CircleDot,
  Loader2,
  MessageSquare,
  Plus,
  User as UserIcon,
  Wrench,
} from "lucide-react";

import { api, iam } from "@/lib/api";
import type { Member } from "@/lib/api";
import type { Counterparty, MaintenanceTicket } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import {
  useAddTicketComment,
  useCreateTicket,
  useProperties,
  useTicket,
  useTickets,
  useUpdateTicket,
} from "@/lib/queries";
import { formatDateTime, relativeDate, titleCase } from "@/lib/format";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { PageHeader, EmptyState } from "@/components/ui/page";
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
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, Input, Textarea } from "@/components/ui/form-field";
import { Skeleton } from "@/components/ui/skeleton";

// Canonical lifecycle the board always lays out, left → right. Any extra status
// present in the data (e.g. on_hold) is appended so no ticket is ever orphaned.
const BOARD_STATUSES = ["open", "in_progress", "resolved"] as const;
const ALL_STATUSES = [
  "open",
  "triage",
  "scheduled",
  "in_progress",
  "on_hold",
  "resolved",
  "closed",
] as const;

const PRIORITIES = ["low", "normal", "high", "urgent"] as const;
const CATEGORIES = [
  "general",
  "plumbing",
  "electrical",
  "hvac",
  "appliance",
  "structural",
  "landscaping",
  "turn",
] as const;

type Tone = "neutral" | "good" | "warn" | "bad" | "info" | "accent";

/** Tone for a ticket priority. */
function priorityTone(p: string): Tone {
  if (p === "urgent") return "bad";
  if (p === "high") return "warn";
  if (p === "normal") return "info";
  return "neutral";
}

/** Tone for a ticket status. */
function statusToneFor(s: string): Tone {
  if (s === "resolved" || s === "closed") return "good";
  if (s === "in_progress" || s === "scheduled") return "info";
  if (s === "on_hold") return "warn";
  return "neutral";
}

function humanize(key: string): string {
  return titleCase(key.replace(/_/g, " "));
}

export default function MaintenancePage() {
  const { can } = useAuth();
  const canManage = can("maintenance:manage");

  const [status, setStatus] = useState<string>("all");
  const [createOpen, setCreateOpen] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);

  const tickets = useTickets(status === "all" ? {} : { status });
  const properties = useProperties();

  // Assignees can be members (users) or entities (contractors). Pull both so we
  // can resolve a friendly name on each card without per-ticket fetches. Both
  // are best-effort: a missing name just falls back to "Unassigned".
  const members = useQuery<Member[]>({
    queryKey: ["iam", "members"],
    queryFn: () => iam.members(),
    staleTime: 5 * 60_000,
  });
  const entities = useQuery<Counterparty[]>({
    queryKey: ["entities", ""],
    queryFn: () => api.entities(),
    staleTime: 5 * 60_000,
  });

  const data = useMemo(() => tickets.data ?? [], [tickets.data]);

  const propName = useMemo(() => {
    const m = new Map((properties.data ?? []).map((p) => [p.id, p.name]));
    return (id: string) => m.get(id) ?? "Unassigned property";
  }, [properties.data]);

  // Build the board columns: canonical lifecycle first, then any extra statuses
  // that actually appear in the data.
  const columns = useMemo(() => {
    const extra = Array.from(new Set(data.map((t) => t.status))).filter(
      (s) => !BOARD_STATUSES.includes(s as (typeof BOARD_STATUSES)[number])
    );
    const keys =
      status === "all"
        ? [...BOARD_STATUSES, ...extra]
        : Array.from(new Set([status, ...BOARD_STATUSES]));
    return keys.map((key) => ({
      key,
      items: data.filter((t) => t.status === key),
    }));
  }, [data, status]);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Maintenance"
        title="Work orders"
        description="Repair and turn tickets across the portfolio, grouped by status."
        actions={
          <div className="flex items-center gap-2">
            <div className="w-44">
              <Select value={status} onValueChange={setStatus}>
                <SelectTrigger aria-label="Filter by status">
                  <SelectValue placeholder="All statuses" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All statuses</SelectItem>
                  {ALL_STATUSES.map((s) => (
                    <SelectItem key={s} value={s}>
                      {humanize(s)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {canManage && (
              <Button onClick={() => setCreateOpen(true)}>
                <Plus className="h-4 w-4" />
                New ticket
              </Button>
            )}
          </div>
        }
      />

      {tickets.isLoading ? (
        <BoardSkeleton />
      ) : data.length === 0 ? (
        <EmptyState
          icon={Wrench}
          title={
            status === "all"
              ? "No work orders yet"
              : `No tickets in “${humanize(status)}”`
          }
          description={
            status === "all"
              ? "Open a maintenance ticket to track repairs and turns against a property."
              : "Try a different status filter, or open a new ticket."
          }
          action={
            canManage ? (
              <Button onClick={() => setCreateOpen(true)}>
                <Plus className="h-4 w-4" />
                New ticket
              </Button>
            ) : undefined
          }
        />
      ) : (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
          {columns.map((col) => (
            <BoardColumn
              key={col.key}
              statusKey={col.key}
              items={col.items}
              propName={propName}
              members={members.data ?? []}
              entities={entities.data ?? []}
              onOpen={setActiveId}
            />
          ))}
        </div>
      )}

      <CreateTicketDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        properties={(properties.data ?? []).map((p) => ({
          id: p.id,
          name: p.name,
        }))}
      />

      <TicketDetailDialog
        ticketId={activeId}
        onOpenChange={(open) => {
          if (!open) setActiveId(null);
        }}
        canManage={canManage}
        propName={propName}
      />
    </div>
  );
}

// ---- Board column ----------------------------------------------------------

function BoardColumn({
  statusKey,
  items,
  propName,
  members,
  entities,
  onOpen,
}: {
  statusKey: string;
  items: MaintenanceTicket[];
  propName: (id: string) => string;
  members: Member[];
  entities: Counterparty[];
  onOpen: (id: string) => void;
}) {
  const assigneeName = (t: MaintenanceTicket): string | null => {
    if (t.assignee_user_id) {
      return members.find((m) => m.user_id === t.assignee_user_id)?.name ?? null;
    }
    if (t.assignee_entity_id) {
      return entities.find((e) => e.id === t.assignee_entity_id)?.name ?? null;
    }
    return null;
  };

  return (
    <div className="rounded-xl border border-line bg-surface-2/40">
      <div className="flex items-center justify-between gap-2 px-4 py-3">
        <div className="flex items-center gap-2">
          <CircleDot
            className={cn(
              "h-3.5 w-3.5",
              statusKey === "resolved" || statusKey === "closed"
                ? "text-good"
                : statusKey === "in_progress" || statusKey === "scheduled"
                  ? "text-info"
                  : statusKey === "on_hold"
                    ? "text-warn"
                    : "text-ink-3"
            )}
          />
          <span className="font-display text-sm font-semibold text-ink">
            {humanize(statusKey)}
          </span>
        </div>
        <span
          data-numeric
          className="rounded-full bg-surface px-2 py-0.5 text-xs font-semibold text-ink-2"
        >
          {items.length}
        </span>
      </div>

      <div className="space-y-2 p-3 pt-0">
        {items.length === 0 ? (
          <div className="rounded-lg border border-dashed border-line px-3 py-6 text-center text-xs text-ink-3">
            No tickets
          </div>
        ) : (
          items.map((t) => (
            <TicketCard
              key={t.id}
              ticket={t}
              propertyName={propName(t.property_id)}
              assignee={assigneeName(t)}
              onOpen={onOpen}
            />
          ))
        )}
      </div>
    </div>
  );
}

function TicketCard({
  ticket,
  propertyName,
  assignee,
  onOpen,
}: {
  ticket: MaintenanceTicket;
  propertyName: string;
  assignee: string | null;
  onOpen: (id: string) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onOpen(ticket.id)}
      className="w-full rounded-lg border border-line bg-surface p-3 text-left shadow-acre transition hover:border-line-2 hover:bg-surface-2/50 focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
    >
      <div className="flex items-start justify-between gap-2">
        <p className="line-clamp-2 text-sm font-semibold text-ink">
          {ticket.title}
        </p>
        <Badge tone={priorityTone(ticket.priority)}>{ticket.priority}</Badge>
      </div>
      <p className="mt-1 truncate text-xs text-ink-3">
        {propertyName}
        {ticket.category ? ` · ${humanize(ticket.category)}` : ""}
      </p>
      <div className="mt-2.5 flex items-center justify-between gap-2 text-xs text-ink-3">
        <span className="flex min-w-0 items-center gap-1.5">
          <UserIcon className="h-3.5 w-3.5 shrink-0" />
          <span className="truncate">{assignee ?? "Unassigned"}</span>
        </span>
        <span className="shrink-0" data-numeric>
          {relativeDate(ticket.created_at)}
        </span>
      </div>
    </button>
  );
}

function BoardSkeleton() {
  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
      {BOARD_STATUSES.map((s) => (
        <div key={s} className="rounded-xl border border-line bg-surface-2/40">
          <div className="px-4 py-3">
            <Skeleton className="h-4 w-24 rounded" />
          </div>
          <div className="space-y-2 p-3 pt-0">
            {Array.from({ length: 2 }).map((_, i) => (
              <Skeleton key={i} className="h-[88px] w-full rounded-lg" />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

// ---- Create dialog ---------------------------------------------------------

const createSchema = z.object({
  property_id: z.string().min(1, "Pick a property"),
  title: z.string().min(2, "Add a short title"),
  description: z.string().optional(),
  category: z.string().min(1),
  priority: z.string().min(1),
  reporter: z.string().optional(),
});
type CreateValues = z.infer<typeof createSchema>;

function CreateTicketDialog({
  open,
  onOpenChange,
  properties,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  properties: { id: string; name: string }[];
}) {
  const {
    register,
    handleSubmit,
    reset,
    watch,
    setValue,
    formState: { errors },
  } = useForm<CreateValues>({
    resolver: zodResolver(createSchema),
    defaultValues: {
      property_id: "",
      title: "",
      description: "",
      category: "general",
      priority: "normal",
      reporter: "",
    },
  });

  const propertyId = watch("property_id");
  const category = watch("category");
  const priority = watch("priority");
  const create = useCreateTicket(propertyId);

  const onSubmit = handleSubmit((values) => {
    create.mutate(
      {
        title: values.title,
        description: values.description || undefined,
        category: values.category,
        priority: values.priority,
        reporter: values.reporter || undefined,
      },
      {
        onSuccess: () => {
          reset();
          onOpenChange(false);
        },
      }
    );
  });

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) reset();
        onOpenChange(next);
      }}
    >
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>New work order</DialogTitle>
        </DialogHeader>

        <form onSubmit={onSubmit} className="space-y-4">
          <Field label="Property" required error={errors.property_id?.message}>
            <Select
              value={propertyId}
              onValueChange={(v) =>
                setValue("property_id", v, { shouldValidate: true })
              }
            >
              <SelectTrigger aria-label="Property">
                <SelectValue placeholder="Select a property" />
              </SelectTrigger>
              <SelectContent>
                {properties.length === 0 ? (
                  <SelectItem value="__none" disabled>
                    No properties available
                  </SelectItem>
                ) : (
                  properties.map((p) => (
                    <SelectItem key={p.id} value={p.id}>
                      {p.name}
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>
          </Field>

          <Field label="Title" required error={errors.title?.message}>
            <Input
              {...register("title")}
              placeholder="e.g. Kitchen faucet leaking"
              error={!!errors.title}
            />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field label="Category">
              <Select
                value={category}
                onValueChange={(v) => setValue("category", v)}
              >
                <SelectTrigger aria-label="Category">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {CATEGORIES.map((c) => (
                    <SelectItem key={c} value={c}>
                      {humanize(c)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Priority">
              <Select
                value={priority}
                onValueChange={(v) => setValue("priority", v)}
              >
                <SelectTrigger aria-label="Priority">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PRIORITIES.map((p) => (
                    <SelectItem key={p} value={p}>
                      {humanize(p)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
          </div>

          <Field label="Reporter" hint="Who reported this issue (optional)">
            <Input {...register("reporter")} placeholder="e.g. Resident" />
          </Field>

          <Field label="Description">
            <Textarea
              {...register("description")}
              placeholder="Add any details for the assignee…"
            />
          </Field>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={create.isPending}>
              {create.isPending && (
                <Loader2 className="h-4 w-4 animate-spin" />
              )}
              Create ticket
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ---- Detail dialog ---------------------------------------------------------

function TicketDetailDialog({
  ticketId,
  onOpenChange,
  canManage,
  propName,
}: {
  ticketId: string | null;
  onOpenChange: (open: boolean) => void;
  canManage: boolean;
  propName: (id: string) => string;
}) {
  return (
    <Dialog open={!!ticketId} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        {ticketId ? (
          <TicketDetailBody
            ticketId={ticketId}
            canManage={canManage}
            propName={propName}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

function TicketDetailBody({
  ticketId,
  canManage,
  propName,
}: {
  ticketId: string;
  canManage: boolean;
  propName: (id: string) => string;
}) {
  const ticket = useTicket(ticketId);
  const update = useUpdateTicket(ticketId);
  const addComment = useAddTicketComment(ticketId);
  const [comment, setComment] = useState("");

  if (ticket.isLoading || !ticket.data) {
    return (
      <div className="space-y-4">
        <DialogHeader>
          <DialogTitle>
            <Skeleton className="h-5 w-48 rounded" />
          </DialogTitle>
        </DialogHeader>
        <Skeleton className="h-20 w-full rounded-lg" />
        <Skeleton className="h-24 w-full rounded-lg" />
      </div>
    );
  }

  const t = ticket.data;

  const submitComment = () => {
    const body = comment.trim();
    if (!body) return;
    addComment.mutate(body, { onSuccess: () => setComment("") });
  };

  return (
    <div className="space-y-5">
      <DialogHeader>
        <DialogTitle className="pr-6">{t.title}</DialogTitle>
        <div className="flex flex-wrap items-center gap-2 pt-1">
          <Badge tone={statusToneFor(t.status)}>{humanize(t.status)}</Badge>
          <Badge tone={priorityTone(t.priority)}>{t.priority}</Badge>
          <span className="text-xs text-ink-3">{propName(t.property_id)}</span>
        </div>
      </DialogHeader>

      {t.description && (
        <p className="rounded-lg bg-surface-2/60 p-3 text-sm text-ink-2">
          {t.description}
        </p>
      )}

      <dl className="grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
        <Detail label="Category" value={humanize(t.category)} />
        <Detail label="Reporter" value={t.reporter ?? "—"} />
        <Detail
          label="Cost"
          value={t.cost_label ?? "—"}
          numeric={!!t.cost_label}
        />
        <Detail
          label="Due"
          value={t.due_date ? formatDateTime(t.due_date) : "—"}
        />
        <Detail label="Opened" value={formatDateTime(t.created_at)} />
        <Detail label="Updated" value={relativeDate(t.updated_at)} />
      </dl>

      {canManage && (
        <div className="space-y-1.5">
          <span className="block text-xs font-semibold text-ink-2">
            Update status
          </span>
          <Select
            value={t.status}
            onValueChange={(next) => update.mutate({ status: next })}
          >
            <SelectTrigger
              aria-label="Update status"
              className="w-full sm:w-56"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {ALL_STATUSES.map((s) => (
                <SelectItem key={s} value={s}>
                  {humanize(s)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}

      <div className="space-y-3 border-t border-line pt-4">
        <div className="flex items-center gap-2 text-sm font-semibold text-ink">
          <MessageSquare className="h-4 w-4 text-ink-3" />
          Comments
          <span data-numeric className="text-ink-3">
            ({t.comments.length})
          </span>
        </div>

        {t.comments.length === 0 ? (
          <p className="text-sm text-ink-3">No comments yet.</p>
        ) : (
          <ul className="space-y-2.5">
            {t.comments.map((c) => (
              <li
                key={c.id}
                className="rounded-lg border border-line bg-surface-2/40 p-3"
              >
                <p className="whitespace-pre-wrap text-sm text-ink">{c.body}</p>
                <p className="mt-1 text-xs text-ink-3">
                  {relativeDate(c.created_at)}
                </p>
              </li>
            ))}
          </ul>
        )}

        {canManage && (
          <div className="space-y-2">
            <Textarea
              value={comment}
              onChange={(e) => setComment(e.target.value)}
              placeholder="Add a comment…"
              className="min-h-[72px]"
            />
            <div className="flex justify-end">
              <Button
                size="sm"
                onClick={submitComment}
                disabled={!comment.trim() || addComment.isPending}
              >
                {addComment.isPending && (
                  <Loader2 className="h-4 w-4 animate-spin" />
                )}
                Comment
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function Detail({
  label,
  value,
  numeric,
}: {
  label: string;
  value: string;
  numeric?: boolean;
}) {
  return (
    <div>
      <dt className="text-xs font-semibold uppercase tracking-wide text-ink-3">
        {label}
      </dt>
      <dd
        className={cn("mt-0.5 text-ink", numeric && "tabular-nums")}
        data-numeric={numeric || undefined}
      >
        {value}
      </dd>
    </div>
  );
}
