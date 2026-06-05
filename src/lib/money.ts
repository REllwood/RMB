import { ipc } from "@/lib/ipc";
import { formatMoney } from "@/lib/format";
import { useIpcQuery } from "@/lib/useIpc";

/** Parse a user-entered major-unit amount (e.g. "12.50") into integer minor units. */
export function parseMoney(input: string, scale = 2): number | null {
  let s = input.trim().replace(/\s/g, "");
  if (s.includes(",") && s.includes(".")) {
    // Both separators present → treat "," as the thousands separator.
    s = s.replace(/,/g, "");
  } else if (s.includes(",")) {
    const oneComma = s.indexOf(",") === s.lastIndexOf(",");
    const decimals = s.slice(s.lastIndexOf(",") + 1).length;
    // A single comma followed by 1–2 digits is a decimal comma ("1,50"); otherwise thousands.
    s = oneComma && decimals <= 2 ? s.replace(",", ".") : s.replace(/,/g, "");
  }
  s = s.replace(/[^0-9.-]/g, "");
  if (s === "" || s === "-" || s === ".") return null;
  const value = Number(s);
  if (!Number.isFinite(value)) return null;
  return Math.round(value * 10 ** scale);
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
