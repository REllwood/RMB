/**
 * Today as a YYYY-MM-DD string in the user's **local** timezone. `toISOString()` is UTC and
 * shows yesterday's date for morning entries east of Greenwich — never use it for business dates.
 */
export function todayLocalISO(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/**
 * Format an integer minor-unit amount for display. The Rust backend is the source of truth
 * for money math; this only formats — never do arithmetic on money here. `scale` is the
 * currency's minor-unit exponent (2 for USD/GBP/EUR/AUD/NZD/CAD).
 */
export function formatMoney(
  minor: number,
  currency: string,
  options?: { locale?: string; scale?: number },
): string {
  const scale = options?.scale ?? 2;
  const major = minor / 10 ** scale;
  try {
    return new Intl.NumberFormat(options?.locale, {
      style: "currency",
      currency,
    }).format(major);
  } catch {
    // An invalid/unknown ISO-4217 code makes Intl throw a RangeError. Degrade to a plain number
    // with the code suffixed instead of blanking every money figure on the screen.
    return `${new Intl.NumberFormat(options?.locale, {
      minimumFractionDigits: scale,
      maximumFractionDigits: scale,
    }).format(major)} ${currency}`;
  }
}

/** How a document number will look, e.g. ("INV-", 7, 4) → "INV-0007" (matches the backend). */
export function formatNumberPreview(prefix: string, sequence: number, pad: number): string {
  return `${prefix.trim()}${String(sequence).padStart(pad, "0")}`;
}
