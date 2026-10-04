"use client";

// One job in the vendor portal: the same screen the emailed link opens.

import { useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { portalClient, type VendorJob } from "@/lib/vendorLink";
import { Job } from "@/components/vendor/JobView";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function VendorJobPage() {
  const { batch } = useParams<{ batch: string }>();
  const qc = useQueryClient();
  const client = portalClient(batch);
  const q = useQuery({
    queryKey: ["vendor-portal", "job", batch],
    queryFn: () => client.view(),
    retry: false,
  });
  const [job, setJob] = useState<VendorJob | null>(null);
  const view = job ?? q.data;
  return (
    <div className="space-y-4">
      <Link
        href="/account/vendor"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        All jobs
      </Link>
      {q.isLoading && <Skeleton className="h-64" />}
      {q.error && (
        <Panel>
          <EmptyState
            title="Job not found"
            description="It may have been sent again or taken back."
          />
        </Panel>
      )}
      {view && (
        <Job
          client={client}
          job={view}
          onChange={(j) => {
            setJob(j);
            void qc.invalidateQueries({ queryKey: ["vendor-portal", "jobs"] });
          }}
        />
      )}
    </div>
  );
}
