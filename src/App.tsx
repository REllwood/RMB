import { useEffect, useState } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { LoaderCircle } from "lucide-react";

import { queryClient } from "@/lib/query";
import { initTheme, type Theme } from "@/lib/theme";
import { ToastProvider } from "@/components/ui/toast";
import { Layout } from "@/app/Layout";

function App() {
  const [theme, setTheme] = useState<Theme | null>(null);

  useEffect(() => {
    void initTheme().then(setTheme);
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
