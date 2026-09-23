import { ipc } from "@/lib/ipc";
import { formatMoney } from "@/lib/format";
import { useIpcQuery } from "@/lib/useIpc";

/** Parse a user-entered major-unit amount (e.g. "12.50") into integer minor units. */
export function parseMoney(input: string, scale = 2): number | null {
  if (!Number.isInteger(scale) || scale < 0 || scale > 6) return null;
  let source = input.trim().replace(/\s/g, "");
  if (!/^[+-]?(?:\d+(?:[.,]\d*)?|[.,]\d+)(?:[.,]\d+)*$/.test(source)) return null;

  let sign = "";
  if (source.startsWith("-") || source.startsWith("+")) {
    sign = source[0] === "-" ? "-" : "";
    source = source.slice(1);
  }

  const commaCount = (source.match(/,/g) ?? []).length;
  const dotCount = (source.match(/\./g) ?? []).length;
  let integerPart = source;
  let fractionalPart = "";

  if (commaCount > 0 && dotCount > 0) {
    const decimal = source.lastIndexOf(",") > source.lastIndexOf(".") ? "," : ".";
    const thousands = decimal === "," ? "." : ",";
    if (source.split(decimal).length !== 2) return null;
    const [groupedInteger, fraction] = source.split(decimal);
    const groups = groupedInteger.split(thousands);
    if (
      groups.some((group) => !/^\d+$/.test(group)) ||
      (groups.length > 1 &&
        (groups[0].length < 1 ||
          groups[0].length > 3 ||
          groups.slice(1).some((g) => g.length !== 3)))
    ) {
      return null;
    }
    integerPart = groups.join("");
    fractionalPart = fraction;
  } else {
    const separator = commaCount > 0 ? "," : dotCount > 0 ? "." : null;
    if (separator) {
      const parts = source.split(separator);
      if (parts.length === 2 && parts[1].length <= scale) {
        integerPart = parts[0] || "0";
        fractionalPart = parts[1];
      } else if (
        parts.length > 1 &&
        parts[0].length >= 1 &&
        parts[0].length <= 3 &&
        parts.every((part, index) => index === 0 || part.length === 3)
      ) {
        integerPart = parts.join("");
      } else {
        return null;
      }
    }
  }

  if (!/^\d+$/.test(integerPart) || !/^\d*$/.test(fractionalPart)) return null;
  if (fractionalPart.length > scale) return null;
  const paddedFraction = fractionalPart.padEnd(scale, "0");
  const magnitude = BigInt(integerPart) * 10n ** BigInt(scale) + BigInt(paddedFraction || "0");
  const signed = sign === "-" ? -magnitude : magnitude;
  if (signed > BigInt(Number.MAX_SAFE_INTEGER) || signed < BigInt(Number.MIN_SAFE_INTEGER)) {
    return null;
  }
  return Number(signed);
}

/** Render integer minor units as a plain major-unit string for an editable input (e.g. "12.50"). */
export function minorToInput(minor: number, scale = 2): string {
  return (minor / 10 ** scale).toFixed(scale);
}

/** Hook returning a formatter bound to the configured currency. */
export function useMoneyFormat() {
  const { data: settings } = useIpcQuery(["settings"], () => ipc.getSettings());
  const currency = settings?.currency ?? "USD";
  return (minor: number) => formatMoney(minor, currency);
}
