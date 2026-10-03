"use client";

// The land and the money around it: parcel and zoning, flood zone, taxes,
// what it's worth, who holds title, loans and liens, and the utilities.

import { useQuery } from "@tanstack/react-query";
import { Landmark, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { day, label } from "@/lib/propertyRecords";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { Fact, why } from "./bits";

const sqft = (n: number | null | undefined) =>
  n == null ? null : `${n.toLocaleString()} sq ft`;

export function Parcel({ propertyId }: { propertyId: string }) {
  const { can } = useAuth();
  const intel = useQuery({
    queryKey: ["intel", propertyId],
    queryFn: () => api.propertyIntel(propertyId),
  });
  const ownership = useQuery({
    queryKey: ["ownership", propertyId],
    queryFn: () => api.ownership(propertyId),
    enabled: can("property:read"),
    retry: false,
  });
  const liens = useQuery({
    queryKey: ["liens", propertyId],
    queryFn: () => api.liens(propertyId),
    retry: false,
  });
  const loans = useQuery({
    queryKey: ["mortgages", propertyId],
    queryFn: () => api.mortgages(propertyId),
    enabled: can("finance:read"),
    retry: false,
  });

  async function refresh() {
    try {
      await api.enrichProperty(propertyId);
      toast.success("Refreshing the public records. Check back in a minute.");
    } catch (e) {
      toast.error(why(e, "Couldn't start it"));
    }
  }

  if (intel.isLoading) return <Skeleton className="h-64" />;
  const d = intel.data?.detail ?? null;
  const taxes = intel.data?.taxes ?? [];
  const values = intel.data?.valuations ?? [];
  const utilities = intel.data?.utilities ?? [];
  const latestValue = values[0];

  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <Panel>
        <PanelHeader
          title="Parcel"
          description={
            d?.last_enriched_at
              ? `Public records as of ${new Date(d.last_enriched_at).toLocaleDateString()}`
              : "From public records."
          }
          action={
            can("property:write") && (
              <Button size="sm" variant="ghost" onClick={refresh}>
                <RefreshCw />
                Refresh
              </Button>
            )
          }
        />
        <div className="p-5 pt-3">
          {!d ? (
            <EmptyState
              icon={<Landmark />}
              title="No parcel record yet"
              description="Refresh to pull the parcel, taxes, value, schools and utilities."
              className="py-6"
            />
          ) : (
            <dl className="divide-y divide-line">
              <Fact label="Parcel number (APN)">{d.apn}</Fact>
              <Fact label="County">
                {d.county}
                {d.fips && ` (FIPS ${d.fips})`}
              </Fact>
              <Fact label="Zoning">{d.zoning}</Fact>
              <Fact label="Subdivision">{d.subdivision}</Fact>
              <Fact label="Legal description">{d.legal_description}</Fact>
              <Fact label="Lot">{sqft(d.lot_size_sqft)}</Fact>
              <Fact label="Building">
                {[
                  sqft(d.sqft),
                  d.stories &&
                    `${d.stories} stor${d.stories === 1 ? "y" : "ies"}`,
                  d.beds != null && `${d.beds} bd`,
                  d.baths != null && `${d.baths} ba`,
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </Fact>
              <Fact label="Heating / cooling">
                {[d.heating, d.cooling].filter(Boolean).join(" / ")}
              </Fact>
              <Fact label="Parking">
                {d.parking_spaces != null && `${d.parking_spaces} spaces`}
              </Fact>
              <Fact label="Flood zone">
                {d.flood_zone && (
                  <Badge tone={/^[AV]/i.test(d.flood_zone) ? "bad" : "neutral"}>
                    {d.flood_zone}
                  </Badge>
                )}
              </Fact>
              <Fact label="Owner of record">{d.owner_of_record}</Fact>
              <Fact label="Last sale">
                {d.last_sale_date &&
                  `${day(d.last_sale_date)}${d.last_sale_price_label ? ` for ${d.last_sale_price_label}` : ""}`}
              </Fact>
              <Fact label="Location">
                {d.latitude != null && d.longitude != null && (
                  <a
                    className="text-accent hover:underline"
                    href={`https://www.google.com/maps?q=${d.latitude},${d.longitude}`}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    {d.latitude.toFixed(5)}, {d.longitude.toFixed(5)}
                  </a>
                )}
              </Fact>
              <Fact label="Walk score">{d.walk_score}</Fact>
            </dl>
          )}
        </div>
      </Panel>

      <div className="space-y-4">
        <Panel>
          <PanelHeader
            title="Taxes and value"
            description={
              latestValue?.estimated_value_label
                ? `Worth about ${latestValue.estimated_value_label}${latestValue.estimated_rent_label ? `, rents for about ${latestValue.estimated_rent_label}` : ""}`
                : undefined
            }
          />
          <div className="p-2 pt-3">
            {taxes.length === 0 ? (
              <p className="px-3 pb-3 text-[13px] text-fg-3">
                No tax records yet.
              </p>
            ) : (
              <table className="w-full text-[13px]">
                <thead>
                  <tr className="text-left text-[11px] tracking-wide text-fg-3 uppercase">
                    <th className="px-3 py-2 font-medium">Year</th>
                    <th className="px-3 py-2 text-right font-medium">
                      Assessed
                    </th>
                    <th className="px-3 py-2 text-right font-medium">Tax</th>
                  </tr>
                </thead>
                <tbody>
                  {taxes.map((t) => (
                    <tr key={t.tax_year} className="border-t border-line">
                      <td className="px-3 py-2 text-fg">{t.tax_year}</td>
                      <td className="figure px-3 py-2 text-right text-fg-2">
                        {t.assessed_value_label ?? "—"}
                      </td>
                      <td className="figure px-3 py-2 text-right text-fg">
                        {t.tax_amount_label ?? "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </Panel>

        <Panel>
          <PanelHeader title="Title, loans and liens" />
          <div className="space-y-3 p-5 pt-3 text-[13px]">
            {(ownership.data ?? []).map((o) => (
              <div key={o.id} className="flex justify-between gap-3">
                <span className="text-fg">
                  {o.owner_name}
                  {o.vesting && (
                    <span className="text-fg-3"> · {o.vesting}</span>
                  )}
                </span>
                <span className="figure text-fg-2">
                  {(o.percent_bps / 100).toFixed(o.percent_bps % 100 ? 2 : 0)}%
                </span>
              </div>
            ))}
            {(loans.data ?? []).map((m) => (
              <div key={m.id} className="flex justify-between gap-3">
                <span className="text-fg">
                  Loan {m.position > 1 ? `(${m.position}nd)` : ""}{" "}
                  <span className="text-fg-3">
                    {m.interest_rate_pct != null &&
                      `${m.interest_rate_pct}% · `}
                    {m.status}
                  </span>
                </span>
                <span className="figure text-fg-2">
                  {m.current_balance_label ?? m.original_amount_label ?? "—"}
                </span>
              </div>
            ))}
            {(liens.data ?? []).map((l) => (
              <div key={l.id} className="flex justify-between gap-3">
                <span className="text-fg">
                  {l.lienholder_name}{" "}
                  <span className="text-fg-3">
                    · {l.kind.replace(/_/g, " ")} · {l.status}
                  </span>
                </span>
                <span className="figure text-fg-2">
                  {l.amount_label ?? "—"}
                </span>
              </div>
            ))}
            {!ownership.data?.length &&
              !loans.data?.length &&
              !liens.data?.length && (
                <p className="text-fg-3">Nothing recorded.</p>
              )}
          </div>
        </Panel>

        {utilities.length > 0 && (
          <Panel>
            <PanelHeader title="Utilities" />
            <dl className="divide-y divide-line px-5 pb-4">
              {utilities.map((u) => (
                <Fact key={u.utility_type} label={label(u.utility_type)}>
                  {u.provider}
                  {u.est_monthly_cost_label &&
                    ` · about ${u.est_monthly_cost_label}/mo`}
                  {u.phone && ` · ${u.phone}`}
                </Fact>
              ))}
            </dl>
          </Panel>
        )}
      </div>
    </div>
  );
}
