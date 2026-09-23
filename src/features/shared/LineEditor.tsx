import { Plus, X } from "lucide-react";

import type { Item, TaxRate } from "@/lib/types";
import { minorToInput, useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  emptyLine,
  hasLineIssues,
  lineAmount,
  lineIssueMessages,
  NO_TAX,
  taxKey,
  taxLabel,
  taxOptions,
  toChoice,
  type EditLine,
  type LineIssue,
} from "@/features/shared/lines";

/**
 * Line-item editor with a catalog picker. Choosing an item links the line (`item_id` — which is
 * what makes tracked stock decrement at issue) and applies the item's default price + tax;
 * description and price stay editable per line. "Custom" lines are free-typed.
 */
export function LineEditor({
  lines,
  onChange,
  taxes,
  items,
  idPrefix,
  issues = [],
  defaultTaxRateId,
}: {
  lines: EditLine[];
  onChange: (lines: EditLine[]) => void;
  taxes: TaxRate[];
  items: Item[];
  idPrefix: string;
  issues?: LineIssue[];
  /** The business's default tax for new lines. */
  defaultTaxRateId?: number | null;
}) {
  const money = useMoneyFormat();
  const products = items.filter((i) => i.kind === "product");
  const services = items.filter((i) => i.kind !== "product");

  function patch(i: number, p: Partial<EditLine>) {
    onChange(lines.map((l, idx) => (idx === i ? { ...l, ...p } : l)));
  }

  function pickItem(i: number, value: string) {
    if (!value) {
      patch(i, { item_id: null });
      return;
    }
    const it = items.find((x) => x.id === Number(value));
    if (!it) return;
    const tax = taxes.find((t) => t.id === it.default_tax_rate_id);
    patch(i, {
      item_id: it.id,
      description: it.name,
      price: minorToInput(it.default_price_minor),
      tax: tax ? toChoice(tax) : NO_TAX,
    });
  }

  function itemOption(it: Item) {
    return (
      <option key={it.id} value={it.id}>
        {it.tracked ? `${it.name} (${it.qty_on_hand} in stock)` : it.name}
      </option>
    );
  }

  return (
    <div className="space-y-3">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-44">Item</TableHead>
            <TableHead>Description</TableHead>
            <TableHead className="w-20">Qty</TableHead>
            <TableHead className="w-28">Unit price</TableHead>
            <TableHead className="w-36">Tax</TableHead>
            <TableHead className="w-24 text-right">Amount</TableHead>
            <TableHead className="w-10">
              <span className="sr-only">Remove</span>
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {lines.map((l, i) => {
            // Offer No Tax + active rates; keep the line's stored rate visible even if retired.
            const options = taxOptions(taxes, l.tax);
            const issue = issues[i] ?? {};
            const errorSummaryId = `${idPrefix}-line-errors`;
            return (
              <TableRow key={i}>
                <TableCell>
                  <label className="sr-only" htmlFor={`${idPrefix}-item-${i}`}>
                    Catalog item
                  </label>
                  <Select
                    id={`${idPrefix}-item-${i}`}
                    aria-invalid={Boolean(issue.item)}
                    aria-describedby={issue.item ? errorSummaryId : undefined}
                    value={l.item_id ?? ""}
                    onChange={(e) => pickItem(i, e.target.value)}
                  >
                    <option value="">Custom</option>
                    {l.item_id !== null && !items.some((it) => it.id === l.item_id) && (
                      <option value={l.item_id} disabled>
                        Archived item
                      </option>
                    )}
                    {products.length > 0 && (
                      <optgroup label="Products">{products.map(itemOption)}</optgroup>
                    )}
                    {services.length > 0 && (
                      <optgroup label="Services">{services.map(itemOption)}</optgroup>
                    )}
                  </Select>
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`${idPrefix}-desc-${i}`}>
                    Description
                  </label>
                  <Input
                    id={`${idPrefix}-desc-${i}`}
                    maxLength={2000}
                    aria-invalid={Boolean(issue.description)}
                    aria-describedby={issue.description ? errorSummaryId : undefined}
                    value={l.description}
                    onChange={(e) => patch(i, { description: e.target.value })}
                  />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`${idPrefix}-qty-${i}`}>
                    Quantity
                  </label>
                  <Input
                    id={`${idPrefix}-qty-${i}`}
                    aria-invalid={Boolean(issue.quantity)}
                    aria-describedby={issue.quantity ? errorSummaryId : undefined}
                    inputMode="decimal"
                    value={l.quantity}
                    onChange={(e) => patch(i, { quantity: e.target.value })}
                  />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`${idPrefix}-price-${i}`}>
                    Unit price
                  </label>
                  <Input
                    id={`${idPrefix}-price-${i}`}
                    aria-invalid={Boolean(issue.price)}
                    aria-describedby={issue.price ? errorSummaryId : undefined}
                    inputMode="decimal"
                    value={l.price}
                    onChange={(e) => patch(i, { price: e.target.value })}
                  />
                </TableCell>
                <TableCell>
                  <label className="sr-only" htmlFor={`${idPrefix}-tax-${i}`}>
                    Tax rate
                  </label>
                  <Select
                    id={`${idPrefix}-tax-${i}`}
                    value={taxKey(l.tax)}
                    onChange={(e) => {
                      const chosen = options.find((o) => taxKey(o) === e.target.value);
                      if (chosen) patch(i, { tax: chosen });
                    }}
                  >
                    {options.map((o) => (
                      <option key={taxKey(o)} value={taxKey(o)}>
                        {taxLabel(o)}
                      </option>
                    ))}
                  </Select>
                </TableCell>
                <TableCell className="text-right tabular-nums text-muted-foreground">
                  {money(lineAmount(l))}
                </TableCell>
                <TableCell className="text-right">
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => onChange(lines.filter((_, idx) => idx !== i))}
                    aria-label={`Remove line ${i + 1}`}
                  >
                    <X className="size-4" />
                  </Button>
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
      {hasLineIssues(issues) && (
        <div
          id={`${idPrefix}-line-errors`}
          role="alert"
          className="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive"
        >
          {issues
            .flatMap((issue, index) =>
              lineIssueMessages(issue).map((message) => `Line ${index + 1}: ${message}`),
            )
            .join(" · ")}
        </div>
      )}
      <Button
        variant="outline"
        size="sm"
        onClick={() => onChange([...lines, emptyLine(taxes, defaultTaxRateId)])}
      >
        <Plus className="size-4" /> Add line
      </Button>
    </div>
  );
}
