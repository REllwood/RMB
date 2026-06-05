import { useEffect, useRef, useState } from "react";
import { Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { setTheme, type Theme } from "@/lib/theme";
import { SECTIONS, type SectionId } from "@/app/sections";

import { DashboardPage } from "@/features/dashboard/DashboardPage";
import { CustomersPage } from "@/features/customers/CustomersPage";
import { CatalogPage } from "@/features/catalog/CatalogPage";
import { QuotesPage } from "@/features/quotes/QuotesPage";
import { JobsPage } from "@/features/jobs/JobsPage";
import { InvoicesPage } from "@/features/invoices/InvoicesPage";
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
    case "settings":
      return <SettingsPage />;
  }
}

export function Layout({ initialTheme }: { initialTheme: Theme }) {
  const [active, setActive] = useState<SectionId>("dashboard");
  const [theme, setThemeState] = useState<Theme>(initialTheme);
  const mainRef = useRef<HTMLElement>(null);

  // SPA focus management: move focus to the main region when the section changes.
  useEffect(() => {
    mainRef.current?.focus();
  }, [active]);

  function toggleTheme() {
    const next: Theme = theme === "dark" ? "light" : "dark";
    setThemeState(next);
    void setTheme(next);
  }

  return (
    <div className="grid min-h-screen grid-cols-[14rem_1fr] bg-background">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:m-2 focus:rounded-md focus:bg-primary focus:px-3 focus:py-2 focus:text-primary-foreground"
      >
        Skip to content
      </a>

      <nav aria-label="Primary" className="flex flex-col gap-1 border-r bg-card p-3">
        <div className="px-2 py-3 text-xl font-bold tracking-tight">RMB</div>
        {SECTIONS.map((section) => {
          const Icon = section.icon;
          const isActive = active === section.id;
          return (
            <button
              key={section.id}
              type="button"
              onClick={() => setActive(section.id)}
              aria-current={isActive ? "page" : undefined}
              className={cn(
                "flex items-center gap-2.5 rounded-md px-3 py-2 text-left text-sm font-medium transition-colors",
                "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
                isActive
                  ? "bg-primary text-primary-foreground"
                  : "hover:bg-accent hover:text-accent-foreground",
              )}
            >
              <Icon className="size-4 shrink-0" />
              {section.label}
            </button>
          );
        })}

        <div className="mt-auto pt-2">
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start"
            onClick={toggleTheme}
            aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} theme`}
          >
            {theme === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
            {theme === "dark" ? "Light mode" : "Dark mode"}
          </Button>
        </div>
      </nav>

      <main id="main" ref={mainRef} tabIndex={-1} className="overflow-y-auto p-6 outline-none">
        <div className="mx-auto max-w-5xl">{renderSection(active)}</div>
      </main>
    </div>
  );
}
