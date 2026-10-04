"use client";

// One work order: its status, what it's costing against the estimate, the
// tasks (with vendors for contractor work), parts, notes and photos, and
// expenses with receipts.

import { useCallback, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  ArrowLeft,
  ArrowRightLeft,
  Camera,
  ClipboardPlus,
  Clock,
  HardHat,
  MapPin,
  MessageSquare,
  Plus,
  Receipt,
  Wrench,
} from "lucide-react";
import { api, ApiError } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { desk, dollars, money, tradeLabel } from "@/lib/servicedesk";
import { STATUS_WORDS } from "@/lib/ticketFlow";
import { Expenses } from "@/components/desk/Expenses";
import { Assign } from "@/components/desk/Assign";
import {
  ActionWizard,
  type WizardAction,
} from "@/components/desk/ActionWizard";
import { Feed } from "@/components/desk/Feed";
import { TimePanel } from "@/components/desk/TimePanel";
import { Parts } from "@/components/desk/Parts";
import { OwnerApprovalPanel } from "@/components/desk/OwnerApproval";
import { Schedule } from "@/components/desk/Schedule";
import { TicketActions } from "@/components/desk/TicketActions";
import { TaskList } from "@/components/desk/TaskList";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const rise = (i: number) => ({
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0 },
  transition: {
    duration: 0.4,
    delay: i * 0.04,
    ease: [0.22, 1, 0.36, 1] as const,
  },
});

