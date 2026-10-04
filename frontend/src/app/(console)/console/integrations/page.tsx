"use client";

// Integrations: how this workspace connects to the outside. The business
// profile and its Google reviews, widgets for your own website, how the site
// reads in search, single sign-on with Alpha, vendors linked to Alpha, stored
// documents, and the log of email and texts sent.

import { Suspense } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { KeyRound, Plug } from "lucide-react";
import { useAuth } from "@/lib/auth";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { BusinessTab, ReviewsTab } from "./business";
import { SearchTab, WidgetsTab } from "./website";
import { PartnersTab, SsoTab } from "./alpha";
import { DocumentsTab, SentTab } from "./records";

type Key =
  | "business"
  | "reviews"
  | "widgets"
  | "search"
  | "sso"
  | "partners"
  | "documents"
  | "sent";

const ALL: { key: Key; label: string; perm: string }[] = [
  { key: "business", label: "Business profile", perm: "integrations:manage" },
  { key: "reviews", label: "Google reviews", perm: "integrations:manage" },
  { key: "widgets", label: "Website widgets", perm: "integrations:manage" },
  { key: "search", label: "Search", perm: "integrations:manage" },
  { key: "sso", label: "Single sign-on", perm: "integrations:manage" },
  { key: "partners", label: "Alpha vendors", perm: "entity:read" },
  { key: "documents", label: "Documents", perm: "document:read" },
  { key: "sent", label: "Sent log", perm: "integrations:manage" },
];

export default function IntegrationsPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <Integrations />
    </Suspense>
  );
}

function Integrations() {
  const { can } = useAuth();
  const params = useSearchParams();
  const router = useRouter();
  const tabs = ALL.filter((t) => can(t.perm));
  const asked = params.get("tab");
  const tab = (tabs.find((t) => t.key === asked) ?? tabs[0])?.key;
  const choose = (k: Key) =>
    router.replace(`/console/integrations?tab=${k}`, { scroll: false });

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="Integrations"
        description="Your business on Google, widgets for your own website, and the systems you connect to."
        actions={
          can("integrations:manage") && (
            <Button asChild variant="secondary">
              <Link href="/console/settings">
                <KeyRound />
                API keys
              </Link>
            </Button>
          )
        }
      />
      {!tab ? (
        <Panel>
          <EmptyState
            icon={<Plug />}
            title="Nothing here for you"
            description="Ask a company owner for the integrations:manage permission."
          />
        </Panel>
      ) : (
        <>
          <Tabs
            tabs={tabs.map((t) => [t.key, t.label] as const)}
            value={tab}
            onChange={choose}
          />
          {tab === "business" && <BusinessTab />}
          {tab === "reviews" && <ReviewsTab />}
          {tab === "widgets" && <WidgetsTab />}
          {tab === "search" && <SearchTab />}
          {tab === "sso" && <SsoTab />}
          {tab === "partners" && <PartnersTab />}
          {tab === "documents" && <DocumentsTab />}
          {tab === "sent" && <SentTab />}
        </>
      )}
    </div>
  );
}
