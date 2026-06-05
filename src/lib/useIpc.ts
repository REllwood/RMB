import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

/** Query wrapper around an IPC call. */
export function useIpcQuery<T>(key: unknown[], fn: () => Promise<T>, enabled = true) {
  return useQuery({ queryKey: key, queryFn: fn, enabled });
}

/** Mutation wrapper that invalidates the given query keys on success. */
export function useIpcMutation<TArgs, TResult>(
  fn: (args: TArgs) => Promise<TResult>,
  invalidateKeys: unknown[][] = [],
) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onError: (err) => {
      // Surface backend errors (e.g. "tracked product needs a whole quantity") instead of
      // failing silently. Replace with a toast when one is added.
      console.error("Action failed:", err);
      if (typeof window !== "undefined" && typeof window.alert === "function") {
        window.alert(err instanceof Error ? err.message : String(err));
      }
    },
    onSuccess: () => invalidateKeys.forEach((key) => qc.invalidateQueries({ queryKey: key })),
  });
}
