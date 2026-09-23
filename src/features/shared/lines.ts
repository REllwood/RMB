import type { Item, LineInput, TaxRate } from "@/lib/types";
import { lineAmountMinor, lineTaxMinor, minorToInput, parseDecimal, parseMoney } from "@/lib/money";

/** Tax applied to a line — carried by value so archived/legacy rates survive editing. */
export type TaxChoice = { name: string; bp: number; inclusive: boolean };

export const NO_TAX: TaxChoice = { name: "No Tax", bp: 0, inclusive: false };

/** One editable document line. `item_id` links the line to the catalog (stock, defaults). */
export type EditLine = {
  item_id: number | null;
  description: string;
  quantity: string;
  price: string; // major units while editing, e.g. "12.50"
  tax: TaxChoice;
};

export type LineIssue = {
  item?: string;
  description?: string;
  quantity?: string;
  price?: string;
};

/** The messages for one line, in the order the fields appear. */
export function lineIssueMessages(issue: LineIssue): string[] {
  return [issue.item, issue.description, issue.quantity, issue.price].filter(
    (message): message is string => Boolean(message),
  );
}

export function hasLineIssues(issues: LineIssue[]): boolean {
  return issues.some((issue) => lineIssueMessages(issue).length > 0);
}

export const taxKey = (t: TaxChoice) => `${t.name}|${t.bp}|${t.inclusive ? 1 : 0}`;

/** "VAT 20% (20%)" is noisy, so the rate is only appended when the name doesn't already say it. */
export function taxLabel(t: TaxChoice): string {
  const percent = `${Number((t.bp / 100).toFixed(2))}%`;
  const detail = t.inclusive ? `${percent}, incl.` : percent;
  return t.name.includes(percent) && !t.inclusive ? t.name : `${t.name} (${detail})`;
}

export function toChoice(t: TaxRate): TaxChoice {
  return { name: t.name, bp: t.rate_bp, inclusive: t.inclusive };
}

/**
 * The tax a new line starts with: the business's chosen default rate, otherwise its first
 * non-zero rate (so a UK business starts on VAT 20%, not "No Tax"), otherwise No Tax.
 */
export function defaultTax(taxes: TaxRate[], defaultTaxRateId?: number | null): TaxChoice {
  const active = taxes.filter((t) => !t.archived);
  const chosen = active.find((t) => t.id === defaultTaxRateId) ?? active.find((t) => t.rate_bp > 0);
  return chosen ? toChoice(chosen) : NO_TAX;
}

/** The tax options for a line: No Tax, the active rates, and the line's own rate if it's retired. */
export function taxOptions(taxes: TaxRate[], current?: TaxChoice): TaxChoice[] {
  const options: TaxChoice[] = [NO_TAX];
  for (const rate of taxes.filter((t) => !t.archived).map(toChoice)) {
    if (!options.some((o) => taxKey(o) === taxKey(rate))) options.push(rate);
  }
  if (current && !options.some((o) => taxKey(o) === taxKey(current))) options.unshift(current);
  return options;
}

/** A fresh line on the business's default tax. */
export function emptyLine(taxes: TaxRate[], defaultTaxRateId?: number | null): EditLine {
  return {
    item_id: null,
    description: "",
    quantity: "1",
    price: "0.00",
    tax: defaultTax(taxes, defaultTaxRateId),
  };
}

/** Rebuild editable lines from stored document rows (for editing drafts). */
export function fromRows(
  rows: Array<{
    item_id: number | null;
    description: string;
    quantity: string;
    unit_price_minor: number;
    tax_rate_name: string;
    tax_rate_bp: number;
    tax_inclusive: boolean;
  }>,
): EditLine[] {
  return rows.map((r) => ({
    item_id: r.item_id,
    description: r.description,
    quantity: r.quantity,
    price: minorToInput(r.unit_price_minor),
    tax: { name: r.tax_rate_name, bp: r.tax_rate_bp, inclusive: r.tax_inclusive },
  }));
}

/** Most quantities fit in 10 decimal places; the backend allows up to 1,000,000,000 units. */
const MAX_QUANTITY_DECIMALS = 10;

/** A positive quantity as a normalised decimal string ("1,5" → "1.5"), or null. */
export function normaliseQuantity(value: string): string | null {
  const normalised = parseDecimal(value, MAX_QUANTITY_DECIMALS);
  if (normalised === null || normalised === "0" || normalised.startsWith("-")) return null;
  const whole = normalised.split(".")[0];
  if (whole.length > 10 || Number(normalised) > 1_000_000_000) return null;
  return normalised;
}

/** Tracked stock moves in whole units, so its quantity must have no fractional part. */
export function isWholeQuantity(normalised: string): boolean {
  return !normalised.includes(".");
}

/** The amount a line will carry, or 0 while its inputs are incomplete. */
export function lineAmount(line: EditLine): number {
  const quantity = normaliseQuantity(line.quantity);
  const price = parseMoney(line.price);
  return quantity === null || price === null ? 0 : lineAmountMinor(price, quantity);
}

/** Subtotal, tax and total for the lines, computed with the same per-line rules as the backend. */
export function previewTotals(lines: EditLine[]): { subtotal: number; tax: number; total: number } {
  return lines.reduce(
    (sum, line) => {
      const split = lineTaxMinor(lineAmount(line), line.tax.bp, line.tax.inclusive);
      return {
        subtotal: sum.subtotal + split.net,
        tax: sum.tax + split.tax,
        total: sum.total + split.gross,
      };
    },
    { subtotal: 0, tax: 0, total: 0 },
  );
}

/** Validate editable rows before an IPC call; the Rust boundary repeats these checks. */
export function validateEditLines(lines: EditLine[], items: Item[] = []): LineIssue[] {
  return lines.map((line) => {
    const quantity = normaliseQuantity(line.quantity);
    const price = parseMoney(line.price);
    const item =
      line.item_id === null ? undefined : items.find((candidate) => candidate.id === line.item_id);
    return {
      item:
        line.item_id !== null && item === undefined
          ? "This catalogue item has been archived — choose another item or Custom"
          : undefined,
      description: !line.description.trim()
        ? "Description is required"
        : line.description.trim().length > 2_000
          ? "Description cannot exceed 2,000 characters"
          : undefined,
      quantity:
        quantity === null
          ? "Enter a quantity greater than zero"
          : item?.tracked && !isWholeQuantity(quantity)
            ? "Tracked products need a whole quantity"
            : undefined,
      price:
        price === null
          ? "Enter a valid price with no more than two decimal places (use a minus sign for a discount)"
          : undefined,
    };
  });
}

/** Convert validated editor state to the backend payload. Invalid values fail closed. */
export function toLineInputs(lines: EditLine[]): LineInput[] {
  return lines.map((l) => ({
    item_id: l.item_id,
    description: l.description.trim(),
    quantity: normaliseQuantity(l.quantity) ?? "0",
    unit_price_minor: parseMoney(l.price) ?? -1,
    tax_rate_name: l.tax.name,
    tax_rate_bp: l.tax.bp,
    tax_inclusive: l.tax.inclusive,
  }));
}
