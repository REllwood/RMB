import type { ReactNode } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { NavTargetContext } from "@/app/nav";
import type {
  Customer,
  InvoiceDetail,
  InvoiceRow,
  Item,
  JobDetail,
  QuoteDetail,
  Settings,
  TaxRate,
} from "@/lib/types";
import { expectNoA11yViolations } from "../../test/axe";

const settings: Settings = {
  business_name: "Acme",
  address: "1 High St",
  email: "hello@acme.test",
  phone: "555",
  logo_path: null,
  currency: "USD",
  tax_label: "Tax",
  tax_number: "",
  prices_tax_inclusive: false,
  invoice_prefix: "INV-",
  invoice_next_seq: 2,
  quote_prefix: "Q-",
  quote_next_seq: 2,
  number_pad: 4,
  default_tax_rate_id: 1,
  default_due_days: 14,
  currency_locked: true,
};

const vat: TaxRate = { id: 1, name: "VAT 20%", rate_bp: 2000, inclusive: false, archived: false };

const customer: Customer = {
  id: 1,
  name: "Jane Doe",
  email: "jane@example.test",
  phone: "555",
  billing_address: "1 Low St\nTown",
  notes: "",
  created_at: "2026-06-05",
};

const widget: Item = {
  id: 1,
  kind: "product",
  name: "Widget",
  sku: "W-1",
  unit: "each",
  default_price_minor: 1000,
  default_tax_rate_id: 1,
  tracked: true,
  qty_on_hand: 3,
  reorder_point: 5,
  has_movements: true,
};

const line = {
  id: 1,
  item_id: 1,
  description: "Widget",
  quantity: "1",
  unit_price_minor: 1000,
  tax_rate_name: "VAT 20%",
  tax_rate_bp: 2000,
  tax_inclusive: false,
  net_minor: 1000,
  tax_minor: 200,
  gross_minor: 1200,
};

const invoiceRow: InvoiceRow = {
  id: 1,
  customer_id: 1,
  number: "INV-0001",
  status: "part_paid",
  issue_date: "2026-06-05",
  due_date: "2026-06-19",
  subtotal_minor: 1000,
  tax_minor: 200,
  total_minor: 1200,
  notes: "Thanks",
  created_at: "2026-06-05",
  source_quote_id: null,
  source_job_id: null,
  void_date: null,
  due_days: null,
  customer_name: "Jane Doe",
};

function invoice(row: Partial<InvoiceRow> = {}, paid = 500): InvoiceDetail {
  return {
    invoice: { ...invoiceRow, ...row },
    lines: [line],
    amount_paid_minor: paid,
    business_snapshot: null,
    customer_snapshot: null,
    tax_summary: "[]",
  };
}

const quote: QuoteDetail = {
  quote: {
    id: 1,
    customer_id: 1,
    number: "Q-0001",
    status: "sent",
    valid_until: "2026-07-01",
    subtotal_minor: 1000,
    tax_minor: 200,
    total_minor: 1200,
    notes: "",
    converted_invoice_id: null,
    created_at: "2026-06-05",
    customer_name: "Jane Doe",
    converted_invoice_number: null,
  },
  lines: [line],
  tax_summary: "[]",
};

const job: JobDetail = {
  job: {
    id: 1,
    customer_id: 1,
    title: "Rewire kitchen",
    description: "",
    status: "in_progress",
    source_quote_id: null,
    created_at: "2026-06-05",
    customer_name: "Jane Doe",
    invoice_id: null,
    invoice_number: null,
  },
  time_entries: [
    {
      id: 1,
      date: "2026-06-05",
      minutes: 90,
      rate_minor: 5000,
      description: "First fix",
      tax_rate_name: "VAT 20%",
      tax_rate_bp: 2000,
      tax_inclusive: false,
      invoiced: false,
    },
  ],
  materials: [
    {
      id: 1,
      item_id: 1,
      description: "Widget",
      quantity: "2",
      unit_price_minor: 1000,
      tax_rate_name: "VAT 20%",
      tax_rate_bp: 2000,
      tax_inclusive: false,
      invoiced: false,
    },
  ],
  labour_total_minor: 7500,
  materials_total_minor: 2000,
  subtotal_minor: 9500,
  tax_minor: 1900,
  total_minor: 11400,
  unbilled_total_minor: 11400,
};

const ipc = vi.hoisted(() => ({
  getMeta: vi.fn(async () => null),
  getSettings: vi.fn(),
  listTaxRates: vi.fn(),
  listArchivedTaxRates: vi.fn(),
  backupFolder: vi.fn(async () => "/data/backups"),
  listCustomers: vi.fn(),
  getCustomer: vi.fn(),
  customerHistory: vi.fn(),
  listItems: vi.fn(),
  listArchivedItems: vi.fn(async () => []),
  itemMovements: vi.fn(async () => []),
  listInvoices: vi.fn(async () => []),
  getInvoice: vi.fn(),
  invoicePayments: vi.fn(),
  listQuotes: vi.fn(async () => []),
  getQuote: vi.fn(),
  listJobs: vi.fn(async () => []),
  getJob: vi.fn(),
  listRecurring: vi.fn(async () => []),
}));
vi.mock("@/lib/ipc", () => ({ ipc }));

