import { ipc } from "@/lib/ipc";
import { formatMoney } from "@/lib/format";
import { useIpcQuery } from "@/lib/useIpc";

/** The decimal and grouping separators for `locale` (the system locale when omitted). */
export function localeSeparators(locale?: string): { decimal: string; group: string } {
  const parts = new Intl.NumberFormat(locale).formatToParts(12345.6);
  return {
    decimal: parts.find((p) => p.type === "decimal")?.value ?? ".",
    group: parts.find((p) => p.type === "group")?.value ?? ",",
  };
}

/** Undo digit grouping ("1,234,567" → "1234567"); null unless the groups are well formed. */
function ungroup(value: string, separator: string): string | null {
  const groups = value.split(separator);
  const [first, ...rest] = groups;
  if (!/^[1-9]\d{0,2}$/.test(first) || rest.some((g) => !/^\d{3}$/.test(g))) return null;
  return groups.join("");
}

/**
 * Parse a typed decimal number into a normalised string ("-1234.5"), or null if it isn't a clean
 * number with at most `maxDecimals` decimal places. Accepts either "." or "," as the decimal mark
 * and either as a thousands separator, plus spaces and apostrophes as grouping. A single separator
 * followed by exactly three digits ("1,250" / "1.250") is genuinely ambiguous, so it is read the way
 * the user's locale writes numbers — never guessed as thousands when it could be decimals.
 */
export function parseDecimal(input: string, maxDecimals: number, locale?: string): string | null {
  let source = input.trim().replace(/[\s\u00a0\u202f']/g, "");
  let sign = "";
  if (source.startsWith("-") || source.startsWith("+")) {
    sign = source[0] === "-" ? "-" : "";
    source = source.slice(1);
  }
  if (!/^[\d.,]+$/.test(source) || !/\d/.test(source)) return null;

  const commas = source.split(",").length - 1;
  const dots = source.split(".").length - 1;
  let integerPart = source;
  let fraction = "";

  if (commas > 0 && dots > 0) {
    const decimal = source.lastIndexOf(",") > source.lastIndexOf(".") ? "," : ".";
    const group = decimal === "," ? "." : ",";
    const pieces = source.split(decimal);
    if (pieces.length !== 2) return null;
    const ungrouped = ungroup(pieces[0], group);
    if (ungrouped === null) return null;
    integerPart = ungrouped;
    fraction = pieces[1];
  } else if (commas + dots > 1) {
    // Several of the same separator can only be grouping ("1,000,000").
    const ungrouped = ungroup(source, commas > 0 ? "," : ".");
    if (ungrouped === null) return null;
    integerPart = ungrouped;
  } else if (commas + dots === 1) {
    const separator = commas > 0 ? "," : ".";
    const [before, after] = source.split(separator);
    const couldBeGrouping = after.length === 3 && /^[1-9]\d{0,2}$/.test(before);
    if (couldBeGrouping && separator !== localeSeparators(locale).decimal) {
      integerPart = before + after;
    } else {
      integerPart = before || "0";
      fraction = after;
    }
  }

  if (!/^\d+$/.test(integerPart) || !/^\d*$/.test(fraction)) return null;
  if (fraction.length > maxDecimals) return null;
  integerPart = integerPart.replace(/^0+(?=\d)/, "");
  fraction = fraction.replace(/0+$/, "");
  const magnitude = fraction ? `${integerPart}.${fraction}` : integerPart;
  return magnitude === "0" ? "0" : sign + magnitude;
}

/** A normalised decimal string as an exact scaled integer: "12.5" → { units: 125n, scale: 1 }. */
function scaled(decimal: string): { units: bigint; scale: number } {
  const [integer, fraction = ""] = decimal.split(".");
  return { units: BigInt(integer + fraction), scale: fraction.length };
}

/** Parse a user-entered major-unit amount (e.g. "12.50", "-10") into integer minor units. */
export function parseMoney(input: string, scale = 2, locale?: string): number | null {
  if (!Number.isInteger(scale) || scale < 0 || scale > 6) return null;
  const decimal = parseDecimal(input, scale, locale);
  if (decimal === null) return null;
  const { units, scale: places } = scaled(decimal);
  const minor = units * 10n ** BigInt(scale - places);
  if (minor > BigInt(Number.MAX_SAFE_INTEGER) || minor < BigInt(Number.MIN_SAFE_INTEGER)) {
    return null;
  }
  return Number(minor);
}

/** Parse a whole number within `[min, max]` (grouping allowed, decimals not). */
export function parseWholeNumber(
  input: string,
  { min, max }: { min: number; max: number },
  locale?: string,
): number | null {
  const decimal = parseDecimal(input, 0, locale);
  if (decimal === null) return null;
  const value = Number(decimal);
  return Number.isSafeInteger(value) && value >= min && value <= max ? value : null;
}

/** Parse a percentage ("20", "12.5", "12,5") into basis points (2000, 1250), 0–1,000%. */
export function parsePercentToBp(input: string, locale?: string): number | null {
  const decimal = parseDecimal(input, 2, locale);
  if (decimal === null || decimal.startsWith("-")) return null;
  const { units, scale } = scaled(decimal);
  const bp = Number(units * 10n ** BigInt(2 - scale));
  return bp <= 100_000 ? bp : null;
}

/** Render integer minor units as a plain major-unit string for an editable input (e.g. "12.50"). */
export function minorToInput(minor: number, scale = 2): string {
  const sign = minor < 0 ? "-" : "";
  const digits = String(Math.abs(Math.trunc(minor))).padStart(scale + 1, "0");
  return scale === 0
    ? `${sign}${digits}`
    : `${sign}${digits.slice(0, -scale)}.${digits.slice(-scale)}`;
}

/** `numerator / denominator` rounded half away from zero — the backend's rounding rule. */
function divideRounded(numerator: bigint, denominator: bigint): bigint {
  const negative = numerator < 0n !== denominator < 0n;
  const n = numerator < 0n ? -numerator : numerator;
  const d = denominator < 0n ? -denominator : denominator;
  const rounded = 2n * (n % d) >= d ? n / d + 1n : n / d;
  return negative ? -rounded : rounded;
}

/** A line's amount: unit price × quantity, rounded to the minor unit exactly as the backend does. */
export function lineAmountMinor(unitPriceMinor: number, quantity: string): number {
  const { units, scale } = scaled(quantity);
  return Number(divideRounded(BigInt(unitPriceMinor) * units, 10n ** BigInt(scale)));
}

/** Labour for a time entry: hourly rate × minutes ÷ 60, rounded like the backend (from exact minutes). */
export function labourAmountMinor(rateMinor: number, minutes: number): number {
  return Number(divideRounded(BigInt(rateMinor) * BigInt(minutes), 60n));
}

/** Net / tax / gross for one line amount — the same per-line rule the backend stores. */
export function lineTaxMinor(
  amountMinor: number,
  rateBp: number,
  inclusive: boolean,
): { net: number; tax: number; gross: number } {
  const amount = BigInt(amountMinor);
  const bp = BigInt(rateBp);
  if (inclusive) {
    const net = divideRounded(amount * 10_000n, 10_000n + bp);
    return { net: Number(net), tax: Number(amount - net), gross: amountMinor };
  }
  const tax = divideRounded(amount * bp, 10_000n);
  return { net: amountMinor, tax: Number(tax), gross: Number(amount + tax) };
}

/** Hook returning a formatter bound to the configured currency. */
export function useMoneyFormat() {
  const { data: settings } = useIpcQuery(["settings"], () => ipc.getSettings());
  const currency = settings?.currency ?? "USD";
  return (minor: number) => formatMoney(minor, currency);
}
