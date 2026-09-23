import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ErrorState } from "@/components/ui/states";

describe("ErrorState", () => {
  it("shows progress and blocks duplicate retries until retry completes", async () => {
    let finish: (() => void) | undefined;
    const retry = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    render(<ErrorState error={new Error("Unavailable")} onRetry={retry} />);

    await user.click(screen.getByRole("button", { name: "Try again" }));
    const pending = screen.getByRole("button", { name: "Trying again…" });
    expect(pending).toBeDisabled();
    expect(pending).toHaveAttribute("aria-busy", "true");
    expect(retry).toHaveBeenCalledTimes(1);

    finish?.();
    expect(await screen.findByRole("button", { name: "Try again" })).toBeEnabled();
  });
});
