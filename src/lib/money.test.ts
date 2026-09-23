import { describe, expect, it } from "vitest";

import { minorToInput, parseMoney } from "@/lib/money";

describe("money input", () => {
  it("parses decimal points, decimal commas, and grouped amounts", () => {
    expect(parseMoney("12.50")).toBe(1_250);
    expect(parseMoney("12,50")).toBe(1_250);
    expect(parseMoney("1,234.56")).toBe(123_456);
    expect(parseMoney("1.234,56")).toBe(123_456);
    expect(parseMoney("1 234,56")).toBe(123_456);
  });

  it("rejects malformed, over-precise, and unsafe values", () => {
    expect(parseMoney("$12.50")).toBeNull();
    expect(parseMoney("12.3456")).toBeNull();
    expect(parseMoney("1,23,4")).toBeNull();
    expect(parseMoney("12 dollars")).toBeNull();
    expect(parseMoney("999999999999999999")).toBeNull();
  });

  it("round-trips editable two-decimal values", () => {
    expect(minorToInput(parseMoney("123.40") ?? 0)).toBe("123.40");
  });
});