import { CustomersPage } from "@/features/customers/CustomersPage";
import { CatalogPage } from "@/features/catalog/CatalogPage";
import { InvoicesPage } from "@/features/invoices/InvoicesPage";
import { QuotesPage } from "@/features/quotes/QuotesPage";
import { JobsPage } from "@/features/jobs/JobsPage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import { ToastProvider } from "@/components/ui/toast";

beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
});

beforeEach(() => {
  ipc.getSettings.mockResolvedValue(settings);
  ipc.listTaxRates.mockResolvedValue([vat]);
  ipc.listArchivedTaxRates.mockResolvedValue([
    { id: 2, name: "Old rate", rate_bp: 1750, inclusive: false, archived: true },
  ]);
  ipc.listCustomers.mockResolvedValue([customer]);
  ipc.getCustomer.mockResolvedValue(customer);
  ipc.customerHistory.mockResolvedValue({ quotes: [], jobs: [], invoices: [invoiceRow] });
  ipc.listItems.mockResolvedValue([widget]);
  ipc.getInvoice.mockResolvedValue(invoice());
  ipc.invoicePayments.mockResolvedValue([
    { id: 1, date: "2026-06-06 10:00:00", amount_minor: 500, method: "cash", reference: "" },
  ]);
  ipc.getQuote.mockResolvedValue(quote);
  ipc.getJob.mockResolvedValue(job);
});

afterEach(cleanup);

/** Render a page opened on `recordId`, as navigation from another section does. */
function renderPage(ui: ReactNode, recordId: number | null = null) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ToastProvider>
        <NavTargetContext.Provider value={recordId}>{ui}</NavTargetContext.Provider>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

describe("detail views, forms and dialogs (WCAG-AA, jsdom)", () => {
  it("invoice detail and the payment dialog", async () => {
    const user = userEvent.setup();
    renderPage(<InvoicesPage />, 1);
    await screen.findByRole("button", { name: "Record payment" });
    await screen.findByText("cash");
    await expectNoA11yViolations(document.body);

    await user.click(screen.getByRole("button", { name: "Record payment" }));
    const dialog = screen.getByRole("dialog", { name: "Record payment", hidden: true });
    expect(within(dialog).getByLabelText(/Amount/)).toHaveFocus();
    await expectNoA11yViolations(document.body);
  });

  it("a zero-total invoice, marked paid at issue, can still be voided", async () => {
    ipc.getInvoice.mockResolvedValue(
      invoice({ status: "paid", subtotal_minor: 0, tax_minor: 0, total_minor: 0 }, 0),
    );
    ipc.invoicePayments.mockResolvedValue([]);
    renderPage(<InvoicesPage />, 1);
    expect(await screen.findByRole("button", { name: "Void" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Record payment" })).not.toBeInTheDocument();
  });

  it("a paid invoice with payments can't be voided", async () => {
    ipc.getInvoice.mockResolvedValue(invoice({ status: "paid" }, 1200));
    renderPage(<InvoicesPage />, 1);
    await screen.findByText("cash");
    expect(screen.queryByRole("button", { name: "Void" })).not.toBeInTheDocument();
  });

  it("new invoice form", async () => {
    const user = userEvent.setup();
    renderPage(<InvoicesPage />);
    await user.click((await screen.findAllByRole("button", { name: /New invoice/ }))[0]);
    await screen.findByRole("heading", { name: "New invoice" });
    await expectNoA11yViolations(document.body);
  });

  it("quote detail", async () => {
    renderPage(<QuotesPage />, 1);
    await screen.findByText("Q-0001", { exact: false });
    await expectNoA11yViolations(document.body);
  });

  it("job detail, with status as pressed buttons", async () => {
    renderPage(<JobsPage />, 1);
    const status = await screen.findByRole("group", { name: "Job status" });
    expect(within(status).getByRole("button", { name: "In progress" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(within(status).getByRole("button", { name: "Done" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    await expectNoA11yViolations(document.body);
  });

  it("customer detail and edit form", async () => {
    const user = userEvent.setup();
    renderPage(<CustomersPage />, 1);
    await screen.findByText("INV-0001");
    await expectNoA11yViolations(document.body);

    await user.click(screen.getByRole("button", { name: /Edit/ }));
    expect(screen.getByLabelText("Billing address").tagName).toBe("TEXTAREA");
    await expectNoA11yViolations(document.body);
  });

  it("catalog item form and stock history", async () => {
    const user = userEvent.setup();
    renderPage(<CatalogPage />);
    await user.click(await screen.findByRole("button", { name: "Edit Widget" }));
    await expectNoA11yViolations(document.body);

    await user.click(screen.getByRole("button", { name: "Stock history for Widget" }));
    await screen.findByText("No stock changes recorded yet.");
    await expectNoA11yViolations(document.body);
  });

  it("settings with archived tax rates and the archive confirmation", async () => {
    const user = userEvent.setup();
    renderPage(<SettingsPage />);
    await user.click(await screen.findByRole("button", { name: /Show archived rates \(1\)/ }));
    expect(screen.getByRole("button", { name: "Restore Old rate" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Archive VAT 20%" }));
    screen.getByRole("dialog", { name: "Archive VAT 20%?", hidden: true });
    await expectNoA11yViolations(document.body);
  });
});
