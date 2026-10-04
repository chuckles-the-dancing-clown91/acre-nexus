"use client";

// A guest's stay, opened from the link in their email: dates, site, what's
// owed, and a way to cancel before arrival.

import { useParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { publicCamp, STATUS_WORDS, statusTone } from "@/lib/campground";
import { usd } from "@/lib/format";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function GuestStayPage() {
  const { token } = useParams<{ token: string }>();
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["guest-stay", token],
    queryFn: () => publicCamp.guest(token),
    retry: false,
  });
  const cancel = useMutation({
    mutationFn: () => publicCamp.cancel(token),
    onSuccess: (d) => {
      qc.setQueryData(["guest-stay", token], d);
      toast.success("Your stay is cancelled");
    },
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Couldn't cancel"),
  });
  const d = q.data;
  return (
    <div className="min-h-dvh bg-bg">
      <main className="mx-auto max-w-xl space-y-5 px-4 py-8">
        {q.isLoading && <Skeleton className="h-56 rounded-2xl" />}
        {q.error && (
          <Panel className="p-6 text-center text-[14px] text-fg-2">
            This link doesn&apos;t match a stay.
          </Panel>
        )}
        {d && (
          <>
            <div className="space-y-1">
              <div className="eyebrow">{d.campground}</div>
              <h1 className="flex flex-wrap items-center gap-2 text-[24px] font-semibold text-fg">
                {d.stay.site_name}
                <Badge tone={statusTone(d.stay.status)}>
                  {d.stay.status === "held"
                    ? "Waiting to be confirmed"
                    : STATUS_WORDS[d.stay.status]}
                </Badge>
              </h1>
            </div>
            <Panel className="p-5">
              <dl className="grid grid-cols-2 gap-4 text-[14px]">
                <div>
                  <dt className="text-[12px] text-fg-3">Arrive</dt>
                  <dd className="font-medium text-fg">{d.stay.check_in}</dd>
                  <dd className="text-[12px] text-fg-3">
                    after {d.check_in_time}
                  </dd>
                </div>
                <div>
                  <dt className="text-[12px] text-fg-3">Leave</dt>
                  <dd className="font-medium text-fg">{d.stay.check_out}</dd>
                  <dd className="text-[12px] text-fg-3">
                    by {d.check_out_time}
                  </dd>
                </div>
                <div>
                  <dt className="text-[12px] text-fg-3">Guests</dt>
                  <dd className="font-medium text-fg">{d.stay.guests}</dd>
                </div>
                <div>
                  <dt className="text-[12px] text-fg-3">Total</dt>
                  <dd className="font-medium text-fg">
                    {usd(d.stay.total_cents)}
                  </dd>
                  {d.stay.balance_cents > 0 && (
                    <dd className="text-[12px] text-fg-3">
                      {usd(d.stay.balance_cents)} still owed
                    </dd>
                  )}
                </div>
              </dl>
            </Panel>
            {d.policies && (
              <p className="text-[13px] whitespace-pre-line text-fg-3">
                {d.policies}
              </p>
            )}
            {d.can_cancel && (
              <Button
                variant="ghost"
                loading={cancel.isPending}
                onClick={() => {
                  if (window.confirm("Cancel this stay?")) cancel.mutate();
                }}
              >
                Cancel my stay
              </Button>
            )}
          </>
        )}
      </main>
    </div>
  );
}
