"use client";

// A resident's repairs: what they've asked for and where each stands. Links
// from texts and emails open straight into a request (`?ticket=`) or a new
// one already filled in (`?new=1&title=..`).

import { Suspense } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { ChevronRight, Plus, Wrench } from "lucide-react";
import { api, ApiError } from "@/lib/api";
import { prefillFrom, residentStatus } from "@/lib/resident";
import { NewRequest } from "@/components/account/NewRequest";
import { RequestView } from "@/components/account/RequestView";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function RepairsPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64" />}>
      <Repairs />
    </Suspense>
  );
}

function Repairs() {
  const params = useSearchParams();
  const router = useRouter();
  const path = usePathname();
  const ticket = params.get("ticket");
  const prefill = prefillFrom(new URLSearchParams(params.toString()));

  const list = useQuery({
    queryKey: ["my-tickets"],
    queryFn: api.myTickets,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });

  const go = (q: string) => router.push(q ? `${path}?${q}` : path);

  if (ticket) return <RequestView id={ticket} onBack={() => go("")} />;

  if (prefill)
    return (
      <NewRequest
        prefill={prefill}
        onCancel={() => go("")}
        onCreated={(id) => router.replace(`${path}?ticket=${id}`)}
      />
    );

  if (list.error) {
    const noLease = list.error instanceof ApiError && list.error.status === 404;
    return (
      <Panel>
        <EmptyState
          icon={<Wrench />}
          title={
            noLease ? "No home on file yet" : "Couldn't load your requests"
          }
          description={
            noLease
              ? "Once your lease is set up with this email, your repair requests live here."
              : list.error.message
          }
        />
      </Panel>
    );
  }

  const rows = list.data ?? [];
  const open = rows.filter(
    (t) => !["resolved", "closed", "cancelled"].includes(t.status)
  );
  const past = rows.filter((t) => !open.includes(t));

  return (
    <div className="space-y-6">
      <div className="flex items-end justify-between gap-3">
        <div>
          <h1 className="text-[24px] font-semibold text-fg">Repairs</h1>
          <p className="text-[13px] text-fg-3">
            Ask for a fix and follow it here.
          </p>
        </div>
        <Button onClick={() => go("new=1")}>
          <Plus />
          New request
        </Button>
      </div>

      {list.isLoading && <Skeleton className="h-40" />}

      {list.isSuccess && rows.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Wrench />}
            title="Nothing to fix right now"
            description="When something breaks, send a request with a photo or video and we'll take it from there."
          />
        </Panel>
      )}

      {open.length > 0 && (
        <Group title="Open" rows={open} onOpen={(id) => go(`ticket=${id}`)} />
      )}
      {past.length > 0 && (
        <Group title="Done" rows={past} onOpen={(id) => go(`ticket=${id}`)} />
      )}
    </div>
  );
}

function Group({
  title,
  rows,
  onOpen,
}: {
  title: string;
  rows: {
    id: string;
    title: string;
    status: string;
    waiting_on: string | null;
    created_at: string;
    rating: number | null;
  }[];
  onOpen: (id: string) => void;
}) {
  return (
    <section>
      <div className="eyebrow mb-2">{title}</div>
      <Panel className="divide-y divide-line">
        {rows.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => onOpen(t.id)}
            className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-fill/50"
          >
            <div className="min-w-0 flex-1">
              <div className="truncate text-[14px] font-medium text-fg">
                {t.title}
              </div>
              <div className="text-xs text-fg-3">
                Sent {new Date(t.created_at).toLocaleDateString()}
                {(t.status === "resolved" || t.status === "closed") &&
                  t.rating == null &&
                  " · Rate this repair"}
              </div>
            </div>
            <Badge tone={statusTone(t.status)}>
              {residentStatus(t.status, t.waiting_on)}
            </Badge>
            <ChevronRight className="size-4 text-fg-4" />
          </button>
        ))}
      </Panel>
    </section>
  );
}
