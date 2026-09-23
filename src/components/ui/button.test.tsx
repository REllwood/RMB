import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Button } from "@/components/ui/button";

describe("Button loading state", () => {
  it("shows the ordinary label until loading begins", () => {
    render(<Button loadingLabel="Saving">Save</Button>);

    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
    expect(screen.queryByText("Saving")).not.toBeInTheDocument();
  });

  it("disables interaction and exposes progress to assistive technology", () => {
    const onClick = vi.fn();
    render(
      <Button loading loadingLabel="Saving" onClick={onClick}>
        Save
      </Button>,
    );

    const button = screen.getByRole("button", { name: "Saving" });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    button.click();
    expect(onClick).not.toHaveBeenCalled();
  });

  it("retains an icon button's accessible name while loading", () => {
    render(
      <Button size="icon" loading aria-label="Exporting report">
        Export
      </Button>,
    );

    expect(screen.getByRole("button", { name: "Exporting report" })).toBeDisabled();
  });
});
