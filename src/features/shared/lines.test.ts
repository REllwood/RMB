import { describe, expect, it } from "vitest";

import {
  NO_TAX,
  normaliseQuantity,
  toLineInputs,
  validateEditLines,
  type EditLine,
} from "@/features/shared/lines";
import type { Item } from "@/lib/types";

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
