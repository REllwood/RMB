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
  return new Intl.NumberFormat(options?.locale, {
    style: "currency",
    currency,
  }).format(major);
}
