// A plain table in the console's style: header row, body, optional totals.
// Cells are already formatted; numbers line up on the right.

import { cn } from "@/lib/utils";

export interface Column {
  label: string;
  /** Right-align (money, counts). */
  num?: boolean;
}

export function DataTable({
  columns,
  rows,
  totals,
  empty = "Nothing here yet.",
  className,
}: {
  columns: (Column | string)[];
  rows: React.ReactNode[][];
  totals?: React.ReactNode[];
  empty?: string;
  className?: string;
}) {
  const cols = columns.map((c) => (typeof c === "string" ? { label: c } : c));
  return (
    <div className={cn("overflow-x-auto", className)}>
      <table className="w-full text-[13px]">
        <thead>
          <tr className="border-b border-line text-left">
            {cols.map((c) => (
              <th
                key={c.label}
                scope="col"
                className={cn(
                  "eyebrow px-4 py-2.5 font-medium whitespace-nowrap",
                  c.num && "text-right"
                )}
              >
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td
                colSpan={cols.length}
                className="px-4 py-8 text-center text-fg-3"
              >
                {empty}
              </td>
            </tr>
          ) : (
            rows.map((r, i) => (
              <tr
                key={i}
                className="border-b border-line/60 last:border-0 hover:bg-surface-2/60"
              >
                {r.map((cell, j) => (
                  <td
                    key={j}
                    className={cn(
                      "px-4 py-2.5 whitespace-nowrap text-fg-2",
                      j === 0 && "font-medium text-fg",
                      cols[j]?.num && "text-right font-mono tabular-nums"
                    )}
                  >
                    {cell}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
        {totals && rows.length > 0 && (
          <tfoot>
            <tr className="border-t-2 border-line font-semibold text-fg">
              {totals.map((cell, j) => (
                <td
                  key={j}
                  className={cn(
                    "px-4 py-2.5 whitespace-nowrap",
                    cols[j]?.num && "text-right font-mono tabular-nums"
                  )}
                >
                  {cell}
                </td>
              ))}
            </tr>
          </tfoot>
        )}
      </table>
    </div>
  );
}

/** A row of segmented tabs. */
export function Tabs<K extends string>({
  tabs,
  value,
  onChange,
  className,
}: {
  tabs: readonly (readonly [K, string])[];
  value: K;
  onChange: (k: K) => void;
  className?: string;
}) {
  return (
    <div
      role="tablist"
      className={cn(
        "flex max-w-full overflow-x-auto rounded-xl bg-surface-2 p-1",
        className
      )}
    >
      {tabs.map(([k, label]) => (
        <button
          key={k}
          role="tab"
          type="button"
          aria-selected={value === k}
          onClick={() => onChange(k)}
          className={cn(
            "shrink-0 rounded-lg px-3.5 py-1.5 text-[13px] font-medium whitespace-nowrap transition",
            value === k
              ? "bg-surface text-fg shadow-sm"
              : "text-fg-3 hover:text-fg"
          )}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

/** A labelled figure. */
export function Stat({
  label,
  value,
  hint,
  tone,
}: {
  label: string;
  value: React.ReactNode;
  hint?: React.ReactNode;
  tone?: "good" | "warn" | "bad";
}) {
  return (
    <div className="glass rounded-2xl p-4">
      <div className="eyebrow">{label}</div>
      <div
        className={cn(
          "figure mt-1.5 text-[24px] leading-none font-semibold",
          tone === "good"
            ? "text-good"
            : tone === "warn"
              ? "text-warn"
              : tone === "bad"
                ? "text-bad"
                : "text-fg"
        )}
      >
        {value}
      </div>
      {hint && <div className="mt-1.5 text-[12px] text-fg-3">{hint}</div>}
    </div>
  );
}
