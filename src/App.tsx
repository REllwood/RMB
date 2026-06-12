import { useEffect, useState } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import { queryClient } from "@/lib/query";
import { initTheme, type Theme } from "@/lib/theme";
import { ToastProvider } from "@/components/ui/toast";
import { Layout } from "@/app/Layout";

function App() {
  const [theme, setTheme] = useState<Theme | null>(null);

  useEffect(() => {
    void initTheme().then(setTheme);
  }, []);

  // Avoid a theme flash: render once the persisted/system theme is resolved.
  if (theme === null) return null;

  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <Layout initialTheme={theme} />
      </ToastProvider>
    </QueryClientProvider>
  );
}

export default App;
