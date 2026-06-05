import { useState } from "react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { LineInput } from "@/lib/types";
import { minorToInput, parseMoney, useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";

type View = { mode: "list" } | { mode: "create" } | { mode: "detail"; id: number };

const STATUS_VARIANT: Record<
  string,
  "default" | "secondary" | "success" | "warning" | "outline" | "destructive"
> = {
  draft: "outline",
  issued: "secondary",
  part_paid: "warning",
  paid: "success",
  void: "destructive",
};

function StatusBadge({ status }: { status: string }) {
  const label = status.replace("_", "-");
  return <Badge variant={STATUS_VARIANT[status] ?? "outline"}>{label}</Badge>;
}

export function InvoicesPage() {
  const [view, setView] = useState<View>({ mode: "list" });

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold tracking-tight">Invoices</h1>
        {view.mode === "list" && <Button onClick={() => setView({ mode: "create" })}>New invoice</Button>}
        {view.mode !== "list" && <Button variant="ghost" onClick={() => setView({ mode: "list" })}>← Back to list</Button>}
      </div>

      {view.mode === "list" && <InvoiceList onOpen={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "create" && <InvoiceCreate onCreated={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "detail" && <InvoiceDetailView id={view.id} />}
    </div>
  );
}

function InvoiceList({ onOpen }: { onOpen: (id: number) => void }) {
  const money = useMoneyFormat();
  const q = useIpcQuery(["invoices"], () => ipc.listInvoices());
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data || q.data.length === 0)
    return <EmptyState title="No invoices yet" description="Create your first invoice to get paid." />;
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Number</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Total</TableHead>
          <TableHead><span className="sr-only">Open</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {q.data.map((inv) => (
          <TableRow key={inv.id}>
            <TableCell className="font-medium">{inv.number ?? `Draft #${inv.id}`}</TableCell>
            <TableCell><StatusBadge status={inv.status} /></TableCell>
            <TableCell>{money(inv.total_minor)}</TableCell>
            <TableCell className="text-right">
              <Button variant="ghost" size="sm" onClick={() => onOpen(inv.id)}>Open</Button>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

type EditLine = { description: string; quantity: string; price: string; taxIdx: number };

function InvoiceCreate({ onCreated }: { onCreated: (id: number) => void }) {
  const customersQ = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const create = useIpcMutation(
    (v: { customerId: number; lines: LineInput[]; notes: string }) =>
      ipc.createInvoice(v.customerId, v.lines, null, v.notes),
    [["invoices"]],
  );

  const [customerId, setCustomerId] = useState<number | null>(null);
  const [notes, setNotes] = useState("");
  const [lines, setLines] = useState<EditLine[]>([{ description: "", quantity: "1", price: "0.00", taxIdx: 0 }]);

  const taxes = taxQ.data ?? [];
  const netPreview = lines.reduce(
    (sum, l) => sum + Math.round((parseMoney(l.price) ?? 0) * (Number(l.quantity) || 0)),
    0,
  );
  const money = useMoneyFormat();

  function setLine(i: number, patch: Partial<EditLine>) {
    setLines((ls) => ls.map((l, idx) => (idx === i ? { ...l, ...patch } : l)));
  }

  async function onSave() {
    if (customerId === null) return;
    const payload: LineInput[] = lines
      .filter((l) => l.description.trim())
      .map((l) => {
        const tax = taxes[l.taxIdx];
        return {
          item_id: null,
          description: l.description.trim(),
          quantity: l.quantity || "1",
          unit_price_minor: parseMoney(l.price) ?? 0,
          tax_rate_name: tax?.name ?? "No Tax",
          tax_rate_bp: tax?.rate_bp ?? 0,
          tax_inclusive: tax?.inclusive ?? false,
        };
      });
    if (payload.length === 0) return;
    const id = await create.mutateAsync({ customerId, lines: payload, notes });
    onCreated(id);
  }

  if (customersQ.data && customersQ.data.length === 0)
    return <EmptyState title="Add a customer first" description="Invoices need a customer — create one under Customers." />;

  return (
    <Card>
      <CardHeader><CardTitle>New invoice</CardTitle></CardHeader>
      <CardContent className="space-y-4">
        <div className="max-w-sm">
          <Field label="Customer" required>
            {(p) => (
              <Select {...p} value={customerId ?? ""} onChange={(e) => setCustomerId(e.target.value ? Number(e.target.value) : null)}>
                <option value="" disabled>Choose a customer…</option>
                {customersQ.data?.map((c) => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
              </Select>
            )}
          </Field>
        </div>

        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Description</TableHead>
              <TableHead className="w-20">Qty</TableHead>
              <TableHead className="w-28">Unit price</TableHead>
              <TableHead className="w-40">Tax</TableHead>
              <TableHead><span className="sr-only">Remove</span></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {lines.map((l, i) => (
              <TableRow key={i}>
                <TableCell>
                  <label className="sr-only" htmlFor={`desc-${i}`}>Description</label>
                  <Input id={`desc-${i}`} value={l.description} onChange={(e) => setLine(i, { description: e.target.value })} />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`qty-${i}`}>Quantity</label>
                  <Input id={`qty-${i}`} inputMode="decimal" value={l.quantity} onChange={(e) => setLine(i, { quantity: e.target.value })} />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`price-${i}`}>Unit price</label>
                  <Input id={`price-${i}`} inputMode="decimal" value={l.price} onChange={(e) => setLine(i, { price: e.target.value })} />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`tax-${i}`}>Tax rate</label>
                  <Select id={`tax-${i}`} value={l.taxIdx} onChange={(e) => setLine(i, { taxIdx: Number(e.target.value) })}>
                    {taxes.length === 0 && <option value={0}>No Tax</option>}
                    {taxes.map((t, idx) => (
                      <option key={t.id} value={idx}>{t.name}</option>
                    ))}
                  </Select>
                </TableCell>
                <TableCell className="text-right">
                  <Button variant="ghost" size="sm" onClick={() => setLines((ls) => ls.filter((_, idx) => idx !== i))} aria-label={`Remove line ${i + 1}`}>✕</Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>

        <div className="flex items-center justify-between">
          <Button variant="outline" size="sm" onClick={() => setLines((ls) => [...ls, { description: "", quantity: "1", price: "0.00", taxIdx: 0 }])}>
            Add line
          </Button>
          <p className="text-sm text-muted-foreground">Approx. subtotal (excl. tax): <span className="font-medium text-foreground">{money(netPreview)}</span></p>
        </div>

        <Field label="Notes">
          {(p) => <Input {...p} value={notes} onChange={(e) => setNotes(e.target.value)} />}
        </Field>

        <Button onClick={onSave} disabled={customerId === null || create.isPending}>
          {create.isPending ? "Saving…" : "Save draft"}
        </Button>
      </CardContent>
    </Card>
  );
}

function InvoiceDetailView({ id }: { id: number }) {
  const money = useMoneyFormat();
  const q = useIpcQuery(["invoice", id], () => ipc.getInvoice(id));
  const issue = useIpcMutation(() => ipc.issueInvoice(id), [["invoice", id], ["invoices"]]);
  const voidMut = useIpcMutation(() => ipc.voidInvoice(id), [["invoice", id], ["invoices"], ["items"]]);
  const pay = useIpcMutation(
    (v: { amount: number; method: string }) => ipc.recordPayment(id, v.amount, v.method, ""),
    [["invoice", id], ["invoices"]],
  );

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Invoice not found" />;

  const { invoice, lines, amount_paid_minor } = q.data;
  const balance = invoice.total_minor - amount_paid_minor;

  function onIssue() {
    if (window.confirm("Issue this invoice? It gets a number and is locked — corrections need a void + reissue.")) issue.mutate(undefined);
  }
  function onVoid() {
    if (window.confirm("Void this invoice? Stock is restored; the number is kept.")) voidMut.mutate(undefined);
  }
  function onPay() {
    const raw = window.prompt(`Payment amount (balance ${minorToInput(balance)}):`, minorToInput(balance));
    const amount = raw ? parseMoney(raw) : null;
    const method = (window.prompt("Method (cash, card, transfer):", "bank transfer") ?? "").trim();
    if (amount && amount > 0) pay.mutate({ amount, method });
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>{invoice.number ?? `Draft #${invoice.id}`}</CardTitle>
          <StatusBadge status={invoice.status} />
        </CardHeader>
        <CardContent className="space-y-4">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Description</TableHead>
                <TableHead className="text-right">Qty</TableHead>
                <TableHead className="text-right">Unit</TableHead>
                <TableHead className="text-right">Net</TableHead>
                <TableHead className="text-right">Tax</TableHead>
                <TableHead className="text-right">Total</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {lines.map((l) => (
                <TableRow key={l.id}>
                  <TableCell>{l.description}</TableCell>
                  <TableCell className="text-right">{l.quantity}</TableCell>
                  <TableCell className="text-right">{money(l.unit_price_minor)}</TableCell>
                  <TableCell className="text-right">{money(l.net_minor)}</TableCell>
                  <TableCell className="text-right">{money(l.tax_minor)}</TableCell>
                  <TableCell className="text-right">{money(l.gross_minor)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>

          <div className="ml-auto grid max-w-xs gap-1 text-sm">
            <Row label="Subtotal" value={money(invoice.subtotal_minor)} />
            <Row label="Tax" value={money(invoice.tax_minor)} />
            <Row label="Total" value={money(invoice.total_minor)} strong />
            <Row label="Paid" value={money(amount_paid_minor)} />
            <Row label="Balance" value={money(balance)} strong />
          </div>

          <div className="flex flex-wrap gap-2 border-t pt-4">
            {invoice.status === "draft" && <Button onClick={onIssue} disabled={issue.isPending}>Issue invoice</Button>}
            {(invoice.status === "issued" || invoice.status === "part_paid") && (
              <>
                <Button onClick={onPay} disabled={pay.isPending}>Record payment</Button>
                {amount_paid_minor === 0 && (
                  <Button variant="outline" onClick={onVoid} disabled={voidMut.isPending}>Void</Button>
                )}
              </>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-muted-foreground">{label}</span>
      <span className={strong ? "font-semibold" : ""}>{value}</span>
    </div>
  );
}
