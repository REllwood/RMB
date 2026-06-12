import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { FileDown } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcQuery } from "@/lib/useIpc";
import { todayLocalISO } from "@/lib/format";
import { useMoneyFormat } from "@/lib/money";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { PageHeader } from "@/components/ui/page-header";
import { Select } from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ErrorState, Loading } from "@/components/ui/states";

type Preset = "this-month" | "last-month" | "this-year" | "all-time" | "custom";

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/** Inclusive local-date range for a preset; null bound = unbounded. */
function presetRange(preset: Preset): { from: string | null; to: string | null } {
  const now = new Date();
  const y = now.getFullYear();
  const m = now.getMonth(); // 0-based
  switch (preset) {
    case "this-month":
      return { from: `${y}-${pad(m + 1)}-01`, to: todayLocalISO() };
    case "last-month": {
      const first = new Date(y, m - 1, 1);
      const last = new Date(y, m, 0); // day 0 of this month = last day of previous
      return {
        from: `${first.getFullYear()}-${pad(first.getMonth() + 1)}-01`,
        to: `${last.getFullYear()}-${pad(last.getMonth() + 1)}-${pad(last.getDate())}`,
      };
    }
    case "this-year":
      return { from: `${y}-01-01`, to: todayLocalISO() };
    default:
      return { from: null, to: null };
  }
}

export function ReportsPage() {
  const money = useMoneyFormat();
  const toast = useToast();
  const [preset, setPreset] = useState<Preset>("this-year");
  const [customFrom, setCustomFrom] = useState("");
  const [customTo, setCustomTo] = useState("");

  const range =
    preset === "custom"
      ? { from: customFrom || null, to: customTo || null }
      : presetRange(preset);
  const key = [range.from, range.to];

  const taxQ = useIpcQuery(["report-tax", ...key], () => ipc.reportTaxSummary(range.from, range.to));
  const monthQ = useIpcQuery(["report-month", ...key], () => ipc.reportSalesMonthly(range.from, range.to));
  const custQ = useIpcQuery(["report-cust", ...key], () => ipc.reportSalesCustomers(range.from, range.to));

  async function exportCsv(kind: "invoices" | "payments" | "customers") {
    const path = await save({
      defaultPath: `rmb-${kind}${range.from ? `-${range.from}-to-${range.to ?? "now"}` : ""}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      if (kind === "invoices") await ipc.exportInvoicesCsv(path, range.from, range.to);
      else if (kind === "payments") await ipc.exportPaymentsCsv(path, range.from, range.to);
      else await ipc.exportCustomersCsv(path);
      toast("success", `Exported ${kind} CSV`);
    } catch (err) {
      toast("error", err instanceof Error ? err.message : String(err));
    }
  }

  const taxTotals = (taxQ.data ?? []).reduce(
    (acc, r) => ({
      net: acc.net + r.net_minor,
      tax: acc.tax + r.tax_minor,
      gross: acc.gross + r.gross_minor,
    }),
    { net: 0, tax: 0, gross: 0 },
  );

  return (
    <div className="space-y-6">
      <PageHeader
        title="Reports"
        description="Accrual basis — grouped by invoice issue date. Drafts and voided invoices are excluded."
      />

      <div className="flex flex-wrap items-end gap-3">
        <Field label="Period">
          {(p) => (
            <Select {...p} className="w-44" value={preset} onChange={(e) => setPreset(e.target.value as Preset)}>
              <option value="this-month">This month</option>
              <option value="last-month">Last month</option>
              <option value="this-year">This year</option>
              <option value="all-time">All time</option>
              <option value="custom">Custom…</option>
            </Select>
          )}
        </Field>
        {preset === "custom" && (
          <>
            <Field label="From">
              {(p) => <Input {...p} type="date" className="w-40" value={customFrom} onChange={(e) => setCustomFrom(e.target.value)} />}
            </Field>
            <Field label="To">
              {(p) => <Input {...p} type="date" className="w-40" value={customTo} onChange={(e) => setCustomTo(e.target.value)} />}
            </Field>
          </>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Tax collected</CardTitle>
          <CardDescription>Per rate, from line-level snapshots — your BAS / VAT-return numbers.</CardDescription>
        </CardHeader>
        <CardContent>
          {taxQ.isLoading ? (
            <Loading />
          ) : taxQ.error ? (
            <ErrorState error={taxQ.error} onRetry={() => taxQ.refetch()} />
          ) : !taxQ.data || taxQ.data.length === 0 ? (
            <p className="text-sm text-muted-foreground">No issued invoices in this period.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Rate</TableHead>
                  <TableHead className="text-right">Net</TableHead>
                  <TableHead className="text-right">Tax</TableHead>
                  <TableHead className="text-right">Gross</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {taxQ.data.map((r) => (
                  <TableRow key={`${r.tax_rate_name}-${r.tax_rate_bp}`}>
                    <TableCell className="font-medium">{r.tax_rate_name}</TableCell>
                    <TableCell className="text-right tabular-nums">{money(r.net_minor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{money(r.tax_minor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{money(r.gross_minor)}</TableCell>
                  </TableRow>
                ))}
                <TableRow className="font-semibold">
                  <TableCell>Total</TableCell>
                  <TableCell className="text-right tabular-nums">{money(taxTotals.net)}</TableCell>
                  <TableCell className="text-right tabular-nums">{money(taxTotals.tax)}</TableCell>
                  <TableCell className="text-right tabular-nums">{money(taxTotals.gross)}</TableCell>
                </TableRow>
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Sales by month</CardTitle>
          </CardHeader>
          <CardContent>
            {monthQ.isLoading ? (
              <Loading />
            ) : !monthQ.data || monthQ.data.length === 0 ? (
              <p className="text-sm text-muted-foreground">Nothing issued in this period.</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Month</TableHead>
                    <TableHead className="text-right">Invoices</TableHead>
                    <TableHead className="text-right">Tax</TableHead>
                    <TableHead className="text-right">Gross</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {monthQ.data.map((r) => (
                    <TableRow key={r.month}>
                      <TableCell className="font-medium">{r.month}</TableCell>
                      <TableCell className="text-right tabular-nums">{r.invoice_count}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(r.tax_minor)}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(r.gross_minor)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Top customers</CardTitle>
          </CardHeader>
          <CardContent>
            {custQ.isLoading ? (
              <Loading />
            ) : !custQ.data || custQ.data.length === 0 ? (
              <p className="text-sm text-muted-foreground">Nothing issued in this period.</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Customer</TableHead>
                    <TableHead className="text-right">Invoices</TableHead>
                    <TableHead className="text-right">Total</TableHead>
                    <TableHead className="text-right">Paid</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {custQ.data.slice(0, 10).map((r) => (
                    <TableRow key={r.customer_id}>
                      <TableCell className="font-medium">{r.name}</TableCell>
                      <TableCell className="text-right tabular-nums">{r.invoice_count}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(r.gross_minor)}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(r.paid_minor)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Export CSV</CardTitle>
          <CardDescription>
            Spreadsheet-ready files for your accountant. Invoices and payments respect the period
            above; customers exports everything.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-3">
          <Button variant="outline" onClick={() => exportCsv("invoices")}>
            <FileDown className="size-4" /> Invoices
          </Button>
          <Button variant="outline" onClick={() => exportCsv("payments")}>
            <FileDown className="size-4" /> Payments
          </Button>
          <Button variant="outline" onClick={() => exportCsv("customers")}>
            <FileDown className="size-4" /> Customers
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
