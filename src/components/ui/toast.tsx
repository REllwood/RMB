import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from "react";
import { CheckCircle2, XCircle } from "lucide-react";

import { cn } from "@/lib/utils";

type ToastKind = "success" | "error";
type Toast = { id: number; kind: ToastKind; message: string };
type PushToast = (kind: ToastKind, message: string) => void;

const ToastContext = createContext<PushToast | null>(null);

/** App-level toast region. Errors persist longer; everything announces via live regions. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const push = useCallback<PushToast>((kind, message) => {
    const id = nextId.current++;
    setToasts((ts) => [...ts.slice(-3), { id, kind, message }]);
    window.setTimeout(
      () => setToasts((ts) => ts.filter((t) => t.id !== id)),
      kind === "error" ? 8000 : 4000,
    );
  }, []);

  return (
    <ToastContext.Provider value={push}>
      {children}
      <div
        role="region"
        aria-label="Notifications"
        className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2"
      >
        {toasts.map((t) => (
          <div
            key={t.id}
            role={t.kind === "error" ? "alert" : "status"}
            className={cn(
              "pointer-events-auto flex items-start gap-2.5 rounded-lg border bg-card px-4 py-3 text-sm text-card-foreground shadow-lg",
              t.kind === "error" && "border-destructive/40",
            )}
          >
            {t.kind === "error" ? (
              <XCircle className="mt-0.5 size-4 shrink-0 text-destructive" aria-hidden />
            ) : (
              <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-success" aria-hidden />
            )}
            <span className="min-w-0 break-words">{t.message}</span>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
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
