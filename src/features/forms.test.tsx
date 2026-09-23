import type { ReactNode } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import type { Item } from "@/lib/types";

const widget: Item = {
  id: 1,
  kind: "product",
  name: "Widget",
  sku: "W-1",
  unit: "each",
  default_price_minor: 1000,
  default_tax_rate_id: null,
  tracked: true,
  qty_on_hand: 3,
  reorder_point: 5,
  has_movements: true,
};

const ipc = vi.hoisted(() => ({
  getMeta: vi.fn(async () => null),
  dismissStartupWarning: vi.fn(async () => {}),
  getSettings: vi.fn(async () => ({
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
  })),
  dashboardSummary: vi.fn(async () => ({
    outstanding_minor: 0,
    draft_count: 0,
    unpaid_count: 0,
    overdue_count: 0,
    paid_count: 0,
    low_stock: [],
  })),
  listInvoices: vi.fn(async () => []),
  listCustomers: vi.fn(async () => []),
  createCustomer: vi.fn(async () => 7),
  listTaxRates: vi.fn(async () => []),
  listItems: vi.fn(async (): Promise<Item[]> => []),
  listArchivedItems: vi.fn(async (): Promise<Item[]> => []),
  createItem: vi.fn(async () => 2),
  updateItem: vi.fn(async () => {}),
  restoreItem: vi.fn(async () => {}),
  itemMovements: vi.fn(async () => [
    {
      id: 2,
      qty_delta: -2,
      reason: "sale",
      occurred_at: "2026-06-05 10:00:00",
      note: "",
      invoice_number: "INV-0001",
    },
    {
      id: 1,
      qty_delta: 5,
      reason: "adjustment",
      occurred_at: "2026-06-01 09:00:00",
      note: "opening",
      invoice_number: null,
    },
  ]),
}));
vi.mock("@/lib/ipc", () => ({ ipc }));

import { Layout } from "@/app/Layout";
import { CatalogPage } from "@/features/catalog/CatalogPage";
import { ToastProvider } from "@/components/ui/toast";

// jsdom has no top layer; model just enough of <dialog> for the component's behaviour.
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
  vi.clearAllMocks();
  ipc.listItems.mockResolvedValue([]);
  ipc.listArchivedItems.mockResolvedValue([]);
});

afterEach(cleanup);

function renderWithQuery(ui: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ToastProvider>{ui}</ToastProvider>
    </QueryClientProvider>,
  );
}

describe("catalog", () => {
  it("creates an item when Enter is pressed in the form", async () => {
    const user = userEvent.setup();
    renderWithQuery(<CatalogPage />);
    await user.click((await screen.findAllByRole("button", { name: /Add item/ }))[0]);
    const name = screen.getByLabelText(/^Name/);
    await user.type(name, "Gadget{Enter}");
    expect(ipc.createItem).toHaveBeenCalledTimes(1);
    expect(ipc.createItem).toHaveBeenCalledWith(expect.objectContaining({ name: "Gadget" }));
  });

  it("shows SKUs, names each row's actions and locks the type once stock exists", async () => {
    ipc.listItems.mockResolvedValue([widget]);
    const user = userEvent.setup();
    renderWithQuery(<CatalogPage />);
    expect(await screen.findByText("W-1")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Edit Widget" }));
    const type = screen.getByLabelText("Type");
    expect(type).toBeDisabled();
    expect(type).toHaveAccessibleDescription(/stays a tracked product/);
    // The form takes focus at its first editable field.
    expect(screen.getByLabelText(/^Name/)).toHaveFocus();
  });

  it("lists stock history with the invoice behind each sale", async () => {
    ipc.listItems.mockResolvedValue([widget]);
    const user = userEvent.setup();
    renderWithQuery(<CatalogPage />);
    await user.click(await screen.findByRole("button", { name: "Stock history for Widget" }));
    const dialog = screen.getByRole("dialog", { name: "Stock history — Widget", hidden: true });
    expect(await within(dialog).findByText("Invoice INV-0001")).toBeInTheDocument();
    expect(within(dialog).getByText("+5")).toBeInTheDocument();
    expect(within(dialog).getByText("opening")).toBeInTheDocument();
  });

  it("restores archived items", async () => {
    ipc.listArchivedItems.mockResolvedValue([{ ...widget, id: 9, name: "Old widget" }]);
    const user = userEvent.setup();
    renderWithQuery(<CatalogPage />);
    await user.click(await screen.findByRole("button", { name: /Show archived items \(1\)/ }));
    await user.click(screen.getByRole("button", { name: "Restore Old widget" }));
    expect(ipc.restoreItem).toHaveBeenCalledWith(9);
  });
});

describe("unsaved changes", () => {
  it("asks before leaving a section with a half-filled form", async () => {
    const user = userEvent.setup();
    renderWithQuery(<Layout initialTheme="light" />);
    const nav = screen.getByRole("navigation", { name: "Primary" });

    // An untouched form leaves without asking.
    await user.click(within(nav).getByRole("button", { name: "Customers" }));
    await user.click((await screen.findAllByRole("button", { name: /Add customer/ }))[0]);
    await user.click(within(nav).getByRole("button", { name: "Catalog" }));
    expect(await screen.findByRole("heading", { name: "Catalog" })).toBeInTheDocument();

    await user.click(within(nav).getByRole("button", { name: "Customers" }));
    await user.click((await screen.findAllByRole("button", { name: /Add customer/ }))[0]);
    await user.type(screen.getByLabelText(/^Name/), "Half typed");
    await user.click(within(nav).getByRole("button", { name: "Catalog" }));

    // Staying keeps the typed value.
    const dialog = screen.getByRole("dialog", { hidden: true });
    expect(dialog).toHaveAccessibleName("Discard unsaved changes?");
    await user.click(within(dialog).getByRole("button", { name: "Cancel", hidden: true }));
    expect(screen.getByLabelText(/^Name/)).toHaveValue("Half typed");

    // Discarding leaves.
    await user.click(within(nav).getByRole("button", { name: "Catalog" }));
    await user.click(
      within(screen.getByRole("dialog", { hidden: true })).getByRole("button", {
        name: "Discard changes",
        hidden: true,
      }),
    );
    expect(await screen.findByRole("heading", { name: "Catalog" })).toBeInTheDocument();
    expect(screen.queryByDisplayValue("Half typed")).not.toBeInTheDocument();
  });

  it("saves the customer form with Enter", async () => {
    const user = userEvent.setup();
    renderWithQuery(<Layout initialTheme="light" />);
    const nav = screen.getByRole("navigation", { name: "Primary" });
    await user.click(within(nav).getByRole("button", { name: "Customers" }));
    await user.click((await screen.findAllByRole("button", { name: /Add customer/ }))[0]);
    await user.type(screen.getByLabelText(/^Name/), "Jane{Enter}");
    expect(ipc.createCustomer).toHaveBeenCalledWith(expect.objectContaining({ name: "Jane" }));
  });
});
