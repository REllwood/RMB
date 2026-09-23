import type { ComponentType } from "react";
import { AlertTriangle, Banknote, CheckCircle2, FileText, Hourglass, Sparkles } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import { useMoneyFormat } from "@/lib/money";
import { useNav } from "@/app/nav";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/ui/page-header";
import { Badge } from "@/components/ui/badge";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";
import { cn } from "@/lib/utils";
import { invoiceStatus } from "@/features/invoices/status";

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
          <p className="mt-1 text-2xl font-semibold tracking-tight break-words tabular-nums">
            {value}
          </p>
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
  const goTo = useNav();
  const q = useIpcQuery(["dashboard"], () => ipc.dashboardSummary());
  const recentQ = useIpcQuery(["invoices"], () => ipc.listInvoices());
  const settingsQ = useIpcQuery(["settings"], () => ipc.getSettings());
  const warningQ = useIpcQuery(["startup-warning"], () => ipc.getMeta("startup.warning"));
  const dismissWarning = useIpcMutation(() => ipc.dismissStartupWarning(), [["startup-warning"]]);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;

  const s = q.data;
  const recent = (recentQ.data ?? []).slice(0, 6);
  const needsSetup = settingsQ.data !== undefined && settingsQ.data.business_name.trim() === "";

  return (
    <div className="space-y-6">
      <PageHeader title="Dashboard" description="Where the business stands right now." />

      {warningQ.data?.trim() && (
        <Card className="border-warning/60 bg-card">
          <CardContent className="flex items-start gap-3 pt-6">
            <AlertTriangle className="mt-0.5 size-5 shrink-0 text-foreground" aria-hidden />
            <div className="min-w-0 flex-1">
              <p className="font-medium">Action needed</p>
              <p className="mt-1 whitespace-pre-line text-sm text-muted-foreground">
                {warningQ.data}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => dismissWarning.mutate(undefined)}
              loading={dismissWarning.isPending}
              loadingLabel="Dismissing…"
            >
              Dismiss
            </Button>
          </CardContent>
        </Card>
      )}

      {needsSetup && (
        <Card className="border-primary/30 bg-primary/5">
          <CardContent className="flex flex-wrap items-center justify-between gap-3 pt-6">
            <div className="flex items-start gap-3">
              <Sparkles className="mt-0.5 size-5 text-primary" aria-hidden />
              <div>
                <p className="font-medium">Welcome to RMB</p>
                <p className="text-sm text-muted-foreground">
                  Set your business name, currency, and tax rates first — they appear on every quote
                  and invoice you send.
                </p>
              </div>
            </div>
            <Button onClick={() => goTo("settings")}>Set up your business</Button>
          </CardContent>
        </Card>
      )}

      {s && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-5">
          <Stat
            label="Money owed to you"
            value={money(s.outstanding_minor)}
            icon={Banknote}
            tone="primary"
          />
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
            {recentQ.error ? (
              <ErrorState error={recentQ.error} onRetry={() => recentQ.refetch()} />
            ) : recent.length === 0 ? (
              <p className="text-sm text-muted-foreground">No invoices yet.</p>
            ) : (
              <ul className="divide-y">
                {recent.map((inv) => {
                  const status = invoiceStatus(inv);
                  return (
                    <li key={inv.id}>
                      <button
                        type="button"
                        onClick={() => goTo("invoices", inv.id)}
                        className="flex w-full items-center justify-between gap-3 rounded-md px-1 py-2 text-left text-sm hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
                      >
                        <span className="min-w-0">
                          <span className="font-medium">{inv.number ?? `Draft #${inv.id}`}</span>
                          <span className="block truncate text-muted-foreground">
                            {inv.customer_name || "—"}
                          </span>
                        </span>
                        <span className="flex shrink-0 items-center gap-3">
                          <span className="tabular-nums">{money(inv.total_minor)}</span>
                          <Badge variant={status.variant}>{status.label}</Badge>
                        </span>
                      </button>
                    </li>
                  );
                })}
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
                  <li key={it.id}>
                    <button
                      type="button"
                      onClick={() => goTo("catalog")}
                      className="flex w-full items-center justify-between rounded-md px-1 py-2 text-left text-sm hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
                    >
                      <span className="font-medium">{it.name}</span>
                      <span className="flex items-center gap-2">
                        {it.qty_on_hand} left
                        {it.reorder_point !== null && ` (reorder at ${it.reorder_point})`}
                        <Badge variant="warning">Low</Badge>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>
      </div>

      {!s && (
        <EmptyState
          title="No data yet"
          description="Add customers, items, and invoices to see your numbers."
        />
      )}
    </div>
  );
}
