import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { expectNoA11yViolations } from "../../test/axe";

// The dashboard loads on start; give it data so the populated layout is what gets checked.
vi.mock("@/lib/ipc", () => ({
  ipc: {
    getMeta: async () => null,
    getSettings: async () => ({
      business_name: "Acme",
      address: "",
      email: "",
      phone: "",
      logo_path: null,
      currency: "USD",
      tax_label: "Tax",
      tax_number: "",
      prices_tax_inclusive: false,
      invoice_prefix: "INV-",
      invoice_next_seq: 1,
      quote_prefix: "Q-",
      quote_next_seq: 1,
      number_pad: 4,
      default_tax_rate_id: null,
      default_due_days: null,
      currency_locked: false,
    }),
    dashboardSummary: async () => ({
      outstanding_minor: 0,
      draft_count: 0,
      unpaid_count: 0,
      overdue_count: 0,
      paid_count: 0,
      low_stock: [],
    }),
    listInvoices: async () => [],
  },
}));

import { Layout } from "@/app/Layout";

afterEach(cleanup);

describe("Layout", () => {
  it("has no WCAG-AA accessibility violations", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <Layout initialTheme="light" />
      </QueryClientProvider>,
    );
    await screen.findByText("Money owed to you");
    expect(screen.getByRole("button", { name: "Dashboard" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expectNoA11yViolations(container);
  });
});
