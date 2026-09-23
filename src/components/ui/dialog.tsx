import {
  useEffect,
  useId,
  useReducer,
  useRef,
  useSyncExternalStore,
  type FormEvent,
  type ReactNode,
} from "react";
import { X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/*
 * Open modal dialogs, topmost last. A modal <dialog> sits in the browser's top layer and makes the
 * rest of the page inert, so anything that must stay visible and reachable while one is open (the
 * toast region) renders inside the topmost dialog.
 */
let openDialogs: HTMLDialogElement[] = [];
const listeners = new Set<() => void>();
function setOpenDialogs(next: HTMLDialogElement[]) {
  openDialogs = next;
  listeners.forEach((listener) => listener());
}

/** The topmost open modal dialog, or null. */
export function useTopDialog(): HTMLDialogElement | null {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => openDialogs[openDialogs.length - 1] ?? null,
    () => null,
  );
}

/** Where focus goes when a dialog opens: an explicit target, the first field, else the footer. */
function focusInitial(dialog: HTMLDialogElement) {
  const target =
    dialog.querySelector<HTMLElement>("[data-autofocus]") ??
    dialog.querySelector<HTMLElement>(
      "[data-dialog-body] input:not([disabled]), [data-dialog-body] select:not([disabled]), [data-dialog-body] textarea:not([disabled])",
    ) ??
    dialog.querySelector<HTMLElement>("[data-dialog-footer] button:not([disabled])");
  target?.focus();
}

/**
 * Modal dialog on the native `<dialog>` element — focus containment, Escape, and inert background
 * come from the platform. Controlled: pass `open` + `onClose`. Focus moves to the first field (or
 * the first footer button) on open and returns to where it was on close, even if the dialog is
 * unmounted while open.
 */
export function Dialog({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  className,
  onSubmit,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children?: ReactNode;
  footer?: ReactNode;
  className?: string;
  /** Makes the dialog a form: Enter in a field (or a `type="submit"` footer button) calls this. */
  onSubmit?: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const descriptionId = useId();
  const returnFocus = useRef<HTMLElement | null>(null);
  const pressStartedOnBackdrop = useRef(false);
  // Re-render after the platform closes the dialog on its own, so the effect below can reopen it
  // if the owner still wants it open (e.g. while an action is pending).
  const [, resync] = useReducer((n: number) => n + 1, 0);

  useEffect(() => {
    const el = ref.current;
    // jsdom (tests) lacks showModal — guard so closed dialogs render harmlessly.
    if (!el || typeof el.showModal !== "function") return;
    if (open && !el.open) {
      if (!returnFocus.current && document.activeElement instanceof HTMLElement) {
        returnFocus.current = document.activeElement;
      }
      el.showModal();
      setOpenDialogs([...openDialogs.filter((d) => d !== el), el]);
      focusInitial(el);
    }
    if (!open && el.open) el.close();
  });

  useEffect(() => {
    const el = ref.current;
    return () => {
      // Unmounted while open: the platform can't restore focus, so do it here.
      if (el) setOpenDialogs(openDialogs.filter((d) => d !== el));
      if (returnFocus.current?.isConnected) returnFocus.current.focus();
    };
  }, []);

  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      onCancel={(e) => {
        // Esc: keep React state authoritative instead of letting the dialog self-close.
        e.preventDefault();
        onClose();
      }}
      onClose={() => {
        const el = ref.current;
        if (el) setOpenDialogs(openDialogs.filter((d) => d !== el));
        if (open) {
          // Closed by the platform (e.g. a repeated Escape); let the owner decide.
          onClose();
          resync();
        } else if (returnFocus.current?.isConnected) {
          returnFocus.current.focus();
        }
        returnFocus.current = null;
      }}
      onMouseDown={(e) => {
        pressStartedOnBackdrop.current = e.target === ref.current;
      }}
      onClick={(e) => {
        // Backdrop click — only when the press also started there (not a text-selection drag).
        if (e.target === ref.current && pressStartedOnBackdrop.current) onClose();
        pressStartedOnBackdrop.current = false;
      }}
      className={cn(
        "m-auto w-full max-w-md rounded-xl border bg-card p-0 text-card-foreground shadow-2xl",
        "backdrop:bg-black/50 backdrop:backdrop-blur-[2px]",
        className,
      )}
    >
      {open && (
        <Frame
          className="flex max-h-[85vh] flex-col"
          onSubmit={
            onSubmit
              ? (e) => {
                  e.preventDefault();
                  onSubmit();
                }
              : undefined
          }
        >
          <div className="flex items-start justify-between gap-4 border-b px-5 py-4">
            <div className="space-y-0.5">
              <h2 id={titleId} className="text-base font-semibold tracking-tight">
                {title}
              </h2>
              {description && (
                <p id={descriptionId} className="text-sm text-muted-foreground">
                  {description}
                </p>
              )}
            </div>
            <Button variant="ghost" size="icon" onClick={onClose} aria-label="Close dialog">
              <X className="size-4" />
            </Button>
          </div>
          {children && (
            <div data-dialog-body className="overflow-y-auto px-5 py-4">
              {children}
            </div>
          )}
          {footer && (
            <div
              data-dialog-footer
              className="flex flex-wrap justify-end gap-2 border-t bg-muted/30 px-5 py-3"
            >
              {footer}
            </div>
          )}
        </Frame>
      )}
    </dialog>
  );
}

/** The dialog's content box: a form when the dialog submits, otherwise a plain container. */
function Frame({
  className,
  children,
  onSubmit,
}: {
  className: string;
  children: ReactNode;
  onSubmit?: (e: FormEvent<HTMLFormElement>) => void;
}) {
  return onSubmit ? (
    // The app validates its own fields and explains problems inline.
    <form className={className} onSubmit={onSubmit} noValidate>
      {children}
    </form>
  ) : (
    <div className={className}>{children}</div>
  );
}

/** Confirmation dialog for destructive / irreversible actions. Focus starts on Cancel. */
export function ConfirmDialog({
  open,
  onClose,
  onConfirm,
  title,
  description,
  confirmLabel = "Confirm",
  destructive = false,
  pending = false,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  description?: string;
  confirmLabel?: string;
  destructive?: boolean;
  pending?: boolean;
}) {
  const close = () => {
    if (!pending) onClose();
  };
  return (
    <Dialog
      open={open}
      onClose={close}
      title={title}
      description={description}
      footer={
        <>
          <Button variant="outline" onClick={close} disabled={pending}>
            Cancel
          </Button>
          <Button
            variant={destructive ? "destructive" : "default"}
            onClick={onConfirm}
            loading={pending}
            loadingLabel="Working…"
          >
            {confirmLabel}
          </Button>
        </>
      }
    />
  );
}
