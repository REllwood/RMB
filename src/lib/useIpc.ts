import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useToast } from "@/components/ui/toast";

/** Query wrapper around an IPC call. */
export function useIpcQuery<T>(key: unknown[], fn: () => Promise<T>, enabled = true) {
  return useQuery({ queryKey: key, queryFn: fn, enabled });
}

/** Mutation wrapper: surfaces backend errors as toasts and invalidates keys on success. */
export function useIpcMutation<TArgs, TResult>(
  fn: (args: TArgs) => Promise<TResult>,
  invalidateKeys: unknown[][] = [],
  opts?: { successMessage?: string },
) {
  const qc = useQueryClient();
  const toast = useToast();
  return useMutation({
    mutationFn: fn,
    onError: (err) => {
      // Backend errors (e.g. "tracked product needs a whole quantity") must reach the user.
      console.error("Action failed:", err);
      toast("error", err instanceof Error ? err.message : String(err));
    },
    onSuccess: () => {
      if (opts?.successMessage) toast("success", opts.successMessage);
      invalidateKeys.forEach((key) => qc.invalidateQueries({ queryKey: key }));
    },
  });
}
