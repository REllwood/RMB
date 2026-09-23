import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { FileDown, Hammer, Pencil, Plus, Receipt, Trash2 } from "lucide-react";

import { useNav, useView } from "@/app/nav";
import { ipc } from "@/lib/ipc";
import { DOCUMENT_KEYS } from "@/lib/query";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import { useMoneyFormat } from "@/lib/money";
import { todayLocalISO } from "@/lib/format";
import type { QuoteRow } from "@/lib/types";
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
import { useToast } from "@/components/ui/toast";
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

/** An open quote whose valid-until date has passed (it can still be accepted, with a warning). */
function pastValidity(quote: Pick<QuoteRow, "status" | "valid_until">): boolean {
  return (
    (quote.status === "draft" || quote.status === "sent") &&
    quote.valid_until !== null &&
    quote.valid_until < todayLocalISO()
  );
}

function StatusBadge({ status }: { status: string }) {
  return <Badge variant={STATUS_VARIANT[status] ?? "outline"}>{status}</Badge>;
}

export function QuotesPage() {
  const [view, setView] = useView<View>((recordId) =>
    recordId === null ? { mode: "list" } : { mode: "detail", id: recordId },
  );
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
        <QuoteDetailView
          id={view.id}
          onEdit={() => setView({ mode: "edit", id: view.id })}
          onDeleted={() => setView({ mode: "list" })}
        />
      )}
    </div>
  );
}

