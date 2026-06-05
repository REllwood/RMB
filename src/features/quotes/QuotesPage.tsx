import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { LineInput } from "@/lib/types";
import { parseMoney, useMoneyFormat } from "@/lib/money";
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
  sent: "secondary",
  accepted: "success",
  declined: "destructive",
  expired: "warning",
  converted: "default",
};

function StatusBadge({ status }: { status: string }) {
  return <Badge variant={STATUS_VARIANT[status] ?? "outline"}>{status}</Badge>;
}

export function QuotesPage() {
  const [view, setView] = useState<View>({ mode: "list" });
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold tracking-tight">Quotes</h1>
        {view.mode === "list" ? (
          <Button onClick={() => setView({ mode: "create" })}>New quote</Button>
        ) : (
          <Button variant="ghost" onClick={() => setView({ mode: "list" })}>← Back to list</Button>
        )}
      </div>
      {view.mode === "list" && <QuoteList onOpen={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "create" && <QuoteCreate onCreated={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "detail" && <QuoteDetailView id={view.id} />}
    </div>
  );
}

function QuoteList({ onOpen }: { onOpen: (id: number) => void }) {
  const money = useMoneyFormat();
  const q = useIpcQuery(["quotes"], () => ipc.listQuotes());
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data || q.data.length === 0)
    return <EmptyState title="No quotes yet" description="Create a quote, then convert it to an invoice when accepted." />;
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
        {q.data.map((row) => (
          <TableRow key={row.id}>
            <TableCell className="font-medium">{row.number ?? `#${row.id}`}</TableCell>
            <TableCell><StatusBadge status={row.status} /></TableCell>
            <TableCell>{money(row.total_minor)}</TableCell>
            <TableCell className="text-right">
              <Button variant="ghost" size="sm" onClick={() => onOpen(row.id)}>Open</Button>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

type EditLine = { description: string; quantity: string; price: string; taxIdx: number };

function QuoteCreate({ onCreated }: { onCreated: (id: number) => void }) {
  const customersQ = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const create = useIpcMutation(
    (v: { customerId: number; lines: LineInput[]; validUntil: string | null; notes: string }) =>
      ipc.createQuote(v.customerId, v.lines, v.validUntil, v.notes),
    [["quotes"]],
  );
  const money = useMoneyFormat();

  const [customerId, setCustomerId] = useState<number | null>(null);
  const [validUntil, setValidUntil] = useState("");
  const [notes, setNotes] = useState("");
  const [lines, setLines] = useState<EditLine[]>([{ description: "", quantity: "1", price: "0.00", taxIdx: 0 }]);

  const taxes = taxQ.data ?? [];
  const netPreview = lines.reduce(
    (sum, l) => sum + Math.round((parseMoney(l.price) ?? 0) * (Number(l.quantity) || 0)),
    0,
  );

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
    const id = await create.mutateAsync({
      customerId,
      lines: payload,
      validUntil: validUntil || null,
      notes,
    });
    onCreated(id);
  }

  if (customersQ.data && customersQ.data.length === 0)
    return <EmptyState title="Add a customer first" description="Quotes need a customer." />;

  return (
    <Card>
      <CardHeader><CardTitle>New quote</CardTitle></CardHeader>
      <CardContent className="space-y-4">
        <div className="grid max-w-xl gap-4 sm:grid-cols-2">
          <Field label="Customer" required>
            {(p) => (
              <Select {...p} value={customerId ?? ""} onChange={(e) => setCustomerId(e.target.value ? Number(e.target.value) : null)}>
                <option value="" disabled>Choose a customer…</option>
                {customersQ.data?.map((c) => (<option key={c.id} value={c.id}>{c.name}</option>))}
              </Select>
            )}
          </Field>
          <Field label="Valid until">
            {(p) => <Input {...p} type="date" value={validUntil} onChange={(e) => setValidUntil(e.target.value)} />}
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
                  <label className="sr-only" htmlFor={`qdesc-${i}`}>Description</label>
                  <Input id={`qdesc-${i}`} value={l.description} onChange={(e) => setLine(i, { description: e.target.value })} />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`qqty-${i}`}>Quantity</label>
                  <Input id={`qqty-${i}`} inputMode="decimal" value={l.quantity} onChange={(e) => setLine(i, { quantity: e.target.value })} />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`qprice-${i}`}>Unit price</label>
                  <Input id={`qprice-${i}`} inputMode="decimal" value={l.price} onChange={(e) => setLine(i, { price: e.target.value })} />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`qtax-${i}`}>Tax rate</label>
                  <Select id={`qtax-${i}`} value={l.taxIdx} onChange={(e) => setLine(i, { taxIdx: Number(e.target.value) })}>
                    {taxes.length === 0 && <option value={0}>No Tax</option>}
                    {taxes.map((t, idx) => (<option key={t.id} value={idx}>{t.name}</option>))}
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
          <Button variant="outline" size="sm" onClick={() => setLines((ls) => [...ls, { description: "", quantity: "1", price: "0.00", taxIdx: 0 }])}>Add line</Button>
          <p className="text-sm text-muted-foreground">Approx. subtotal (excl. tax): <span className="font-medium text-foreground">{money(netPreview)}</span></p>
        </div>

        <Field label="Notes">
          {(p) => <Input {...p} value={notes} onChange={(e) => setNotes(e.target.value)} />}
        </Field>

        <Button onClick={onSave} disabled={customerId === null || create.isPending}>
          {create.isPending ? "Saving…" : "Save quote"}
        </Button>
      </CardContent>
    </Card>
  );
}

