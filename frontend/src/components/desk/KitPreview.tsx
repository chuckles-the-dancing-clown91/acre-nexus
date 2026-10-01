// What a job kit puts on a work order: its tasks by trade (contractor work
// flagged), its parts with typical costs, and the estimate.

import { HardHat } from "lucide-react";
import { dollars, minutesLabel, tradeLabel, type Kit } from "@/lib/servicedesk";
import { Badge } from "@/components/ui/badge";

export function KitPreview({ kit }: { kit: Kit }) {
  return (
    <div className="space-y-5">
      {kit.description && (
        <p className="text-[13px] text-fg-2">{kit.description}</p>
      )}
      <div className="grid grid-cols-3 gap-2 text-center">
        <Figure label="Labor" value={dollars(kit.est_labor_cents)} />
        <Figure label="Parts" value={dollars(kit.est_parts_cents)} />
        <Figure label="Estimate" value={kit.est_total_label} strong />
      </div>
      {kit.tasks.length > 0 && (
        <div>
          <div className="eyebrow mb-2">Tasks</div>
          <ol className="space-y-1.5">
            {kit.tasks.map((t, i) => (
              <li
                key={`${t.title}-${i}`}
                className="flex items-center gap-2 text-[13px]"
              >
                <span className="figure w-5 shrink-0 text-right text-fg-4">
                  {i + 1}
                </span>
                <span className="min-w-0 flex-1 truncate text-fg">
                  {t.title}
                </span>
                {t.needs_contractor && (
                  <HardHat
                    className="size-3.5 shrink-0 text-warn"
                    aria-label="Needs a contractor"
                  />
                )}
                <Badge>{tradeLabel(t.trade)}</Badge>
                <span className="w-12 shrink-0 text-right text-xs text-fg-3">
                  {minutesLabel(t.est_minutes)}
                </span>
              </li>
            ))}
          </ol>
        </div>
      )}
      {kit.parts.length > 0 && (
        <div>
          <div className="eyebrow mb-2">Parts</div>
          <ul className="space-y-1 text-[13px]">
            {kit.parts.map((p, i) => (
              <li key={`${p.name}-${i}`} className="flex gap-2">
                <span className="figure w-8 shrink-0 text-right text-fg-3">
                  {p.quantity}×
                </span>
                <span className="min-w-0 flex-1 truncate text-fg-2">
                  {p.name}
                </span>
                {p.unit_cost_cents != null && (
                  <span className="figure text-fg-3">
                    {dollars(p.unit_cost_cents * p.quantity)}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function Figure({
  label,
  value,
  strong,
}: {
  label: string;
  value: string;
  strong?: boolean;
}) {
  return (
    <div className="rounded-xl border border-line bg-fill/50 px-2 py-2.5">
      <div className="text-[11px] text-fg-3">{label}</div>
      <div
        className={
          strong
            ? "figure text-[17px] font-semibold text-accent"
            : "figure text-[15px] font-medium text-fg"
        }
      >
        {value}
      </div>
    </div>
  );
}