function QuoteList({ onOpen }: { onOpen: (id: number) => void }) {
  const money = useMoneyFormat();
  const q = useIpcQuery(["quotes"], () => ipc.listQuotes());
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data || q.data.length === 0)
    return (
      <EmptyState
        title="No quotes yet"
        description="Create a quote, then convert it to a job or invoice when accepted."
      />
    );
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
              <TableCell>{row.customer_name || "—"}</TableCell>
              <TableCell className="text-muted-foreground">{row.valid_until ?? "—"}</TableCell>
              <TableCell>
                <StatusBadge status={row.status} />
                {pastValidity(row) && <span className="sr-only">, past its valid-until date</span>}
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
                  aria-label={`Open quote ${row.number ?? row.id}`}
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
    return (
      <EmptyState
        title="Only draft quotes can be edited"
        description="Pull a sent quote back to draft first."
      />
    );
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
  const toast = useToast();
  const money = useMoneyFormat();
  const goTo = useNav();
  const q = useIpcQuery(["quote", id], () => ipc.getQuote(id));
  const jobsQ = useIpcQuery(["jobs"], () => ipc.listJobs());
  const refresh = DOCUMENT_KEYS;
  const setStatus = useIpcMutation((s: string) => ipc.setQuoteStatus(id, s), refresh);
  const convert = useIpcMutation(() => ipc.convertQuoteToInvoice(id), refresh, {
    successMessage: "Draft invoice created",
  });
  const toJob = useIpcMutation(() => ipc.convertQuoteToJob(id), refresh, {
    successMessage: "Job created",
  });
  const deleteQuote = useIpcMutation(() => ipc.deleteQuote(id), refresh, {
    successMessage: "Quote deleted",
  });
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [action, setAction] = useState<QuoteAction | null>(null);
  const [exporting, setExporting] = useState(false);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Quote not found" />;

  const { quote, lines } = q.data;
  const linkedJob = (jobsQ.data ?? []).find((j) => j.source_quote_id === quote.id);
  const deletable =
    quote.status === "draft" || quote.status === "declined" || quote.status === "expired";

  async function onExportPdf() {
    setExporting(true);
    try {
      const path = await save({
        defaultPath: `${quote.number ?? `quote-${quote.id}`}.pdf`,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (path) {
        await ipc.exportQuotePdf(quote.id, path);
        toast("success", "Quote PDF exported");
      }
    } catch (error) {
      toast("error", error instanceof Error ? error.message : String(error));
    } finally {
      setExporting(false);
    }
  }

  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between">
        <div className="space-y-1">
          <CardTitle>{quote.number ?? `Quote #${quote.id}`}</CardTitle>
          <p className="text-sm text-muted-foreground">
            {quote.customer_name || "—"}
            {quote.valid_until && ` · valid until ${quote.valid_until}`}
          </p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <StatusBadge status={quote.status} />
          {pastValidity(quote) && (
            <span className="text-xs text-muted-foreground">
              Past its valid-until date — mark it expired or re-check prices
            </span>
          )}
        </div>
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
                <TableCell className="text-right tabular-nums">
                  {money(l.unit_price_minor)}
                </TableCell>
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
          <Button
            variant="outline"
            onClick={onExportPdf}
            loading={exporting}
            loadingLabel="Exporting…"
          >
            <FileDown className="size-4" /> Export PDF
          </Button>
          {quote.status === "draft" && (
            <>
              <Button variant="outline" onClick={onEdit} disabled={setStatus.isPending}>
                <Pencil className="size-4" /> Edit
              </Button>
              <Button
                onClick={() => setStatus.mutate("sent")}
                loading={setStatus.isPending && setStatus.variables === "sent"}
                loadingLabel="Updating…"
              >
                Mark sent
              </Button>
              <Button variant="outline" onClick={() => setAction("accepted")}>
                Mark accepted
              </Button>
              <Button variant="ghost" onClick={() => setAction("declined")}>
                Decline
              </Button>
            </>
          )}
          {quote.status === "sent" && (
            <>
              <Button onClick={() => setAction("accepted")} disabled={setStatus.isPending}>
                Mark accepted
              </Button>
              <Button
                variant="outline"
                onClick={() => setAction("declined")}
                disabled={setStatus.isPending}
              >
                Decline
              </Button>
              <Button
                variant="ghost"
                onClick={() => setStatus.mutate("draft")}
                disabled={setStatus.isPending}
                loading={setStatus.isPending && setStatus.variables === "draft"}
                loadingLabel="Updating…"
              >
                Back to draft
              </Button>
              <Button
                variant="ghost"
                onClick={() => setAction("expired")}
                disabled={setStatus.isPending}
              >
                Mark expired
              </Button>
            </>
          )}
          {quote.status === "accepted" && (
            <>
              <Button onClick={() => setAction("to-invoice")}>
                <Receipt className="size-4" /> Convert to invoice
              </Button>
              <Button variant="outline" onClick={() => setAction("to-job")}>
                <Hammer className="size-4" /> Convert to job
              </Button>
              <Button variant="ghost" onClick={() => setAction("declined")}>
                Decline
              </Button>
            </>
          )}
          {deletable && (
            <Button
              variant="ghost"
              onClick={() => setConfirmDelete(true)}
              disabled={setStatus.isPending}
            >
              <Trash2 className="size-4" /> Delete
            </Button>
          )}
          {quote.converted_invoice_id !== null && (
            <Button
              variant="link"
              onClick={() =>
                quote.converted_invoice_id !== null && goTo("invoices", quote.converted_invoice_id)
              }
            >
              Converted to invoice{" "}
              {quote.converted_invoice_number ?? `(draft #${quote.converted_invoice_id})`}
            </Button>
          )}
          {linkedJob && (
            <Button variant="link" onClick={() => goTo("jobs", linkedJob.id)}>
              Converted to job: {linkedJob.title}
            </Button>
          )}
        </div>
      </CardContent>

      {action && (
        <ConfirmDialog
          open
          onClose={() => setAction(null)}
          onConfirm={async () => {
            try {
              if (action === "to-invoice") {
                const invoiceId = await convert.mutateAsync(undefined);
                setAction(null);
                goTo("invoices", invoiceId);
                return;
              }
              if (action === "to-job") {
                const jobId = await toJob.mutateAsync(undefined);
                setAction(null);
                goTo("jobs", jobId);
                return;
              }
              await setStatus.mutateAsync(action);
              setAction(null);
            } catch {
              // Keep the dialog open; the error is shown above it.
            }
          }}
          {...QUOTE_ACTIONS[action]}
          description={
            action === "accepted" && pastValidity(quote)
              ? `This quote was valid until ${quote.valid_until}; check its prices still stand. ${QUOTE_ACTIONS.accepted.description}`
              : QUOTE_ACTIONS[action].description
          }
          pending={setStatus.isPending || convert.isPending || toJob.isPending}
        />
      )}
      <ConfirmDialog
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={async () => {
          try {
            await deleteQuote.mutateAsync(undefined);
            onDeleted();
          } catch {
            // Keep the dialog open so the backend validation remains actionable.
          }
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

type QuoteAction = "accepted" | "declined" | "expired" | "to-invoice" | "to-job";

/** Each quote action can't be undone, so each is confirmed with what it means. */
const QUOTE_ACTIONS: Record<
  QuoteAction,
  { title: string; description: string; confirmLabel: string; destructive?: boolean }
> = {
  accepted: {
    title: "Mark this quote accepted?",
    description:
      "An accepted quote can no longer be edited. You can then convert it to an invoice or a job.",
    confirmLabel: "Mark accepted",
  },
  declined: {
    title: "Decline this quote?",
    description: "A declined quote is closed for good. It can be deleted but not reopened.",
    confirmLabel: "Decline quote",
    destructive: true,
  },
  expired: {
    title: "Mark this quote expired?",
    description: "An expired quote is closed for good. It can be deleted but not reopened.",
    confirmLabel: "Mark expired",
    destructive: true,
  },
  "to-invoice": {
    title: "Convert to an invoice?",
    description:
      "A draft invoice is created from this quote's lines for you to review. The quote can then no longer become a job.",
    confirmLabel: "Create draft invoice",
  },
  "to-job": {
    title: "Convert to a job?",
    description:
      "A job is created with this quote's lines as its materials, ready for time to be logged. The quote can then no longer become an invoice directly.",
    confirmLabel: "Create job",
  },
};

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-muted-foreground">{label}</span>
      <span className={strong ? "font-semibold tabular-nums" : "tabular-nums"}>{value}</span>
    </div>
  );
}
