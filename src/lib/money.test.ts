import { describe, expect, it } from "vitest";

import {
  lineAmountMinor,
  lineTaxMinor,
  minorToInput,
  parseDecimal,
  parseMoney,
  parsePercentToBp,
  parseWholeNumber,
} from "@/lib/money";

describe("money input", () => {
  it("parses decimal points, decimal commas, and grouped amounts", () => {
    expect(parseMoney("12.50")).toBe(1_250);
    expect(parseMoney("12,50")).toBe(1_250);
    expect(parseMoney("1,234.56")).toBe(123_456);
    expect(parseMoney("1.234,56")).toBe(123_456);
    expect(parseMoney("1 234,56")).toBe(123_456);
    expect(parseMoney("1'234.56")).toBe(123_456);
    expect(parseMoney("1,000,000")).toBe(100_000_000);
  });

  it("accepts negative amounts for discount lines", () => {
    expect(parseMoney("-10")).toBe(-1_000);
    expect(parseMoney("-0")).toBe(0);
  });

  it("never reads extra decimal places as thousands", () => {
    // A leading zero can only be a decimal, and money allows two places.
    expect(parseMoney("0.125")).toBeNull();
    expect(parseMoney("0,125")).toBeNull();
    // Ambiguous three-digit groups follow the locale's decimal mark.
    expect(parseMoney("1.250", 2, "en-GB")).toBeNull();
    expect(parseMoney("1,250", 2, "en-GB")).toBe(125_000);
    expect(parseMoney("1.250", 2, "de-DE")).toBe(125_000);
    expect(parseMoney("1,250", 2, "de-DE")).toBeNull();
  });

  it("rejects malformed, over-precise, and unsafe values", () => {
    expect(parseMoney("$12.50")).toBeNull();
    expect(parseMoney("12.3456")).toBeNull();
    expect(parseMoney("1,23,4")).toBeNull();
    expect(parseMoney("12 dollars")).toBeNull();
    expect(parseMoney("1e3")).toBeNull();
    expect(parseMoney("0x10")).toBeNull();
    expect(parseMoney("999999999999999999")).toBeNull();
  });

  it("round-trips editable two-decimal values", () => {
    expect(minorToInput(parseMoney("123.40") ?? 0)).toBe("123.40");
    expect(minorToInput(-1_005)).toBe("-10.05");
    expect(minorToInput(7)).toBe("0.07");
  });
});

describe("other numeric input", () => {
  it("normalises quantities the same way prices are read", () => {
    expect(parseDecimal("1,5", 10)).toBe("1.5");
    expect(parseDecimal("1,000", 10, "en-GB")).toBe("1000");
    expect(parseDecimal("1.000", 10, "en-GB")).toBe("1");
    expect(parseDecimal("0.125", 10)).toBe("0.125");
    expect(parseDecimal(".5", 10)).toBe("0.5");
  });

  it("parses whole numbers strictly", () => {
    expect(parseWholeNumber("12", { min: 0, max: 100 })).toBe(12);
    expect(parseWholeNumber("-3", { min: -10, max: 10 })).toBe(-3);
    expect(parseWholeNumber("2.5", { min: 0, max: 100 })).toBeNull();
    expect(parseWholeNumber("abc", { min: 0, max: 100 })).toBeNull();
    expect(parseWholeNumber("0x10", { min: 0, max: 100 })).toBeNull();
    expect(parseWholeNumber("1e3", { min: 0, max: 10_000 })).toBeNull();
    expect(parseWholeNumber("101", { min: 0, max: 100 })).toBeNull();
  });

  it("parses percentages to basis points with two decimal places", () => {
    expect(parsePercentToBp("20")).toBe(2_000);
    expect(parsePercentToBp("12,5")).toBe(1_250);
    expect(parsePercentToBp("8.88")).toBe(888);
    expect(parsePercentToBp("8.875")).toBeNull();
    expect(parsePercentToBp(" ")).toBeNull();
    expect(parsePercentToBp("-5")).toBeNull();
    expect(parsePercentToBp("1001")).toBeNull();
  });
});

describe("document arithmetic", () => {
  it("rounds line amounts half away from zero, exactly like the backend", () => {
    expect(lineAmountMinor(45, "0.7")).toBe(32); // 31.5 → 32 (floating point gives 31)
    expect(lineAmountMinor(110, "1.15")).toBe(127); // 126.5 → 127
    expect(lineAmountMinor(99, "2.5")).toBe(248);
    expect(lineAmountMinor(-45, "0.7")).toBe(-32);
  });

  it("splits exclusive and inclusive tax per line", () => {
    expect(lineTaxMinor(10_000, 2_000, false)).toEqual({ net: 10_000, tax: 2_000, gross: 12_000 });
    expect(lineTaxMinor(11_500, 1_500, true)).toEqual({ net: 10_000, tax: 1_500, gross: 11_500 });
    expect(lineTaxMinor(100, 1_250, false).tax).toBe(13);
  });
});
