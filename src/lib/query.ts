import { QueryClient } from "@tanstack/react-query";

/** Shared query client. Local SQLite is fast and offline, so no retries/refetch-on-focus. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false, staleTime: 5_000 },
  },
});
