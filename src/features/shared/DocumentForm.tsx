import { useState } from "react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import { parseMoney, useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";
import { LineEditor } from "@/features/shared/LineEditor";
import { emptyLine, toLineInputs, type EditLine } from "@/features/shared/lines";

export type DocumentInitial = {
  id: number;
  customer_id: number;
  date: string | null; // due date (invoice) / valid until (quote)
  notes: string;
  lines: EditLine[];
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
  const money = useMoneyFormat();

  const [customerId, setCustomerId] = useState<number | null>(initial?.customer_id ?? null);
  const [date, setDate] = useState(initial?.date ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  // Lines start once tax rates load so new documents default to the business's main rate.
  const [edited, setEdited] = useState<EditLine[] | null>(initial?.lines ?? null);

  const listKey = isInvoice ? ["invoices"] : ["quotes"];
  const detailKey = initial ? [isInvoice ? "invoice" : "quote", initial.id] : null;
  const invalidate = detailKey ? [listKey, detailKey] : [listKey];

  const save = useIpcMutation(
    async (v: { customerId: number; lines: ReturnType<typeof toLineInputs>; date: string | null; notes: string }) => {
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
    invalidate,
    { successMessage: initial ? "Draft updated" : undefined },
  );

  if (customersQ.isLoading || taxQ.isLoading || itemsQ.isLoading) return <Loading />;
  if (customersQ.error)
    return <ErrorState error={customersQ.error} onRetry={() => customersQ.refetch()} />;
  if (customersQ.data && customersQ.data.length === 0)
    return (
      <EmptyState
        title="Add a customer first"
        description={`${isInvoice ? "Invoices" : "Quotes"} need a customer — create one under Customers.`}
      />
    );

  const taxes = taxQ.data ?? [];
  const items = itemsQ.data ?? [];
  const lines = edited ?? [emptyLine(taxes)];
  const payload = toLineInputs(lines);
  const netPreview = lines.reduce(
    (sum, l) => sum + Math.round((parseMoney(l.price) ?? 0) * (Number(l.quantity) || 0)),
    0,
  );

  async function onSave() {
    if (customerId === null || payload.length === 0) return;
    const id = await save.mutateAsync({ customerId, lines: payload, date: date || null, notes });
    onSaved(id);
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
          <Field label="Customer" required>
            {(p) => (
              <Select
                {...p}
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
          <Field label={isInvoice ? "Due date" : "Valid until"}>
            {(p) => <Input {...p} type="date" value={date} onChange={(e) => setDate(e.target.value)} />}
          </Field>
        </div>

        <LineEditor
          lines={lines}
          onChange={setEdited}
          taxes={taxes}
          items={items}
          idPrefix={kind}
        />

        <div className="flex items-center justify-end">
          <p className="text-sm text-muted-foreground">
            Approx. subtotal (excl. tax):{" "}
            <span className="font-medium text-foreground tabular-nums">{money(netPreview)}</span>
          </p>
        </div>

        <Field label="Notes">
          {(p) => <Input {...p} value={notes} onChange={(e) => setNotes(e.target.value)} />}
        </Field>

        <div className="flex gap-2">
          <Button onClick={onSave} disabled={customerId === null || payload.length === 0 || save.isPending}>
            {save.isPending ? "Saving…" : initial ? "Save changes" : `Save draft`}
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
