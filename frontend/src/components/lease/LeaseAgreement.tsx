"use client";

// A lease laid out as a real agreement: a title block, numbered articles,
// tables for money and utilities, each addendum on its own page, and the
// signature lines at the end.

import type { LeaseBlock, LeaseSection } from "@/lib/api";
import { cn } from "@/lib/utils";

function factOf(sections: LeaseSection[], label: string): string {
  for (const s of sections)
    for (const b of s.blocks)
      if (b.type === "facts") {
        const row = b.rows.find(([k]) => k === label);
        if (row) return row[1];
      }
  return "";
}

function Blocks({ blocks }: { blocks: LeaseBlock[] }) {
  return (
    <div className="space-y-3">
      {blocks.map((b, i) => {
        switch (b.type) {
          case "p":
            return (
              <p key={i} className="leading-[1.75]">
                {b.text}
              </p>
            );
          case "note":
            return (
              <p
                key={i}
                className="rounded-md border-l-4 border-amber-500 bg-amber-50 px-3 py-2 text-[13px] text-amber-950"
              >
                {b.text}
              </p>
            );
          case "facts":
            return (
              <dl
                key={i}
                className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5 text-[14px]"
              >
                {b.rows.map(([k, v]) => (
                  <div key={k} className="contents">
                    <dt className="text-stone-500">{k}</dt>
                    <dd className="font-medium text-stone-900">{v}</dd>
                  </div>
                ))}
              </dl>
            );
          case "table":
            return (
              <div key={i} className="overflow-x-auto">
                <table className="w-full border-collapse text-[14px]">
                  <thead>
                    <tr className="border-b-2 border-stone-300 text-left text-[11px] tracking-wider text-stone-500 uppercase">
                      {b.head.map((h, n) => (
                        <th
                          key={n}
                          className={cn(
                            "py-1.5 pr-4 font-semibold",
                            n === b.head.length - 1 &&
                              b.head.length === 2 &&
                              "text-right"
                          )}
                        >
                          {h}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {b.rows.map((r, n) => {
                      const total = /^total/i.test(r[0]);
                      return (
                        <tr
                          key={n}
                          className={cn(
                            "border-b border-stone-200",
                            total && "border-t-2 border-stone-400 font-semibold"
                          )}
                        >
                          {r.map((c, k) => (
                            <td
                              key={k}
                              className={cn(
                                "py-2 pr-4",
                                k === r.length - 1 &&
                                  b.head.length === 2 &&
                                  "text-right tabular-nums"
                              )}
                            >
                              {c}
                            </td>
                          ))}
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            );
          case "list":
            return (
              <ul
                key={i}
                className="list-disc space-y-1.5 pl-6 leading-relaxed"
              >
                {b.items.map((it, n) => (
                  <li key={n}>{it}</li>
                ))}
              </ul>
            );
        }
      })}
    </div>
  );
}

export function LeaseAgreement({
  title,
  sections,
  signedBy,
  signedAt,
  className,
}: {
  title: string;
  sections: LeaseSection[];
  signedBy?: string | null;
  signedAt?: string | null;
  className?: string;
}) {
  const landlord = factOf(sections, "Landlord") || "Landlord";
  const resident = factOf(sections, "Resident") || "Resident";
  const premises = factOf(sections, "Premises");
  const articles = sections.filter((s) => s.kind === "article");
  const addenda = sections.filter((s) => s.kind === "addendum");

  return (
    <article
      id="lease-print"
      className={cn(
        "mx-auto w-full max-w-[820px] rounded-lg bg-white px-6 py-10 font-serif text-[15px] text-stone-800 shadow-md ring-1 ring-stone-200 sm:px-14",
        className
      )}
    >
      <header className="border-b-2 border-stone-800 pb-6 text-center">
        <div className="text-[11px] tracking-[0.3em] text-stone-500 uppercase">
          Residential
        </div>
        <h1 className="mt-1 text-[28px] leading-tight font-semibold tracking-wide text-stone-900 uppercase">
          {title.replace(/^residential\s+/i, "")}
        </h1>
        {premises && <p className="mt-2 text-stone-600">{premises}</p>}
      </header>

      <div className="mt-8 space-y-8">
        {articles.map((s, i) => (
          <section key={s.key}>
            <h2 className="mb-3 flex items-baseline gap-3 font-sans text-[13px] font-semibold tracking-wider text-stone-900 uppercase">
              <span className="text-stone-400 tabular-nums">{i + 1}.</span>
              {s.title}
            </h2>
            <Blocks blocks={s.blocks} />
          </section>
        ))}
      </div>

      <section className="mt-12 break-inside-avoid">
        <h2 className="mb-6 font-sans text-[13px] font-semibold tracking-wider text-stone-900 uppercase">
          Signatures
        </h2>
        <div className="grid gap-8 sm:grid-cols-2">
          {[
            ["Landlord", landlord],
            ["Resident", resident],
          ].map(([role, name]) => (
            <div key={role}>
              <div className="flex h-12 items-end border-b border-stone-500 pb-1 font-[cursive] text-[20px] text-stone-700 italic">
                {role === "Resident" && signedBy ? signedBy : ""}
              </div>
              <div className="mt-1.5 flex justify-between text-[12px] text-stone-500">
                <span>
                  {role}: {name}
                </span>
                <span>
                  Date:{" "}
                  {role === "Resident" && signedAt
                    ? signedAt.slice(0, 10)
                    : "__________"}
                </span>
              </div>
            </div>
          ))}
        </div>
      </section>

      {addenda.map((s, i) => (
        <section
          key={s.key}
          className="mt-14 break-before-page border-t-2 border-dashed border-stone-300 pt-10 print:border-0"
        >
          <div className="text-[11px] tracking-[0.3em] text-stone-500 uppercase">
            Addendum {i + 1}
          </div>
          <h2 className="mt-1 mb-4 text-[22px] font-semibold text-stone-900">
            {s.title}
          </h2>
          <Blocks blocks={s.blocks} />
          <div className="mt-8 flex justify-between gap-6 text-[12px] text-stone-500">
            <span className="flex-1 border-t border-stone-400 pt-1">
              Resident initials
            </span>
            <span className="flex-1 border-t border-stone-400 pt-1">
              Landlord initials
            </span>
          </div>
        </section>
      ))}
    </article>
  );
}
