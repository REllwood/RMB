import { useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { FileDown, Pencil, Plus, Repeat, Trash2 } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { InvoiceDetail, InvoiceRow } from "@/lib/types";
import { minorToInput, parseMoney, useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog, Dialog } from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { PageHeader } from "@/components/ui/page-header";
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
import { DocumentForm } from "@/features/shared/DocumentForm";
import { fromRows } from "@/features/shared/lines";
import { invoiceStatus } from "@/features/invoices/status";
import { RecurringView } from "@/features/invoices/RecurringView";

type View =
  | { mode: "list" }
  | { mode: "create" }
  | { mode: "edit"; id: number }
  | { mode: "detail"; id: number }
  | { mode: "recurring" };

function StatusBadge({ inv }: { inv: Pick<InvoiceRow, "status" | "due_date"> }) {
  const s = invoiceStatus(inv);
  return <Badge variant={s.variant}>{s.label}</Badge>;
}

export function InvoicesPage() {
  const [view, setView] = useState<View>({ mode: "list" });

  return (
    <div className="space-y-6">
      <PageHeader
        title="Invoices"
        description="Issue, get paid, stay on top of what's owed."
        actions={
          view.mode === "list" ? (
            <>
              <Button variant="outline" onClick={() => setView({ mode: "recurring" })}>
                <Repeat className="size-4" /> Recurring
              </Button>
              <Button onClick={() => setView({ mode: "create" })}>
                <Plus className="size-4" /> New invoice
              </Button>
            </>
          ) : (
            <Button variant="ghost" onClick={() => setView({ mode: "list" })}>
              ← Back to list
            </Button>
          )
        }
      />

      {view.mode === "list" && <InvoiceList onOpen={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "recurring" && <RecurringView />}
      {view.mode === "create" && (
        <DocumentForm
          kind="invoice"
          onSaved={(id) => setView({ mode: "detail", id })}
          onCancel={() => setView({ mode: "list" })}
        />
      )}
      {view.mode === "edit" && (
        <InvoiceEdit
          id={view.id}
          onDone={() => setView({ mode: "detail", id: view.id })}
        />
      )}
      {view.mode === "detail" && (
        <InvoiceDetailView
          id={view.id}
          onEdit={() => setView({ mode: "edit", id: view.id })}
          onDeleted={() => setView({ mode: "list" })}
        />
      )}
    </div>
  );
}

function useCustomerNames(): Map<number, string> {
  const q = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  return useMemo(() => new Map((q.data ?? []).map((c) => [c.id, c.name])), [q.data]);
}

function InvoiceList({ onOpen }: { onOpen: (id: number) => void }) {
  const money = useMoneyFormat();
  const names = useCustomerNames();
  const q = useIpcQuery(["invoices"], () => ipc.listInvoices());
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data || q.data.length === 0)
    return <EmptyState title="No invoices yet" description="Create your first invoice to get paid." />;
  return (
    <Card className="overflow-hidden py-0">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="pl-4">Number</TableHead>
            <TableHead>Customer</TableHead>
            <TableHead>Date</TableHead>
            <TableHead>Due</TableHead>
            <TableHead>Status</TableHead>
            <TableHead className="text-right">Total</TableHead>
            <TableHead>
              <span className="sr-only">Open</span>
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {q.data.map((inv) => (
            <TableRow key={inv.id} className="cursor-pointer" onClick={() => onOpen(inv.id)}>
              <TableCell className="pl-4 font-medium">{inv.number ?? `Draft #${inv.id}`}</TableCell>
              <TableCell>{names.get(inv.customer_id) ?? "—"}</TableCell>
              <TableCell className="text-muted-foreground">
                {inv.issue_date ?? inv.created_at.slice(0, 10)}
              </TableCell>
              <TableCell className="text-muted-foreground">{inv.due_date ?? "—"}</TableCell>
              <TableCell>
                <StatusBadge inv={inv} />
              </TableCell>
              <TableCell className="text-right tabular-nums">{money(inv.total_minor)}</TableCell>
              <TableCell className="pr-4 text-right">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    onOpen(inv.id);
                  }}
                >
                  Open
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Card>
  );
}

function InvoiceEdit({ id, onDone }: { id: number; onDone: () => void }) {
  const q = useIpcQuery(["invoice", id], () => ipc.getInvoice(id));
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Invoice not found" />;
  const d: InvoiceDetail = q.data;
  if (d.invoice.status !== "draft")
    return <EmptyState title="Only drafts can be edited" description="Void + reissue to correct an issued invoice." />;
  return (
    <DocumentForm
      kind="invoice"
      initial={{
        id,
        customer_id: d.invoice.customer_id,
        date: d.invoice.due_date,
        notes: d.invoice.notes,
        lines: fromRows(d.lines),
      }}
      onSaved={onDone}
      onCancel={onDone}
    />
  );
}

function PaymentDialog({
  open,
  onClose,
  balanceMinor,
  onSubmit,
  pending,
}: {
  open: boolean;
  onClose: () => void;
  balanceMinor: number;
  onSubmit: (v: { amountMinor: number; method: string; reference: string }) => void;
  pending: boolean;
}) {
  const [amount, setAmount] = useState(minorToInput(balanceMinor));
  const [method, setMethod] = useState("bank transfer");
  const [reference, setReference] = useState("");
  const parsed = parseMoney(amount);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Record payment"
      description="Overpayments are clamped to the outstanding balance."
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            disabled={!parsed || parsed <= 0 || pending}
            onClick={() => parsed && parsed > 0 && onSubmit({ amountMinor: parsed, method, reference })}
          >
            {pending ? "Saving…" : "Record payment"}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <Field label="Amount" required>
          {(p) => <Input {...p} inputMode="decimal" value={amount} onChange={(e) => setAmount(e.target.value)} />}
        </Field>
        <Field label="Method">
          {(p) => (
            <Select {...p} value={method} onChange={(e) => setMethod(e.target.value)}>
              <option value="bank transfer">Bank transfer</option>
              <option value="cash">Cash</option>
              <option value="card">Card</option>
              <option value="cheque">Cheque</option>
              <option value="other">Other</option>
            </Select>
          )}
        </Field>
        <Field label="Reference" hint="Optional — e.g. a transfer reference">
          {(p) => <Input {...p} value={reference} onChange={(e) => setReference(e.target.value)} />}
        </Field>
      </div>
    </Dialog>
  );
}

function InvoiceDetailView({
  id,
  onEdit,
  onDeleted,
}: {
  id: number;
  onEdit: () => void;
  onDeleted: () => void;
}) {
  const money = useMoneyFormat();
  const names = useCustomerNames();
  const q = useIpcQuery(["invoice", id], () => ipc.getInvoice(id));
  const isDraft = q.data?.invoice.status === "draft";
  const paymentsQ = useIpcQuery(["payments", id], () => ipc.invoicePayments(id), !!q.data && !isDraft);

  const refresh: unknown[][] = [["invoice", id], ["invoices"], ["payments", id], ["dashboard"]];
  const issue = useIpcMutation(() => ipc.issueInvoice(id), [...refresh, ["items"]], {
    successMessage: "Invoice issued",
  });
  const voidMut = useIpcMutation(() => ipc.voidInvoice(id), [...refresh, ["items"]], {
    successMessage: "Invoice voided — stock restored",
  });
  const deleteDraft = useIpcMutation(() => ipc.deleteInvoiceDraft(id), [["invoices"]], {
    successMessage: "Draft deleted",
  });
  const pay = useIpcMutation(
    (v: { amountMinor: number; method: string; reference: string }) =>
      ipc.recordPayment(id, v.amountMinor, v.method, v.reference),
    refresh,
    { successMessage: "Payment recorded" },
  );
  const removePayment = useIpcMutation((paymentId: number) => ipc.deletePayment(paymentId), refresh, {
    successMessage: "Payment removed",
  });

  const [confirm, setConfirm] = useState<
    null | { kind: "issue" } | { kind: "void" } | { kind: "delete" } | { kind: "remove-payment"; id: number }
  >(null);
  const [paying, setPaying] = useState(false);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Invoice not found" />;

  const { invoice, lines, amount_paid_minor } = q.data;
  const balance = invoice.total_minor - amount_paid_minor;
  const payable = invoice.status === "issued" || invoice.status === "part_paid";

  async function onExportPdf() {
    const path = await save({
      defaultPath: `${invoice.number ?? `invoice-${invoice.id}`}.pdf`,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (path) await ipc.exportInvoicePdf(invoice.id, path);
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex-row items-start justify-between">
          <div className="space-y-1">
            <CardTitle>{invoice.number ?? `Draft #${invoice.id}`}</CardTitle>
            <p className="text-sm text-muted-foreground">
              {names.get(invoice.customer_id) ?? "—"}
              {invoice.issue_date && ` · issued ${invoice.issue_date}`}
              {invoice.due_date && ` · due ${invoice.due_date}`}
            </p>
          </div>
          <StatusBadge inv={invoice} />
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
                  <TableCell className="text-right tabular-nums">{l.quantity}</TableCell>
                  <TableCell className="text-right tabular-nums">{money(l.unit_price_minor)}</TableCell>
                  <TableCell className="text-right tabular-nums">{money(l.net_minor)}</TableCell>
                  <TableCell className="text-right tabular-nums">{money(l.tax_minor)}</TableCell>
                  <TableCell className="text-right tabular-nums">{money(l.gross_minor)}</TableCell>
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
            <Button variant="outline" onClick={onExportPdf}>
              <FileDown className="size-4" /> Export PDF
            </Button>
            {invoice.status === "draft" && (
              <>
                <Button variant="outline" onClick={onEdit}>
                  <Pencil className="size-4" /> Edit
                </Button>
                <Button onClick={() => setConfirm({ kind: "issue" })} disabled={issue.isPending}>
                  Issue invoice
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => setConfirm({ kind: "delete" })}
                  disabled={deleteDraft.isPending}
                >
                  <Trash2 className="size-4" /> Delete draft
                </Button>
              </>
            )}
            {payable && (
              <>
                <Button onClick={() => setPaying(true)} disabled={pay.isPending}>
                  Record payment
                </Button>
                {amount_paid_minor === 0 && (
                  <Button variant="outline" onClick={() => setConfirm({ kind: "void" })} disabled={voidMut.isPending}>
                    Void
                  </Button>
                )}
              </>
            )}
          </div>
        </CardContent>
      </Card>

      {!isDraft && (
        <Card>
          <CardHeader className="flex-row items-center justify-between">
            <CardTitle className="text-base">Payments</CardTitle>
            {(paymentsQ.data?.length ?? 0) > 0 && (
              <Button
                variant="outline"
                size="sm"
                onClick={async () => {
                  const path = await save({
                    defaultPath: `${invoice.number ?? `invoice-${invoice.id}`}-receipt.pdf`,
                    filters: [{ name: "PDF", extensions: ["pdf"] }],
                  });
                  if (path) await ipc.exportReceiptPdf(invoice.id, path);
                }}
              >
                <FileDown className="size-4" /> Receipt PDF
              </Button>
            )}
          </CardHeader>
          <CardContent>
            {paymentsQ.isLoading ? (
              <Loading />
            ) : !paymentsQ.data || paymentsQ.data.length === 0 ? (
              <p className="text-sm text-muted-foreground">No payments recorded yet.</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Date</TableHead>
                    <TableHead>Method</TableHead>
                    <TableHead>Reference</TableHead>
                    <TableHead className="text-right">Amount</TableHead>
                    <TableHead>
                      <span className="sr-only">Actions</span>
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {paymentsQ.data.map((p) => (
                    <TableRow key={p.id}>
                      <TableCell>{p.date.slice(0, 10)}</TableCell>
                      <TableCell>{p.method || "—"}</TableCell>
                      <TableCell className="text-muted-foreground">{p.reference || "—"}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(p.amount_minor)}</TableCell>
                      <TableCell className="text-right">
                        {invoice.status !== "void" && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setConfirm({ kind: "remove-payment", id: p.id })}
                            aria-label={`Remove payment of ${money(p.amount_minor)}`}
                          >
                            <Trash2 className="size-4" /> Remove
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      )}

      {paying && (
        <PaymentDialog
          open={paying}
          onClose={() => setPaying(false)}
          balanceMinor={balance}
          pending={pay.isPending}
          onSubmit={async (v) => {
            await pay.mutateAsync(v);
            setPaying(false);
          }}
        />
      )}

      <ConfirmDialog
        open={confirm?.kind === "issue"}
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          await issue.mutateAsync(undefined);
          setConfirm(null);
        }}
        title="Issue this invoice?"
        description="It gets the next number and is locked — corrections need a void + reissue. Tracked stock is decremented now."
        confirmLabel="Issue invoice"
        pending={issue.isPending}
      />
      <ConfirmDialog
        open={confirm?.kind === "void"}
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          await voidMut.mutateAsync(undefined);
          setConfirm(null);
        }}
        title="Void this invoice?"
        description="Stock is restored and the number is kept on record. This can't be undone."
        confirmLabel="Void invoice"
        destructive
        pending={voidMut.isPending}
      />
      <ConfirmDialog
        open={confirm?.kind === "delete"}
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          await deleteDraft.mutateAsync(undefined);
          onDeleted();
        }}
        title="Delete this draft?"
        description="Drafts have no number and no stock effect — deleting is permanent."
        confirmLabel="Delete draft"
        destructive
        pending={deleteDraft.isPending}
      />
      <ConfirmDialog
        open={confirm?.kind === "remove-payment"}
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          if (confirm?.kind === "remove-payment") await removePayment.mutateAsync(confirm.id);
          setConfirm(null);
        }}
        title="Remove this payment?"
        description="The invoice balance increases and its status is recalculated. Use this to correct a mis-entered payment."
        confirmLabel="Remove payment"
        destructive
        pending={removePayment.isPending}
      />
    </div>
  );
}

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-muted-foreground">{label}</span>
      <span className={strong ? "font-semibold tabular-nums" : "tabular-nums"}>{value}</span>
    </div>
  );
}
