"use client";

// Import & export: moving a portfolio in from another tool, and taking the
// data out.

import { useState } from "react";
import { useAuth } from "@/lib/auth";
import type { Preview } from "@/lib/datatransfer";
import { ExportPanel } from "@/components/data/ExportPanel";
import { History } from "@/components/data/History";
import { ImportWizard } from "@/components/data/ImportWizard";
import { PageHeader } from "@/components/ui/misc";
import { cn } from "@/lib/utils";

type Tab = "import" | "export";

export default function DataPage() {
  const { can } = useAuth();
  const canImport = can("data:import");
  const [tab, setTab] = useState<Tab>(canImport ? "import" : "export");
  const [resume, setResume] = useState<Preview | null>(null);
  const [wizardKey, setWizardKey] = useState(0);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Platform"
        title="Import & export"
        description="Bring a portfolio over from AppFolio, Buildium, Yardi Breeze, Rent Manager, DoorLoop or a spreadsheet, or take everything out as CSV."
      />
      <div className="flex w-fit gap-1 rounded-xl bg-fill p-1" role="tablist">
        {(["import", "export"] as const)
          .filter((t) => t === "export" || canImport)
          .map((t) => (
            <button
              key={t}
              role="tab"
              aria-selected={tab === t}
              onClick={() => setTab(t)}
              className={cn(
                "rounded-lg px-4 py-1.5 text-[13px] font-medium capitalize transition",
                tab === t
                  ? "bg-surface text-fg shadow-sm"
                  : "text-fg-3 hover:text-fg"
              )}
            >
              {t}
            </button>
          ))}
      </div>
      {tab === "import" && canImport && (
        <>
          <ImportWizard
            key={`${wizardKey}-${resume?.batch.id ?? "new"}`}
            resume={resume ?? undefined}
            onClose={() => {
              setResume(null);
              setWizardKey((k) => k + 1);
            }}
          />
          <History
            onResume={(p) => {
              setResume(p);
              window.scrollTo({ top: 0, behavior: "smooth" });
            }}
          />
        </>
      )}
      {tab === "export" && <ExportPanel />}
    </div>
  );
}
