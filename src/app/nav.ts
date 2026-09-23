import { createContext, useCallback, useContext, useState } from "react";

import type { SectionId } from "@/app/sections";

/** Imperative section navigation, provided by Layout (e.g. dashboard banner → Settings). */
export const NavContext = createContext<(section: SectionId) => void>(() => {});

export function useNav(): (section: SectionId) => void {
  return useContext(NavContext);
}

/** Moves keyboard focus to the start of the main content (provided by Layout). */
export const ViewFocusContext = createContext<() => void>(() => {});

/**
 * State for a page's current view (list / detail / form). Switching views unmounts whatever had
 * focus, so each switch also moves focus to the main content instead of dropping it on the page.
 */
export function useView<T>(initial: T): [T, (next: T) => void] {
  const [view, setView] = useState<T>(initial);
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
