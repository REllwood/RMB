import { describe, it } from "vitest";
import { render } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { Layout } from "@/app/Layout";
import { expectNoA11yViolations } from "../../test/axe";

describe("Layout", () => {
  it("has no WCAG-AA accessibility violations", async () => {
    const qc = new QueryClient();
    const { container } = render(
      <QueryClientProvider client={qc}>
        <Layout initialTheme="light" />
      </QueryClientProvider>,
    );
    await expectNoA11yViolations(container);
  });
});
