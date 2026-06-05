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
    onSuccess: () => invalidateKeys.forEach((key) => qc.invalidateQueries({ queryKey: key })),
  });
}
