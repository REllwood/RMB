import { useState, type ReactNode } from "react";
import { AlertCircle, Inbox, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";

export function Loading({ label = "Loading…" }: { label?: string }) {
  return (
    <div
      role="status"
      className="flex items-center justify-center gap-2 p-10 text-sm text-muted-foreground"
    >
      <Loader2 className="size-4 animate-spin" aria-hidden />
      <span>{label}</span>
    </div>
  );
}

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed p-10 text-center">
      <Inbox className="size-8 text-muted-foreground" aria-hidden />
      <p className="font-medium">{title}</p>
      {description && <p className="max-w-sm text-sm text-muted-foreground">{description}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

export function ErrorState({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry?: () => unknown | Promise<unknown>;
}) {
  const [retrying, setRetrying] = useState(false);
  const message = error instanceof Error ? error.message : String(error);

  async function retry() {
    if (!onRetry || retrying) return;
    setRetrying(true);
    try {
      await onRetry();
    } finally {
      setRetrying(false);
    }
  }

  return (
    <div
      role="alert"
      className="flex flex-col items-center gap-2 rounded-lg border border-destructive/40 bg-destructive/5 p-8 text-center"
    >
      <AlertCircle className="size-6 text-destructive" aria-hidden />
      <p className="text-sm">{message}</p>
      {onRetry && (
        <Button
          variant="outline"
          size="sm"
          onClick={retry}
          loading={retrying}
          loadingLabel="Trying again…"
        >
          Try again
        </Button>
      )}
    </div>
  );
}
