import { QueryClient } from "@tanstack/react-query";

/** Shared query client. Local SQLite is fast and offline, so no retries/refetch-on-focus. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false, staleTime: 5_000 },
  },
});

/**
 * Everything a change to an invoice, quote, job or payment can affect. Documents link to each other
 * (a quote converts to a job, a job bills an invoice, voiding releases the source) and feed the
 * dashboard, customer history and reports, so document mutations refresh all of them. Only queries
 * on screen refetch; the rest are just marked stale.
 */
export const DOCUMENT_KEYS: unknown[][] = [
  ["invoices"],
  ["invoice"],
  ["payments"],
  ["quotes"],
  ["quote"],
  ["jobs"],
  ["job"],
  ["recurring"],
  ["items"],
  ["dashboard"],
  ["customer-history"],
  ["report-tax"],
  ["report-month"],
  ["report-cust"],
];
