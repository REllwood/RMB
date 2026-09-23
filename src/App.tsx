import { useEffect, useState } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { LoaderCircle } from "lucide-react";
import { listen } from "@tauri-apps/api/event";

import { queryClient } from "@/lib/query";
import { initTheme, type Theme } from "@/lib/theme";
import { ToastProvider } from "@/components/ui/toast";
import { Layout } from "@/app/Layout";

function App() {
  const [theme, setTheme] = useState<Theme | null>(null);

  useEffect(() => {
    void initTheme().then(setTheme);
  }, []);

  // The launch backup and recurring generation finish after the window opens; refresh what they
  // may have changed.
  useEffect(() => {
    let stop: (() => void) | undefined;
    listen("startup-complete", () => {
      for (const key of ["startup-warning", "invoices", "dashboard", "recurring"]) {
        void queryClient.invalidateQueries({ queryKey: [key] });
      }
    })
      .then((unlisten) => {
        stop = unlisten;
      })
      .catch(() => {
        // Outside Tauri (tests) there are no backend events.
      });
    return () => stop?.();
  }, []);

  // Avoid a theme flash while still making startup progress visible and accessible.
  if (theme === null)
    return (
      <div
        role="status"
        className="flex min-h-screen items-center justify-center bg-background text-foreground"
      >
        <div className="flex items-center gap-3 rounded-xl border bg-card px-5 py-4 shadow-sm">
          <LoaderCircle className="size-5 animate-spin text-primary" aria-hidden />
          <div>
            <p className="text-sm font-semibold">RMB</p>
            <p className="text-xs text-muted-foreground">Opening your business…</p>
          </div>
        </div>
      </div>
    );

  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <Layout initialTheme={theme} />
      </ToastProvider>
    </QueryClientProvider>
  );
}

export default App;
