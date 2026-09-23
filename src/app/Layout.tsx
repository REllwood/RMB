import { useCallback, useEffect, useRef, useState } from "react";
import { Moon, Store, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { setTheme, type Theme } from "@/lib/theme";
import { NavContext, NavTargetContext, UnsavedChangesContext, ViewFocusContext } from "@/app/nav";
import { SECTIONS, type SectionId } from "@/app/sections";

import { DashboardPage } from "@/features/dashboard/DashboardPage";
import { CustomersPage } from "@/features/customers/CustomersPage";
import { CatalogPage } from "@/features/catalog/CatalogPage";
import { QuotesPage } from "@/features/quotes/QuotesPage";
import { JobsPage } from "@/features/jobs/JobsPage";
import { InvoicesPage } from "@/features/invoices/InvoicesPage";
import { ReportsPage } from "@/features/reports/ReportsPage";
import { SettingsPage } from "@/features/settings/SettingsPage";

function renderSection(id: SectionId) {
  switch (id) {
    case "dashboard":
      return <DashboardPage />;
    case "customers":
      return <CustomersPage />;
    case "catalog":
      return <CatalogPage />;
    case "quotes":
      return <QuotesPage />;
    case "jobs":
      return <JobsPage />;
    case "invoices":
      return <InvoicesPage />;
    case "reports":
      return <ReportsPage />;
    case "settings":
      return <SettingsPage />;
  }
}

export function Layout({ initialTheme }: { initialTheme: Theme }) {
  const [active, setActive] = useState<SectionId>("dashboard");
  // Each navigation remounts the section (so clicking the current section returns to its list)
  // and may carry a record for it to open.
  const [visit, setVisit] = useState<{ key: number; recordId: number | null }>({
    key: 0,
    recordId: null,
  });
  const [theme, setThemeState] = useState<Theme>(initialTheme);
  const mainRef = useRef<HTMLElement>(null);
  // Bumped whenever the section or a page's view changes; focus follows it (never on first load,
  // so keyboard users start at the skip link and navigation).
  const [focusRequest, setFocusRequest] = useState(0);
  const requestFocus = useCallback(() => setFocusRequest((n) => n + 1), []);

  useEffect(() => {
    if (focusRequest > 0) mainRef.current?.focus();
  }, [focusRequest]);

  // Forms with unsaved changes; navigating away from them asks first.
  const unsavedForms = useRef(new Set<symbol>());
  const reportUnsaved = useCallback((form: symbol, dirty: boolean) => {
    if (dirty) unsavedForms.current.add(form);
    else unsavedForms.current.delete(form);
  }, []);
  const [leaving, setLeaving] = useState<{ section: SectionId; recordId?: number } | null>(null);

  function navigate(section: SectionId, recordId?: number) {
    setActive(section);
    setVisit((v) => ({ key: v.key + 1, recordId: recordId ?? null }));
    requestFocus();
  }

  function goTo(section: SectionId, recordId?: number) {
    if (unsavedForms.current.size > 0) setLeaving({ section, recordId });
    else navigate(section, recordId);
  }

  function discardAndLeave() {
    if (!leaving) return;
    unsavedForms.current.clear();
    setLeaving(null);
    navigate(leaving.section, leaving.recordId);
  }

  function toggleTheme() {
    const next: Theme = theme === "dark" ? "light" : "dark";
    setThemeState(next);
    void setTheme(next);
  }

  return (
    // The page scrolls inside <main>, so the sidebar always stays in view.
    <div className="grid h-screen grid-cols-[14.5rem_1fr] overflow-hidden bg-background">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:m-2 focus:rounded-md focus:bg-primary focus:px-3 focus:py-2 focus:text-primary-foreground"
      >
        Skip to content
      </a>

      <nav
        aria-label="Primary"
        className="flex flex-col gap-1 overflow-y-auto border-r bg-card/95 p-3 shadow-[1px_0_0_oklch(0_0_0/0.02)]"
      >
        <div className="mb-3 flex items-center gap-2.5 px-2 py-3">
          <div className="flex size-9 shrink-0 items-center justify-center rounded-[0.7rem] bg-primary text-primary-foreground shadow-sm">
            <Store className="size-5" aria-hidden />
          </div>
          <div className="leading-tight">
            <div className="text-base font-bold tracking-tight">RMB</div>
            <div className="text-[11px] text-muted-foreground">Business manager</div>
          </div>
        </div>
        {SECTIONS.map((section) => {
          const Icon = section.icon;
          const isActive = active === section.id;
          return (
            <button
              key={section.id}
              type="button"
              onClick={() => goTo(section.id)}
              aria-current={isActive ? "page" : undefined}
              className={cn(
                "relative flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-left text-sm font-medium transition-colors",
                "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
                isActive
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
              )}
            >
              <Icon className="size-4 shrink-0" />
              {section.label}
            </button>
          );
        })}

        <div className="mt-auto pt-2">
          <Button variant="ghost" size="sm" className="w-full justify-start" onClick={toggleTheme}>
            {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
            {theme === "dark" ? "Light mode" : "Dark mode"}
          </Button>
        </div>
      </nav>

      <main
        id="main"
        ref={mainRef}
        tabIndex={-1}
        className="overflow-y-auto p-6 outline-none lg:p-8"
      >
        <NavContext.Provider value={goTo}>
          <ViewFocusContext.Provider value={requestFocus}>
            <UnsavedChangesContext.Provider value={reportUnsaved}>
              <NavTargetContext.Provider value={visit.recordId}>
                <div key={visit.key} className="mx-auto max-w-7xl">
                  {renderSection(active)}
                </div>
              </NavTargetContext.Provider>
            </UnsavedChangesContext.Provider>
          </ViewFocusContext.Provider>
        </NavContext.Provider>
      </main>

      <ConfirmDialog
        open={leaving !== null}
        onClose={() => setLeaving(null)}
        onConfirm={discardAndLeave}
        title="Discard unsaved changes?"
        description="You have changes that haven't been saved. Leaving now discards them."
        confirmLabel="Discard changes"
        destructive
      />
    </div>
  );
}