function QuoteDetailView({ id }: { id: number }) {
  const money = useMoneyFormat();
  const q = useIpcQuery(["quote", id], () => ipc.getQuote(id));
  const setStatus = useIpcMutation((s: string) => ipc.setQuoteStatus(id, s), [["quote", id], ["quotes"]]);
  const convert = useIpcMutation(() => ipc.convertQuoteToInvoice(id), [["quote", id], ["quotes"], ["invoices"]]);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Quote not found" />;

  const { quote, lines } = q.data;

  async function onExportPdf() {
    const path = await save({
      defaultPath: `${quote.number ?? `quote-${quote.id}`}.pdf`,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (path) await ipc.exportQuotePdf(quote.id, path);
  }

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>{quote.number ?? `Quote #${quote.id}`}</CardTitle>
        <StatusBadge status={quote.status} />
      </CardHeader>
      <CardContent className="space-y-4">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Description</TableHead>
              <TableHead className="text-right">Qty</TableHead>
              <TableHead className="text-right">Unit</TableHead>
              <TableHead className="text-right">Total</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {lines.map((l) => (
              <TableRow key={l.id}>
                <TableCell>{l.description}</TableCell>
                <TableCell className="text-right">{l.quantity}</TableCell>
                <TableCell className="text-right">{money(l.unit_price_minor)}</TableCell>
                <TableCell className="text-right">{money(l.gross_minor)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>

        <div className="ml-auto grid max-w-xs gap-1 text-sm">
          <Row label="Subtotal" value={money(quote.subtotal_minor)} />
          <Row label="Tax" value={money(quote.tax_minor)} />
          <Row label="Total" value={money(quote.total_minor)} strong />
        </div>

        <div className="flex flex-wrap gap-2 border-t pt-4">
          <Button variant="outline" onClick={onExportPdf}>Export PDF</Button>
          {quote.status === "draft" && <Button onClick={() => setStatus.mutate("sent")}>Mark sent</Button>}
          {quote.status === "sent" && (
            <>
              <Button onClick={() => setStatus.mutate("accepted")}>Mark accepted</Button>
              <Button variant="outline" onClick={() => setStatus.mutate("declined")}>Decline</Button>
            </>
          )}
          {quote.status === "accepted" && (
            <Button variant="default" onClick={() => convert.mutate(undefined)} disabled={convert.isPending}>
              Convert to invoice
            </Button>
          )}
          {quote.converted_invoice_id && (
            <p className="self-center text-sm text-muted-foreground">Converted → invoice #{quote.converted_invoice_id}</p>
          )}
        </div>
      </CardContent>
    </Card>
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
