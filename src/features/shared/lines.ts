import type { Item, LineInput, TaxRate } from "@/lib/types";
import { minorToInput, parseMoney } from "@/lib/money";

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
  description?: string;
  quantity?: string;
  price?: string;
};

export const taxKey = (t: TaxChoice) => `${t.name}|${t.bp}|${t.inclusive ? 1 : 0}`;

export function toChoice(t: TaxRate): TaxChoice {
  return { name: t.name, bp: t.rate_bp, inclusive: t.inclusive };
}

/** A fresh line, defaulting to the business's first tax rate (e.g. GST) when one exists. */
export function emptyLine(taxes: TaxRate[]): EditLine {
  const first = taxes.find((t) => !t.archived);
  return {
    item_id: null,
    description: "",
    quantity: "1",
    price: "0.00",
    tax: first ? toChoice(first) : NO_TAX,
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

export function normaliseQuantity(value: string): string | null {
  const normalised = value.trim().replace(",", ".");
  if (!/^\d+(?:\.\d+)?$/.test(normalised)) return null;
  const numeric = Number(normalised);
  if (!Number.isFinite(numeric) || numeric <= 0 || numeric > 1_000_000_000) return null;
  return normalised;
}

/** Validate editable rows before an IPC call; the Rust boundary repeats these checks. */
export function validateEditLines(lines: EditLine[], items: Item[] = []): LineIssue[] {
  return lines.map((line) => {
    const quantity = normaliseQuantity(line.quantity);
    const price = parseMoney(line.price);
    const item =
      line.item_id === null ? undefined : items.find((candidate) => candidate.id === line.item_id);
    return {
      description: !line.description.trim()
        ? "Description is required"
        : line.description.trim().length > 2_000
          ? "Description cannot exceed 2,000 characters"
          : undefined,
      quantity:
        quantity === null
          ? "Enter a quantity greater than zero"
          : item?.tracked && !Number.isInteger(Number(quantity))
            ? "Tracked products need a whole quantity"
            : undefined,
      price:
        price === null || price < 0
          ? "Enter a valid non-negative price with no more than two decimal places"
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
