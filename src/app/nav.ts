import { createContext, useContext } from "react";

import type { SectionId } from "@/app/sections";

/** Imperative section navigation, provided by Layout (e.g. dashboard banner → Settings). */
export const NavContext = createContext<(section: SectionId) => void>(() => {});

export function useNav(): (section: SectionId) => void {
  return useContext(NavContext);
}
