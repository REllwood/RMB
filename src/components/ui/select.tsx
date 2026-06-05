import * as React from "react";
import { cn } from "@/lib/utils";

/** Native <select> — fully keyboard- and screen-reader-accessible, styled to match. */
function Select({ className, ...props }: React.ComponentProps<"select">) {
  return (
    <select
      data-slot="select"
      className={cn(
        "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
        "aria-invalid:border-destructive aria-invalid:ring-destructive/20",
        className,
      )}
      {...props}
    />
  );
}

export { Select };
