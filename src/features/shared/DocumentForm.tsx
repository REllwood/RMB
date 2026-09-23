import { useState } from "react";

import { useNav, useUnsavedEdits } from "@/app/nav";
import { ipc } from "@/lib/ipc";
import { DOCUMENT_KEYS } from "@/lib/query";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import { useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";
import { LineEditor } from "@/features/shared/LineEditor";
import {
  emptyLine,
  hasLineIssues,
  previewTotals,
  toLineInputs,
  validateEditLines,
  type EditLine,
} from "@/features/shared/lines";

export type DocumentInitial = {
  id: number;
  customer_id: number;
  date: string | null; // due date (invoice) / valid until (quote)
  notes: string;
  lines: EditLine[];
  /** Set when the customer can't change (e.g. a draft billing a job); explains why. */
  customerLockedReason?: string;
};

/**
 * Create/edit form shared by invoices and quotes: customer, date, notes, and the catalog-aware
 * line editor. With `initial` it edits an existing draft; otherwise it creates one.
 */
export function DocumentForm({
  kind,
  initial,
  onSaved,
  onCancel,
}: {
  kind: "invoice" | "quote";
  initial?: DocumentInitial;
  onSaved: (id: number) => void;
  onCancel?: () => void;
}) {
  const isInvoice = kind === "invoice";
  const customersQ = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const itemsQ = useIpcQuery(["items", ""], () => ipc.listItems());
  const settingsQ = useIpcQuery(["settings"], () => ipc.getSettings());
  const money = useMoneyFormat();
  const goTo = useNav();

  const [customerId, setCustomerId] = useState<number | null>(initial?.customer_id ?? null);
  const [date, setDate] = useState(initial?.date ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  // Lines start once tax rates load so new documents default to the business's main rate.
  const [edited, setEdited] = useState<EditLine[] | null>(initial?.lines ?? null);
  // Problems are shown once the user tries to save, not while a new form is still being filled.
  const [attempted, setAttempted] = useState(false);

  const save = useIpcMutation(
    async (v: {
      customerId: number;
      lines: ReturnType<typeof toLineInputs>;
      date: string | null;
      notes: string;
    }) => {
      if (initial) {
        if (isInvoice)
          await ipc.updateInvoiceDraft(initial.id, v.customerId, v.lines, v.date, v.notes);
        else await ipc.updateQuoteDraft(initial.id, v.customerId, v.lines, v.date, v.notes);
        return initial.id;
      }
      return isInvoice
        ? ipc.createInvoice(v.customerId, v.lines, v.date, v.notes)
        : ipc.createQuote(v.customerId, v.lines, v.date, v.notes);
    },
    DOCUMENT_KEYS,
    { successMessage: initial ? "Draft updated" : undefined },
  );
  useUnsavedEdits({ customerId, date, notes, edited });

  if (customersQ.isLoading || taxQ.isLoading || itemsQ.isLoading || settingsQ.isLoading)
    return <Loading />;
  const loadError = customersQ.error ?? taxQ.error ?? itemsQ.error;
  if (loadError)
    return (
      <ErrorState
        error={loadError}
        onRetry={() => Promise.all([customersQ.refetch(), taxQ.refetch(), itemsQ.refetch()])}
      />
    );
  if (customersQ.data && customersQ.data.length === 0)
    return (
      <EmptyState
        title="Add a customer first"
        description={`${isInvoice ? "Invoices" : "Quotes"} need a customer.`}
        action={<Button onClick={() => goTo("customers")}>Go to Customers</Button>}
      />
    );

  const taxes = taxQ.data ?? [];
  const items = itemsQ.data ?? [];
  const defaultTaxRateId = settingsQ.data?.default_tax_rate_id ?? null;
  const lines = edited ?? [emptyLine(taxes, defaultTaxRateId)];
  const lineIssues = validateEditLines(lines, items);
  const lineProblems = hasLineIssues(lineIssues);
  const payload = toLineInputs(lines);
  const totals = previewTotals(lines);

  async function onSave() {
    setAttempted(true);
    if (customerId === null || payload.length === 0 || lineProblems) return;
    try {
      const id = await save.mutateAsync({ customerId, lines: payload, date: date || null, notes });
      onSaved(id);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          {initial
            ? `Edit draft ${isInvoice ? "invoice" : "quote"}`
            : `New ${isInvoice ? "invoice" : "quote"}`}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid max-w-xl gap-4 sm:grid-cols-2">
          <Field
            label="Customer"
            required
            hint={initial?.customerLockedReason}
            error={attempted && customerId === null ? "Choose a customer" : undefined}
          >
            {(p) => (
              <Select
                {...p}
                disabled={Boolean(initial?.customerLockedReason)}
                value={customerId ?? ""}
                onChange={(e) => setCustomerId(e.target.value ? Number(e.target.value) : null)}
              >
                <option value="" disabled>
                  Choose a customer…
                </option>
                {customersQ.data?.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          <Field
            label={isInvoice ? "Due date" : "Valid until"}
            hint={
              isInvoice && !date && settingsQ.data?.default_due_days != null
                ? `Blank: due ${settingsQ.data.default_due_days} days after the invoice is issued`
                : undefined
            }
          >
            {(p) => (
              <Input {...p} type="date" value={date} onChange={(e) => setDate(e.target.value)} />
            )}
          </Field>
        </div>

        <LineEditor
          lines={lines}
          onChange={setEdited}
          taxes={taxes}
          items={items}
          idPrefix={kind}
          issues={attempted ? lineIssues : []}
          defaultTaxRateId={defaultTaxRateId}
        />

        <dl className="ml-auto grid max-w-xs grid-cols-[1fr_auto] gap-x-6 gap-y-1 text-sm">
          <dt className="text-muted-foreground">Subtotal</dt>
          <dd className="text-right tabular-nums">{money(totals.subtotal)}</dd>
          <dt className="text-muted-foreground">Tax</dt>
          <dd className="text-right tabular-nums">{money(totals.tax)}</dd>
          <dt className="font-medium">Total</dt>
          <dd className="text-right font-semibold tabular-nums">{money(totals.total)}</dd>
        </dl>

        <Field label="Notes">
          {(p) => <Textarea {...p} value={notes} onChange={(e) => setNotes(e.target.value)} />}
        </Field>

        <div className="flex gap-2">
          <Button
            onClick={onSave}
            disabled={payload.length === 0}
            loading={save.isPending}
            loadingLabel="Saving…"
          >
            {initial ? "Save changes" : "Save draft"}
          </Button>
          {onCancel && (
            <Button variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
