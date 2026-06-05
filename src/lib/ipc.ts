import { invoke } from "@tauri-apps/api/core";
import type {
  Customer,
  CustomerInput,
  DashboardSummary,
  InvoiceDetail,
  InvoiceRow,
  Item,
  ItemInput,
  LineInput,
  Payment,
  QuoteDetail,
  QuoteRow,
  Settings,
  StockMovement,
  TaxRate,
} from "@/lib/types";

/**
 * Typed client for Tauri commands — the single boundary between the React UI and the Rust
 * backend. Top-level argument keys are camelCase (Tauri maps them to the snake_case Rust params);
 * nested object fields stay snake_case to match serde.
 */
export const ipc = {
  greet: (name: string) => invoke<string>("greet", { name }),

  // app meta + backup
  getMeta: (key: string) => invoke<string | null>("get_meta", { key }),
  setMeta: (key: string, value: string) => invoke<void>("set_meta", { key, value }),
  backupDatabase: (dest: string) => invoke<void>("backup_database", { dest }),
  restoreDatabase: (src: string) => invoke<void>("restore_database", { src }),

  // settings + tax
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (value: Settings) => invoke<void>("update_settings", { value }),
  listTaxRates: () => invoke<TaxRate[]>("list_tax_rates"),
  createTaxRate: (name: string, rateBp: number, inclusive: boolean) =>
    invoke<number>("create_tax_rate", { name, rateBp, inclusive }),
  updateTaxRate: (id: number, name: string, rateBp: number, inclusive: boolean) =>
    invoke<void>("update_tax_rate", { id, name, rateBp, inclusive }),
  archiveTaxRate: (id: number) => invoke<void>("archive_tax_rate", { id }),
  applyTaxPreset: (country: string) => invoke<void>("apply_tax_preset", { country }),

  // customers
  listCustomers: (search?: string) => invoke<Customer[]>("list_customers", { search }),
  getCustomer: (id: number) => invoke<Customer | null>("get_customer", { id }),
  createCustomer: (input: CustomerInput) => invoke<number>("create_customer", { input }),
  updateCustomer: (id: number, input: CustomerInput) =>
    invoke<void>("update_customer", { id, input }),
  deleteCustomer: (id: number) => invoke<void>("delete_customer", { id }),

  // catalog + stock
  listItems: (search?: string) => invoke<Item[]>("list_items", { search }),
  getItem: (id: number) => invoke<Item | null>("get_item", { id }),
  createItem: (input: ItemInput) => invoke<number>("create_item", { input }),
  updateItem: (id: number, input: ItemInput) => invoke<void>("update_item", { id, input }),
  deleteItem: (id: number) => invoke<void>("delete_item", { id }),
  adjustStock: (itemId: number, qtyDelta: number, note: string) =>
    invoke<void>("adjust_stock", { itemId, qtyDelta, note }),
  itemMovements: (itemId: number) => invoke<StockMovement[]>("item_movements", { itemId }),

  // invoices + payments
  listInvoices: () => invoke<InvoiceRow[]>("list_invoices"),
  getInvoice: (id: number) => invoke<InvoiceDetail | null>("get_invoice", { id }),
  createInvoice: (
    customerId: number,
    lines: LineInput[],
    dueDate: string | null,
    notes: string,
  ) => invoke<number>("create_invoice", { customerId, lines, dueDate, notes }),
  issueInvoice: (id: number) => invoke<void>("issue_invoice", { id }),
  voidInvoice: (id: number) => invoke<void>("void_invoice", { id }),
  recordPayment: (invoiceId: number, amountMinor: number, method: string, reference: string) =>
    invoke<number>("record_payment", { invoiceId, amountMinor, method, reference }),
  invoicePayments: (invoiceId: number) => invoke<Payment[]>("invoice_payments", { invoiceId }),

  // dashboard
  dashboardSummary: () => invoke<DashboardSummary>("dashboard_summary"),

  // quotes
  listQuotes: () => invoke<QuoteRow[]>("list_quotes"),
  getQuote: (id: number) => invoke<QuoteDetail | null>("get_quote", { id }),
  createQuote: (
    customerId: number,
    lines: LineInput[],
    validUntil: string | null,
    notes: string,
  ) => invoke<number>("create_quote", { customerId, lines, validUntil, notes }),
  setQuoteStatus: (id: number, status: string) =>
    invoke<void>("set_quote_status", { id, status }),
  deleteQuote: (id: number) => invoke<void>("delete_quote", { id }),
  convertQuoteToInvoice: (id: number) => invoke<number>("convert_quote_to_invoice", { id }),
};
