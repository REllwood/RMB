import { describe, expect, it } from "vitest";

import {
  defaultTax,
  isWholeQuantity,
  NO_TAX,
  normaliseQuantity,
  previewTotals,
  taxLabel,
  taxOptions,
  toLineInputs,
  validateEditLines,
  type EditLine,
} from "@/features/shared/lines";
import type { Item, TaxRate } from "@/lib/types";

const line = (overrides: Partial<EditLine> = {}): EditLine => ({
  item_id: null,
  description: "Labour",
  quantity: "1",
  price: "120.00",
  tax: NO_TAX,
  ...overrides,
});

const trackedItem = {
  id: 7,
  name: "Filter",
  kind: "product",
  tracked: true,
} as Item;

describe("document line validation", () => {
  it("normalises decimal commas and rejects non-positive or excessive quantities", () => {
    expect(normaliseQuantity(" 1,5 ")).toBe("1.5");
    expect(normaliseQuantity("0")).toBeNull();
    expect(normaliseQuantity("1000000001")).toBeNull();
    expect(normaliseQuantity("one")).toBeNull();
  });

  it("reports every invalid field and rejects fractional tracked stock", () => {
    const issues = validateEditLines(
      [line({ item_id: 7, description: " ", quantity: "1.5", price: "12.3456" })],
      [trackedItem],
    );

    expect(issues[0]).toEqual({
      description: "Description is required",
      quantity: "Tracked products need a whole quantity",
      price:
        "Enter a valid price with no more than two decimal places (use a minus sign for a discount)",
    });
  });

  it("accepts negative prices as discount lines", () => {
    expect(validateEditLines([line({ description: "Loyalty discount", price: "-10.00" })])).toEqual(
      [{ description: undefined, quantity: undefined, price: undefined }],
    );
  });

  it("trims validated payload text and preserves exact minor units", () => {
    expect(
      toLineInputs([line({ description: "  Labour  ", quantity: "2,5", price: "19.95" })]),
    ).toEqual([
      {
        item_id: null,
        description: "Labour",
        quantity: "2.5",
        unit_price_minor: 1995,
        tax_rate_name: "No Tax",
        tax_rate_bp: 0,
        tax_inclusive: false,
      },
    ]);
  });
});

const ukRates: TaxRate[] = [
  { id: 1, name: "No Tax", rate_bp: 0, inclusive: false, archived: false },
  { id: 2, name: "VAT 0%", rate_bp: 0, inclusive: false, archived: false },
  { id: 3, name: "VAT 20%", rate_bp: 2000, inclusive: false, archived: false },
  { id: 4, name: "VAT 5%", rate_bp: 500, inclusive: false, archived: false },
];

describe("tax choices", () => {
  it("defaults to the chosen rate, else the first non-zero rate, else No Tax", () => {
    expect(defaultTax(ukRates, 4).name).toBe("VAT 5%");
    expect(defaultTax(ukRates, null).name).toBe("VAT 20%");
    expect(defaultTax([ukRates[0]], null)).toEqual(NO_TAX);
  });

  it("lists No Tax once and keeps a retired rate that a line still uses", () => {
    const names = taxOptions(ukRates).map((o) => o.name);
    expect(names.filter((n) => n === "No Tax")).toHaveLength(1);
    const retired = { name: "VAT 17.5%", bp: 1750, inclusive: false };
    expect(taxOptions(ukRates, retired)[0]).toEqual(retired);
  });

  it("labels rates with their percentage", () => {
    expect(taxLabel({ name: "VAT 20%", bp: 2000, inclusive: false })).toBe("VAT 20%");
    expect(taxLabel({ name: "GST", bp: 1000, inclusive: true })).toBe("GST (10%, incl.)");
  });
});

describe("document previews", () => {
  it("normalises quantities and knows whole units", () => {
    expect(normaliseQuantity("1.000")).toBe("1");
    expect(isWholeQuantity("1")).toBe(true);
    expect(isWholeQuantity("1.0000000000000001")).toBe(false);
    expect(normaliseQuantity("1.0000000000000001")).toBeNull();
  });

  it("previews subtotal, tax and total with inclusive and exclusive lines", () => {
    const totals = previewTotals([
      line({ price: "100.00", tax: { name: "VAT 20%", bp: 2000, inclusive: false } }),
      line({ price: "120.00", tax: { name: "VAT 20%", bp: 2000, inclusive: true } }),
    ]);
    expect(totals).toEqual({ subtotal: 20_000, tax: 4_000, total: 24_000 });
  });
});
