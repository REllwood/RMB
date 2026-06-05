import { ipc } from "@/lib/ipc";
import { useIpcQuery } from "@/lib/useIpc";
import { useMoneyFormat } from "@/lib/money";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <Card>
      <CardContent className="pt-6">
        <p className="text-sm text-muted-foreground">{label}</p>
        <p className="mt-1 text-2xl font-semibold tracking-tight">{value}</p>
      </CardContent>
    </Card>
  );
}

export function DashboardPage() {
  const money = useMoneyFormat();
  const q = useIpcQuery(["dashboard"], () => ipc.dashboardSummary());
  const recentQ = useIpcQuery(["invoices"], () => ipc.listInvoices());

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;

  const s = q.data;
  const recent = (recentQ.data ?? []).slice(0, 5);

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Dashboard</h1>

      {s && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <Stat label="Money owed to you" value={money(s.outstanding_minor)} />
          <Stat label="Unpaid invoices" value={String(s.unpaid_count)} />
          <Stat label="Drafts" value={String(s.draft_count)} />
          <Stat label="Paid invoices" value={String(s.paid_count)} />
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Recent invoices</CardTitle>
          </CardHeader>
          <CardContent>
            {recent.length === 0 ? (
              <p className="text-sm text-muted-foreground">No invoices yet.</p>
            ) : (
              <ul className="divide-y">
                {recent.map((inv) => (
                  <li key={inv.id} className="flex items-center justify-between py-2 text-sm">
                    <span className="font-medium">{inv.number ?? `Draft #${inv.id}`}</span>
                    <span className="flex items-center gap-3">
                      <span>{money(inv.total_minor)}</span>
                      <Badge variant="outline">{inv.status.replace("_", "-")}</Badge>
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Low stock</CardTitle>
          </CardHeader>
          <CardContent>
            {!s || s.low_stock.length === 0 ? (
              <p className="text-sm text-muted-foreground">Nothing needs reordering.</p>
            ) : (
              <ul className="divide-y">
                {s.low_stock.map((it) => (
                  <li key={it.id} className="flex items-center justify-between py-2 text-sm">
                    <span className="font-medium">{it.name}</span>
                    <span className="flex items-center gap-2">
                      {it.qty_on_hand} left
                      <Badge variant="warning">Low</Badge>
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>
      </div>

      {!s && <EmptyState title="No data yet" description="Add customers, items, and invoices to see your numbers." />}
    </div>
  );
}
