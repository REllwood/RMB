import { useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { FileDown, Hammer, Pencil, Plus, Receipt, Trash2 } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import { useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { PageHeader } from "@/components/ui/page-header";
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

type View =
  | { mode: "list" }
  | { mode: "create" }
  | { mode: "edit"; id: number }
  | { mode: "detail"; id: number };

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
      <PageHeader
        title="Quotes"
        description="Estimate the work, send it, then convert to a job or invoice."
        actions={
          view.mode === "list" ? (
            <Button onClick={() => setView({ mode: "create" })}>
              <Plus className="size-4" /> New quote
            </Button>
          ) : (
            <Button variant="ghost" onClick={() => setView({ mode: "list" })}>
              ← Back to list
            </Button>
          )
        }
      />
      {view.mode === "list" && <QuoteList onOpen={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "create" && (
        <DocumentForm
          kind="quote"
          onSaved={(id) => setView({ mode: "detail", id })}
          onCancel={() => setView({ mode: "list" })}
        />
      )}
      {view.mode === "edit" && (
        <QuoteEdit id={view.id} onDone={() => setView({ mode: "detail", id: view.id })} />
      )}
      {view.mode === "detail" && (
        <QuoteDetailView id={view.id} onEdit={() => setView({ mode: "edit", id: view.id })} onDeleted={() => setView({ mode: "list" })} />
      )}
    </div>
  );
}

function useCustomerNames(): Map<number, string> {
  const q = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  return useMemo(() => new Map((q.data ?? []).map((c) => [c.id, c.name])), [q.data]);
}

function QuoteList({ onOpen }: { onOpen: (id: number) => void }) {
  const money = useMoneyFormat();
  const names = useCustomerNames();
  const q = useIpcQuery(["quotes"], () => ipc.listQuotes());
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data || q.data.length === 0)
    return <EmptyState title="No quotes yet" description="Create a quote, then convert it to a job or invoice when accepted." />;
  return (
    <Card className="overflow-hidden py-0">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="pl-4">Number</TableHead>
            <TableHead>Customer</TableHead>
            <TableHead>Valid until</TableHead>
            <TableHead>Status</TableHead>
            <TableHead className="text-right">Total</TableHead>
            <TableHead>
              <span className="sr-only">Open</span>
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {q.data.map((row) => (
            <TableRow key={row.id} className="cursor-pointer" onClick={() => onOpen(row.id)}>
              <TableCell className="pl-4 font-medium">{row.number ?? `#${row.id}`}</TableCell>
              <TableCell>{names.get(row.customer_id) ?? "—"}</TableCell>
              <TableCell className="text-muted-foreground">{row.valid_until ?? "—"}</TableCell>
              <TableCell>
                <StatusBadge status={row.status} />
              </TableCell>
              <TableCell className="text-right tabular-nums">{money(row.total_minor)}</TableCell>
              <TableCell className="pr-4 text-right">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    onOpen(row.id);
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

function QuoteEdit({ id, onDone }: { id: number; onDone: () => void }) {
  const q = useIpcQuery(["quote", id], () => ipc.getQuote(id));
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Quote not found" />;
  if (q.data.quote.status !== "draft")
    return <EmptyState title="Only draft quotes can be edited" description="Pull a sent quote back to draft first." />;
  return (
    <DocumentForm
      kind="quote"
      initial={{
        id,
        customer_id: q.data.quote.customer_id,
        date: q.data.quote.valid_until,
        notes: q.data.quote.notes,
        lines: fromRows(q.data.lines),
      }}
      onSaved={onDone}
      onCancel={onDone}
    />
  );
}

function QuoteDetailView({
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
  const q = useIpcQuery(["quote", id], () => ipc.getQuote(id));
  const jobsQ = useIpcQuery(["jobs"], () => ipc.listJobs());
  const refresh: unknown[][] = [["quote", id], ["quotes"]];
  const setStatus = useIpcMutation((s: string) => ipc.setQuoteStatus(id, s), refresh);
  const convert = useIpcMutation(() => ipc.convertQuoteToInvoice(id), [...refresh, ["invoices"]], {
    successMessage: "Draft invoice created — see Invoices",
  });
  const toJob = useIpcMutation(() => ipc.convertQuoteToJob(id), [...refresh, ["jobs"]], {
    successMessage: "Job created — see Jobs",
  });
  const deleteQuote = useIpcMutation(() => ipc.deleteQuote(id), [["quotes"]], {
    successMessage: "Quote deleted",
  });
  const [confirmDelete, setConfirmDelete] = useState(false);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Quote not found" />;

  const { quote, lines } = q.data;
  const linkedJob = (jobsQ.data ?? []).find((j) => j.source_quote_id === quote.id);
  const deletable = quote.status === "draft" || quote.status === "declined" || quote.status === "expired";

  async function onExportPdf() {
    const path = await save({
      defaultPath: `${quote.number ?? `quote-${quote.id}`}.pdf`,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (path) await ipc.exportQuotePdf(quote.id, path);
  }

  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between">
        <div className="space-y-1">
          <CardTitle>{quote.number ?? `Quote #${quote.id}`}</CardTitle>
          <p className="text-sm text-muted-foreground">
            {names.get(quote.customer_id) ?? "—"}
            {quote.valid_until && ` · valid until ${quote.valid_until}`}
          </p>
        </div>
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
                <TableCell className="text-right tabular-nums">{l.quantity}</TableCell>
                <TableCell className="text-right tabular-nums">{money(l.unit_price_minor)}</TableCell>
                <TableCell className="text-right tabular-nums">{money(l.gross_minor)}</TableCell>
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
          <Button variant="outline" onClick={onExportPdf}>
            <FileDown className="size-4" /> Export PDF
          </Button>
          {quote.status === "draft" && (
            <>
              <Button variant="outline" onClick={onEdit}>
                <Pencil className="size-4" /> Edit
              </Button>
              <Button onClick={() => setStatus.mutate("sent")}>Mark sent</Button>
            </>
          )}
          {quote.status === "sent" && (
            <>
              <Button onClick={() => setStatus.mutate("accepted")}>Mark accepted</Button>
              <Button variant="outline" onClick={() => setStatus.mutate("declined")}>
                Decline
              </Button>
              <Button variant="ghost" onClick={() => setStatus.mutate("draft")}>
                Back to draft
              </Button>
            </>
          )}
          {quote.status === "accepted" && (
            <>
              <Button onClick={() => convert.mutate(undefined)} disabled={convert.isPending}>
                <Receipt className="size-4" /> Convert to invoice
              </Button>
              <Button variant="outline" onClick={() => toJob.mutate(undefined)} disabled={toJob.isPending}>
                <Hammer className="size-4" /> Convert to job
              </Button>
            </>
          )}
          {deletable && (
            <Button variant="ghost" onClick={() => setConfirmDelete(true)}>
              <Trash2 className="size-4" /> Delete
            </Button>
          )}
          {quote.converted_invoice_id && (
            <p className="self-center text-sm text-muted-foreground">
              Converted → invoice #{quote.converted_invoice_id}
            </p>
          )}
          {linkedJob && (
            <p className="self-center text-sm text-muted-foreground">Converted → {linkedJob.title}</p>
          )}
        </div>
      </CardContent>

      <ConfirmDialog
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={async () => {
          await deleteQuote.mutateAsync(undefined);
          onDeleted();
        }}
        title="Delete this quote?"
        description="The quote is removed from your lists. Its number is not reused."
        confirmLabel="Delete quote"
        destructive
        pending={deleteQuote.isPending}
      />
    </Card>
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
