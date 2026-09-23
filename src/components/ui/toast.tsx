import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { CheckCircle2, X, XCircle } from "lucide-react";

import { useTopDialog } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

type ToastKind = "success" | "error";
type Toast = { id: number; kind: ToastKind; message: string };
type PushToast = (kind: ToastKind, message: string) => void;

const ToastContext = createContext<PushToast | null>(null);

/** How long a success message stays; errors stay until dismissed so they can always be read. */
const SUCCESS_MS = 5000;

/**
 * App-level toast region. Two persistent live regions (polite for confirmations, assertive for
 * errors) so screen readers announce every message. While a modal dialog is open the region is
 * rendered inside it: the dialog sits in the browser's top layer and makes the rest of the page
 * inert, so a toast anywhere else would be hidden behind its backdrop and unreachable.
 */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);
  const topDialog = useTopDialog();

  const dismiss = useCallback((id: number) => {
    setToasts((ts) => ts.filter((t) => t.id !== id));
  }, []);

  const push = useCallback<PushToast>((kind, message) => {
    const id = nextId.current++;
    setToasts((ts) => [
      ...ts.filter((t) => t.message !== message).slice(-3),
      { id, kind, message },
    ]);
  }, []);

  const region = (
    <div
      role="region"
      aria-label="Notifications"
      className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2"
    >
      <div aria-live="assertive" className="flex flex-col gap-2">
        {toasts
          .filter((t) => t.kind === "error")
          .map((t) => (
            <ToastCard key={t.id} toast={t} onDismiss={dismiss} />
          ))}
      </div>
      <div aria-live="polite" className="flex flex-col gap-2">
        {toasts
          .filter((t) => t.kind === "success")
          .map((t) => (
            <ToastCard key={t.id} toast={t} onDismiss={dismiss} />
          ))}
      </div>
    </div>
  );

  return (
    <ToastContext.Provider value={push}>
      {children}
      {topDialog ? createPortal(region, topDialog) : region}
    </ToastContext.Provider>
  );
}

function ToastCard({ toast, onDismiss }: { toast: Toast; onDismiss: (id: number) => void }) {
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    if (toast.kind === "error" || paused) return;
    const timer = window.setTimeout(() => onDismiss(toast.id), SUCCESS_MS);
    return () => window.clearTimeout(timer);
  }, [toast, paused, onDismiss]);

  return (
    <div
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
      className={cn(
        "pointer-events-auto flex items-start gap-2.5 rounded-lg border bg-card px-4 py-3 text-sm text-card-foreground shadow-lg",
        toast.kind === "error" && "border-destructive/60",
      )}
    >
      {toast.kind === "error" ? (
        <XCircle className="mt-0.5 size-4 shrink-0 text-destructive" aria-hidden />
      ) : (
        <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-success" aria-hidden />
      )}
      <span className="min-w-0 flex-1 break-words">
        {toast.kind === "error" && <span className="sr-only">Error: </span>}
        {toast.message}
      </span>
      <button
        type="button"
        onClick={() => onDismiss(toast.id)}
        className="-m-1 rounded p-1 text-muted-foreground hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
        aria-label="Dismiss notification"
      >
        <X className="size-3.5" aria-hidden />
      </button>
    </div>
  );
}

/** Push a toast. Without a provider (tests), falls back to console so nothing crashes. */
export function useToast(): PushToast {
  const ctx = useContext(ToastContext);
  return (
    ctx ??
    ((kind, message) => {
      if (kind === "error") console.error(message);
      else console.log(message);
    })
  );
}
