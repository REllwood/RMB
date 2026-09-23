import type { ReactNode } from "react";
import { afterEach, describe, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { expectNoA11yViolations } from "../../test/axe";

// Mock the Tauri IPC boundary so screens render populated, offline, in jsdom.
vi.mock("@/lib/ipc", () => ({
  ipc: {
    getMeta: async () => null,
    getSettings: async () => ({
      business_name: "Acme",
      address: "1 St",
      email: "a@b.c",
      phone: "555",
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
      currency_locked: false,
    }),
    listTaxRates: async () => [
      { id: 1, name: "VAT 20%", rate_bp: 2000, inclusive: false, archived: false },
    ],
    listCustomers: async () => [
      {
        id: 1,
        name: "Jane Doe",
        email: "j@d.c",
        phone: "555",
        billing_address: "1 St",
        notes: "",
        created_at: "2026-06-05",
      },
    ],
    listItems: async () => [
      {
        id: 1,
        kind: "product",
        name: "Widget",
        sku: "W",
        unit: "each",
        default_price_minor: 1000,
        default_tax_rate_id: null,
        tracked: true,
        qty_on_hand: 3,
        reorder_point: 5,
      },
    ],
    listInvoices: async () => [
      {
        id: 1,
        customer_id: 1,
        number: "INV-0001",
        status: "issued",
        issue_date: "2026-06-05",
        due_date: null,
        subtotal_minor: 1000,
        tax_minor: 200,
        total_minor: 1200,
        notes: "",
        created_at: "2026-06-05",
      },
    ],
    listQuotes: async () => [
      {
        id: 1,
        customer_id: 1,
        number: "Q-0001",
        status: "draft",
        valid_until: null,
        subtotal_minor: 1000,
        tax_minor: 200,
        total_minor: 1200,
        notes: "",
        converted_invoice_id: null,
        created_at: "2026-06-05",
      },
    ],
    listJobs: async () => [
      {
        id: 1,
        customer_id: 1,
        title: "Rewire",
        description: "",
        status: "open",
        source_quote_id: null,
        created_at: "2026-06-05",
      },
    ],
    getCustomer: async () => ({
      id: 1,
      name: "Jane Doe",
      email: "j@d.c",
      phone: "555",
      billing_address: "1 St",
      notes: "",
      created_at: "2026-06-05",
    }),
    customerHistory: async () => ({
      quotes: [
        {
          id: 1,
          customer_id: 1,
          number: "Q-0001",
          status: "draft",
          valid_until: null,
          subtotal_minor: 1000,
          tax_minor: 200,
          total_minor: 1200,
          notes: "",
          converted_invoice_id: null,
          created_at: "2026-06-05",
        },
      ],
      jobs: [
        {
          id: 1,
          customer_id: 1,
          title: "Rewire",
          description: "",
          status: "open",
          source_quote_id: null,
          created_at: "2026-06-05",
        },
      ],
      invoices: [
        {
          id: 1,
          customer_id: 1,
          number: "INV-0001",
          status: "issued",
          issue_date: "2026-06-05",
          due_date: null,
          subtotal_minor: 1000,
          tax_minor: 200,
          total_minor: 1200,
          notes: "",
          created_at: "2026-06-05",
        },
      ],
    }),
    invoicePayments: async () => [
      { id: 1, date: "2026-06-05 10:00:00", amount_minor: 500, method: "cash", reference: "" },
    ],
    dashboardSummary: async () => ({
      outstanding_minor: 1200,
      draft_count: 0,
      unpaid_count: 1,
      overdue_count: 1,
      paid_count: 0,
      low_stock: [{ id: 1, name: "Widget", qty_on_hand: 3, reorder_point: 5 }],
    }),
    reportTaxSummary: async () => [
      {
        tax_rate_name: "VAT 20%",
        tax_rate_bp: 2000,
        net_minor: 1000,
        tax_minor: 200,
        gross_minor: 1200,
      },
    ],
    reportSalesMonthly: async () => [
      { month: "2026-06", invoice_count: 1, net_minor: 1000, tax_minor: 200, gross_minor: 1200 },
    ],
    reportSalesCustomers: async () => [
      { customer_id: 1, name: "Jane Doe", invoice_count: 1, gross_minor: 1200, paid_minor: 500 },
    ],
    listRecurring: async () => [],
  },
}));

import { DashboardPage } from "@/features/dashboard/DashboardPage";
import { CustomersPage } from "@/features/customers/CustomersPage";
import { CatalogPage } from "@/features/catalog/CatalogPage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import { InvoicesPage } from "@/features/invoices/InvoicesPage";
import { QuotesPage } from "@/features/quotes/QuotesPage";
import { JobsPage } from "@/features/jobs/JobsPage";
import { ReportsPage } from "@/features/reports/ReportsPage";

afterEach(cleanup);

function renderPage(ui: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

describe("screen accessibility (WCAG-AA, jsdom)", () => {
  it("Dashboard", async () => {
    renderPage(<DashboardPage />);
    await screen.findByText("Money owed to you");
    await expectNoA11yViolations(document.body);
  });

  it("Customers", async () => {
    renderPage(<CustomersPage />);
    await screen.findByText("Jane Doe");
    await expectNoA11yViolations(document.body);
  });

  it("Catalog", async () => {
    renderPage(<CatalogPage />);
    await screen.findByText("Widget");
    await expectNoA11yViolations(document.body);
  });

  it("Settings", async () => {
    renderPage(<SettingsPage />);
    await screen.findByText("Business");
    await expectNoA11yViolations(document.body);
  });

  it("Invoices", async () => {
    renderPage(<InvoicesPage />);
    await screen.findByText("INV-0001");
    await expectNoA11yViolations(document.body);
  });

  it("Quotes", async () => {
    renderPage(<QuotesPage />);
    await screen.findByText("Q-0001");
    await expectNoA11yViolations(document.body);
  });

  it("Jobs", async () => {
    renderPage(<JobsPage />);
    await screen.findByText("Rewire");
    await expectNoA11yViolations(document.body);
  });

  it("Reports", async () => {
    renderPage(<ReportsPage />);
    await screen.findByText("VAT 20%");
    await expectNoA11yViolations(document.body);
  });
});
