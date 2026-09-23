import { createContext, useCallback, useContext, useEffect, useState } from "react";

import type { SectionId } from "@/app/sections";

/** Go to a section, optionally opening one record there (e.g. the invoice a quote became). */
export type GoTo = (section: SectionId, recordId?: number) => void;

/** Imperative section navigation, provided by Layout. */
export const NavContext = createContext<GoTo>(() => {});

export function useNav(): GoTo {
  return useContext(NavContext);
}

/** The record the current section was opened on, if navigation asked for one. */
export const NavTargetContext = createContext<number | null>(null);

/** Moves keyboard focus to the start of the main content (provided by Layout). */
export const ViewFocusContext = createContext<() => void>(() => {});

/**
 * State for a page's current view (list / detail / form). Switching views unmounts whatever had
 * focus, so each switch also moves focus to the main content instead of dropping it on the page.
 * `initial` receives the record navigation asked this section to open, if any.
 */
export function useView<T>(initial: (recordId: number | null) => T): [T, (next: T) => void] {
  const target = useContext(NavTargetContext);
  const [view, setView] = useState<T>(() => initial(target));
  const focusView = useContext(ViewFocusContext);
  const show = useCallback(
    (next: T) => {
      setView(next);
      focusView();
    },
    [focusView],
  );
  return [view, show];
}

/** Records whether a form has unsaved changes (provided by Layout). */
export const UnsavedChangesContext = createContext<(form: symbol, dirty: boolean) => void>(
  () => {},
);

/** While `dirty`, leaving the section asks before the form's changes are discarded. */
export function useUnsavedChanges(dirty: boolean) {
  const report = useContext(UnsavedChangesContext);
  const [form] = useState(() => Symbol("form"));
  useEffect(() => {
    report(form, dirty);
    return () => report(form, false);
  }, [report, form, dirty]);
}

/** `useUnsavedChanges` for a form whose whole state is `value`: dirty once it has changed. */
export function useUnsavedEdits(value: unknown) {
  const [start] = useState(() => JSON.stringify(value));
  useUnsavedChanges(JSON.stringify(value) !== start);
}
