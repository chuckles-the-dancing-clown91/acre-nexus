"use client";

// Who has this work order: a person on the team (shown with what they
// already have, people on the property first) or a vendor. "Take it" is one
// press for whoever's looking.

import { useQuery } from "@tanstack/react-query";
import { HardHat, UserCheck, UserMinus } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { desk, loadLabel, roleLabel } from "@/lib/servicedesk";
import type { MaintenanceTicket } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";

export function Assign({
  ticket,
  manage,
  onChange,
}: {
  ticket: MaintenanceTicket;
  manage: boolean;
  onChange: () => void;
}) {
  const { user } = useAuth();
  const techs = useQuery({
    queryKey: ["techs", ticket.property_id],
    queryFn: () => desk.techs(ticket.property_id),
  });
  const mine = !!user && ticket.assignee_user_id === user.id;
  const holder = techs.data?.find((t) => t.user_id === ticket.assignee_user_id);

  async function set(userId: string | null) {
    try {
      await api.updateTicket(
        ticket.id,
        userId ? { assignee_user_id: userId } : { clear_assignee_user: true }
      );
      toast.success(userId ? "Assigned" : "Taken off");
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't assign it");
    }
  }

  return (
    <Panel className="flex flex-wrap items-center gap-3 p-3">
      <span className="flex size-9 shrink-0 items-center justify-center rounded-full bg-accent/15 text-accent">
        {ticket.assignee_entity_id && !ticket.assignee_user_id ? (
          <HardHat className="size-[18px]" />
        ) : (
          <UserCheck className="size-[18px]" />
        )}
      </span>
      <div className="min-w-0 flex-1 basis-48">
        <div className="eyebrow">Assigned to</div>
        <div className="truncate text-[14px] font-medium text-fg">
          {mine
            ? "You"
            : (holder?.name ??
              ticket.assignee_name ??
              (ticket.assignee_user_id ? "A teammate" : "Nobody yet"))}
          {holder && !mine && (
            <span className="font-normal text-fg-3">
              {" "}
              · {roleLabel(holder.role)}
            </span>
          )}
        </div>
        {ticket.assignee_entity_id && !ticket.assignee_user_id && (
          <div className="text-xs text-fg-3">A vendor has this one.</div>
        )}
      </div>
      {manage && (
        <div className="flex flex-wrap items-center gap-2">
          <select
            aria-label="Assign to a teammate"
            value={ticket.assignee_user_id ?? ""}
            onChange={(e) => e.target.value && set(e.target.value)}
            className="max-w-[16rem] rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg"
          >
            <option value="">
              {ticket.assignee_user_id ? "Reassign…" : "Assign to…"}
            </option>
            {techs.data?.map((t) => (
              <option key={t.user_id} value={t.user_id}>
                {t.name} · {loadLabel(t)}
                {t.on_property ? " · on this property" : ""}
              </option>
            ))}
          </select>
          {!mine && user && (
            <Button size="sm" variant="secondary" onClick={() => set(user.id)}>
              Take it
            </Button>
          )}
          {ticket.assignee_user_id && (
            <Button size="sm" variant="ghost" onClick={() => set(null)}>
              <UserMinus />
              Unassign
            </Button>
          )}
        </div>
      )}
    </Panel>
  );
}