export default function WorkOrderPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const qc = useQueryClient();
  const notMissing = (n: number, e: Error) =>
    !(e instanceof ApiError && e.status < 500) && n < 2;

  const ticket = useQuery({
    queryKey: ["ticket", id],
    queryFn: () => api.ticket(id),
    retry: notMissing,
  });
  const ok = ticket.isSuccess;
  const tasks = useQuery({
    queryKey: ["tasks", id],
    queryFn: () => desk.tasks(id),
    enabled: ok,
  });
  const costs = useQuery({
    queryKey: ["costs", id],
    queryFn: () => desk.costs(id),
    enabled: ok,
  });
  const files = useQuery({
    queryKey: ["files", id],
    queryFn: () => desk.files(id),
    enabled: ok,
  });
  const expenses = useQuery({
    queryKey: ["expenses", id],
    queryFn: () => desk.expenses(id),
    enabled: ok,
  });
  const property = useQuery({
    queryKey: ["properties", ticket.data?.property_id],
    queryFn: () => api.property(ticket.data!.property_id),
    enabled: ok,
  });

  const [wizard, setWizard] = useState<{
    action?: WizardAction;
    taskId?: string;
  } | null>(null);
  const units = useQuery({
    queryKey: ["properties", ticket.data?.property_id, "units"],
    queryFn: () => api.units(ticket.data!.property_id),
    enabled: ok && !!ticket.data?.unit_id,
  });

  const refresh = useCallback(() => {
    for (const k of [
      "ticket",
      "tasks",
      "costs",
      "files",
      "expenses",
      "feed",
      "time",
    ]) {
      void qc.invalidateQueries({ queryKey: [k, id] });
    }
    void qc.invalidateQueries({ queryKey: ["tickets"] });
    void qc.invalidateQueries({ queryKey: ["techs"] });
    void qc.invalidateQueries({ queryKey: ["my-queue"] });
    void qc.invalidateQueries({ queryKey: ["appointments"] });
  }, [qc, id]);

  if (ticket.error) {
    const missing =
      ticket.error instanceof ApiError && ticket.error.status === 404;
    return (
      <Panel className="mx-auto mt-10 max-w-lg">
        <EmptyState
          icon={<Wrench />}
          title={
            missing ? "This work order isn't in your view" : "Couldn't load it"
          }
          description={
            missing
              ? "It may be on a property you aren't assigned to."
              : ticket.error.message
          }
          action={
            <Button variant="secondary" asChild>
              <Link href="/console/maintenance">
                <ArrowLeft />
                Service desk
              </Link>
            </Button>
          }
        />
      </Panel>
    );
  }

  const t = ticket.data;
  const c = costs.data;
  const uncovered = c?.trades_needed.filter((n) => !n.covered) ?? [];

  const unit = units.data?.find((u) => u.id === t?.unit_id);

  return (
    <div className="space-y-6">
      <Link
        href="/console/maintenance"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Service desk
      </Link>

      <motion.header
        {...rise(0)}
        className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between"
      >
        <div className="min-w-0">
          {t ? (
            <>
              <div className="mb-2 flex flex-wrap gap-2">
                <Badge
                  tone={
                    t.priority === "urgent"
                      ? "bad"
                      : t.priority === "high"
                        ? "warn"
                        : "neutral"
                  }
                >
                  {t.priority}
                </Badge>
                <Badge>{tradeLabel(t.category)}</Badge>
                {t.waiting_on && (
                  <Badge tone="warn">waiting on {t.waiting_on}</Badge>
                )}
              </div>
              <h1 className="text-[26px] leading-tight font-semibold text-fg sm:text-[30px]">
                {t.title}
              </h1>
              <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] text-fg-3">
                {property.data && (
                  <Link
                    href={`/console/properties/${t.property_id}`}
                    className="inline-flex items-center gap-1 hover:text-fg"
                  >
                    <MapPin className="size-3.5" />
                    {property.data.name}
                  </Link>
                )}
                {unit && (
                  <Link
                    href={`/console/properties/${t.property_id}/units/${unit.id}`}
                    className="hover:text-fg"
                  >
                    Unit {unit.unit_number}
                  </Link>
                )}
                {t.location && <span>{t.location}</span>}
                {t.reporter && <span>From {t.reporter}</span>}
                <span>
                  Opened {new Date(t.created_at).toLocaleDateString()}
                </span>
              </div>
            </>
          ) : (
            <Skeleton className="h-16 w-80" />
          )}
        </div>
        {t && (
          <div className="flex shrink-0 items-center gap-2">
            <Badge tone={statusTone(t.status)} className="h-8 px-3 text-[13px]">
              {STATUS_WORDS[t.status] ?? t.status.replace("_", " ")}
            </Badge>
            {manage && (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setWizard({ action: "status" })}
              >
                <ArrowRightLeft />
                Change status
              </Button>
            )}
          </div>
        )}
      </motion.header>

      {t && manage && (
        <motion.div {...rise(1)} className="flex flex-wrap items-center gap-2">
          <Button size="sm" onClick={() => setWizard({ action: "note" })}>
            <MessageSquare />
            Add note
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setWizard({ action: "media" })}
          >
            <Camera />
            Photo or video
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setWizard({ action: "expense" })}
          >
            <Receipt />
            Expense
          </Button>
          {t.track_time !== false && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setWizard({ action: "time" })}
            >
              <Clock />
              Time
            </Button>
          )}
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setWizard({ action: "task" })}
          >
            <ClipboardPlus />
            Task
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setWizard({})}>
            <Plus />
            More
          </Button>
        </motion.div>
      )}

      {t && manage && (
        <motion.div {...rise(1)}>
          <TicketActions ticketId={id} status={t.status} onChange={refresh} />
        </motion.div>
      )}

      {uncovered.length > 0 && (
        <Panel className="flex flex-wrap items-center gap-3 border-warn/30 p-4 text-[13px]">
          <HardHat className="size-5 text-warn" />
          <span className="text-fg">Needs a contractor:</span>
          {uncovered.map((n) => (
            <Badge key={n.trade} tone="warn">
              {tradeLabel(n.trade)} · {n.open_tasks}{" "}
              {n.open_tasks === 1 ? "task" : "tasks"}
            </Badge>
          ))}
          <span className="text-fg-3">Open a task to send it to a vendor.</span>
        </Panel>
      )}

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)]">
        <div className="space-y-4">
          {t?.description && (
            <Panel className="p-4 text-[13px] whitespace-pre-wrap text-fg-2">
              {t.description}
            </Panel>
          )}
          {tasks.data ? (
            <TaskList
              ticketId={id}
              propertyId={t?.property_id}
              tasks={tasks.data}
              manage={manage}
              onChange={refresh}
              onNote={(taskId) => setWizard({ action: "note", taskId })}
            />
          ) : (
            <Skeleton className="h-64 rounded-2xl" />
          )}

          <Panel>
            <PanelHeader
              title="Feed"
              description="Everything that's happened, newest first."
              action={
                manage && (
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => setWizard({})}
                  >
                    <Plus />
                    Add
                  </Button>
                )
              }
            />
            <div className="p-5 pt-3">
              <Feed
                ticketId={id}
                onOpenTask={(taskId) => setWizard({ action: "note", taskId })}
              />
            </div>
          </Panel>
        </div>

        <div className="space-y-4">
          {t && (
            <motion.div {...rise(1)} className="space-y-4">
              <Assign ticket={t} manage={manage} onChange={refresh} />
              <Schedule ticket={t} manage={manage} onChange={refresh} />
              <OwnerApprovalPanel
                ticketId={id}
                manage={manage}
                onChange={refresh}
              />
            </motion.div>
          )}

          <section className="grid grid-cols-2 gap-3">
            <Figure
              label="Estimate"
              value={c ? c.est_total_label : "—"}
              hint={
                c
                  ? `${dollars(c.est_labor_cents)} labor · ${dollars(c.est_parts_cents)} parts`
                  : ""
              }
            />
            <Figure
              label="Spent"
              value={c ? money(c.actual_total_cents) : "—"}
              hint={
                c
                  ? `${money(c.expenses_cents)} expenses · ${c.receipts} ${c.receipts === 1 ? "receipt" : "receipts"}`
                  : ""
              }
            />
            <Figure
              label="Against estimate"
              value={c ? c.variance_label : "—"}
              tone={
                c && c.variance_cents > 0 && c.est_total_cents > 0
                  ? "bad"
                  : "good"
              }
            />
            <Figure
              label="Tasks"
              value={c ? `${c.tasks_done}/${c.tasks_total}` : "—"}
              hint={
                c && c.tasks_total
                  ? `${Math.round((c.tasks_done * 100) / c.tasks_total)}% done`
                  : "No tasks yet"
              }
            />
          </section>

          {t && (
            <TimePanel
              ticketId={id}
              tracking={t.track_time !== false}
              manage={manage}
              onLog={() => setWizard({ action: "time" })}
              onChange={refresh}
            />
          )}

          {t && <Parts parts={t.parts} manage={manage} onChange={refresh} />}

          {expenses.data && files.data && (
            <Expenses
              ticketId={id}
              expenses={expenses.data}
              files={files.data}
              manage={manage}
              onChange={refresh}
            />
          )}
        </div>
      </div>

      {wizard && t && (
        <ActionWizard
          ticket={t}
          tasks={tasks.data ?? []}
          initial={wizard.action}
          taskId={wizard.taskId}
          trackTime={t.track_time !== false}
          onClose={() => setWizard(null)}
          onDone={refresh}
        />
      )}
    </div>
  );
}

function Figure({
  label,
  value,
  hint,
  tone,
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: "good" | "bad";
}) {
  return (
    <Panel className="flex h-full flex-col p-4">
      <div className="eyebrow">{label}</div>
      <div
        className={cn(
          "figure mt-2 text-[22px] leading-none font-semibold",
          tone === "bad" ? "text-bad" : tone === "good" ? "text-fg" : "text-fg"
        )}
      >
        {value}
      </div>
      {hint && <div className="mt-2 text-xs text-fg-3">{hint}</div>}
    </Panel>
  );
}
