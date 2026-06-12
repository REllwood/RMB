import type { ComponentType } from "react";
import { AlertTriangle, Banknote, CheckCircle2, FileText, Hourglass } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcQuery } from "@/lib/useIpc";
import { useMoneyFormat } from "@/lib/money";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/ui/page-header";
import { Badge } from "@/components/ui/badge";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";
import { cn } from "@/lib/utils";

type IconType = ComponentType<{ className?: string }>;

function Stat({
  label,
  value,
  icon: Icon,
  tone = "default",
}: {
  label: string;
  value: string;
  icon: IconType;
  tone?: "default" | "primary" | "destructive";
}) {
  return (
    <Card>
      <CardContent className="flex items-start justify-between gap-3 pt-6">
        <div className="min-w-0">
          <p className="text-sm text-muted-foreground">{label}</p>
          <p className="mt-1 truncate text-2xl font-semibold tracking-tight tabular-nums">{value}</p>
        </div>
        <div
          className={cn(
            "flex size-9 shrink-0 items-center justify-center rounded-lg",
            tone === "primary" && "bg-primary/10 text-primary",
            tone === "destructive" && "bg-destructive/10 text-destructive",
            tone === "default" && "bg-muted text-muted-foreground",
          )}
        >
          <Icon className="size-4.5" aria-hidden />
        </div>
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
  const recent = (recentQ.data ?? []).slice(0, 6);

  return (
    <div className="space-y-6">
      <PageHeader title="Dashboard" description="Where the business stands right now." />

      {s && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
          <Stat label="Money owed to you" value={money(s.outstanding_minor)} icon={Banknote} tone="primary" />
          <Stat
            label="Overdue"
            value={String(s.overdue_count)}
            icon={AlertTriangle}
            tone={s.overdue_count > 0 ? "destructive" : "default"}
          />
          <Stat label="Unpaid invoices" value={String(s.unpaid_count)} icon={Hourglass} />
          <Stat label="Drafts" value={String(s.draft_count)} icon={FileText} />
          <Stat label="Paid invoices" value={String(s.paid_count)} icon={CheckCircle2} />
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
                      <span className="tabular-nums">{money(inv.total_minor)}</span>
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
