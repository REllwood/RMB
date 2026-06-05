import { ipc } from "@/lib/ipc";
import { formatMoney } from "@/lib/format";
import { useIpcQuery } from "@/lib/useIpc";

/** Parse a user-entered major-unit amount (e.g. "12.50") into integer minor units. */
export function parseMoney(input: string, scale = 2): number | null {
  const cleaned = input.replace(/[^0-9.-]/g, "").trim();
  if (cleaned === "" || cleaned === "-" || cleaned === ".") return null;
  const value = Number(cleaned);
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
