"use client";

// The resident's ID card, sized for a phone. Show it at the office or the
// gate; the code ties back to their profile.

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { IdCard as IdIcon } from "lucide-react";
import { api, ApiError } from "@/lib/api";
import { IdCardView } from "@/components/resident/IdCardView";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function IdPage() {
  const q = useQuery({
    queryKey: ["my-resident"],
    queryFn: api.myResident,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });
  const missing = q.data?.prefill.missing ?? [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">My ID</h1>
        <p className="text-[13px] text-fg-3">
          Your resident card. It is built from your profile.
        </p>
      </div>
      {q.isLoading && <Skeleton className="mx-auto h-80 max-w-[380px]" />}
      {q.error && (
        <Panel>
          <EmptyState
            icon={<IdIcon />}
            title="Couldn't load your ID"
            description={q.error.message}
          />
        </Panel>
      )}
      {q.data && (
        <>
          <IdCardView card={q.data.card} />
          {missing.length > 0 && (
            <Panel className="mx-auto max-w-[380px] space-y-2 p-4 text-[13px]">
              <div className="font-medium text-fg">Finish your profile</div>
              <p className="text-fg-3">
                Still empty: {missing.join(", ")}. Applications use these.
              </p>
              <Button asChild size="sm" variant="secondary">
                <Link href="/account/profile">Open profile</Link>
              </Button>
            </Panel>
          )}
        </>
      )}
    </div>
  );
}
