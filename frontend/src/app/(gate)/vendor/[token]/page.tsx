"use client";

// The page a vendor's dispatch link opens: the work order, their tasks, and
// one place to accept (with a time), decline, send photos and the invoice,
// and say it's done. No sign-in; the link is the credential.

import { useState } from "react";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { linkClient, type VendorJob } from "@/lib/vendorLink";
import { Job } from "@/components/vendor/JobView";
import { Panel } from "@/components/ui/panel";
import { Skeleton } from "@/components/ui/misc";

export default function VendorPage() {
  const { token } = useParams<{ token: string }>();
  const q = useQuery({
    queryKey: ["vendor-link", token],
    queryFn: () => linkClient(token).view(),
    retry: false,
  });
  const [job, setJob] = useState<VendorJob | null>(null);
  const view = job ?? q.data;

  return (
    <main className="mx-auto min-h-dvh max-w-lg px-4 py-8">
      {q.isLoading && <Skeleton className="h-64" />}
      {q.error && (
        <Panel className="p-6 text-center">
          <div className="text-[17px] font-semibold text-fg">
            This link isn&apos;t valid any more
          </div>
          <p className="mt-2 text-[13px] text-fg-3">
            The work may have been sent again with a newer link. Check your
            latest message from the property manager.
          </p>
        </Panel>
      )}
      {view && <Job client={linkClient(token)} job={view} onChange={setJob} />}
    </main>
  );
}
