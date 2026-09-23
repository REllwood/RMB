import { createContext, useCallback, useContext, useState } from "react";

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
