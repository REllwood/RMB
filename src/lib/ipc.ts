import { invoke } from "@tauri-apps/api/core";
import type {
  Customer,
  CustomerHistory,
  CustomerInput,
  CustomerSalesRow,
  DashboardSummary,
  InvoiceDetail,
  InvoiceRow,
  Item,
  ItemInput,
  Job,
  JobDetail,
  JobInput,
  JobMaterialInput,
  LineInput,
  MonthlySalesRow,
  Payment,
  QuoteDetail,
  QuoteRow,
  RecurringDetail,
  RecurringInput,
  RecurringListRow,
  Settings,
  StockMovement,
  TaxRate,
  TaxSummaryRow,
  TimeEntryInput,
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
  setLogo: (src: string) => invoke<string>("set_logo", { src }),
  clearLogo: () => invoke<void>("clear_logo"),

  // customers
  listCustomers: (search?: string) => invoke<Customer[]>("list_customers", { search }),
  getCustomer: (id: number) => invoke<Customer | null>("get_customer", { id }),
  createCustomer: (input: CustomerInput) => invoke<number>("create_customer", { input }),
  updateCustomer: (id: number, input: CustomerInput) =>
    invoke<void>("update_customer", { id, input }),
  deleteCustomer: (id: number) => invoke<void>("delete_customer", { id }),
  customerHistory: (id: number) => invoke<CustomerHistory>("customer_history", { id }),

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
  createInvoice: (customerId: number, lines: LineInput[], dueDate: string | null, notes: string) =>
    invoke<number>("create_invoice", { customerId, lines, dueDate, notes }),
  updateInvoiceDraft: (
    id: number,
    customerId: number,
    lines: LineInput[],
    dueDate: string | null,
    notes: string,
  ) => invoke<void>("update_invoice_draft", { id, customerId, lines, dueDate, notes }),
  deleteInvoiceDraft: (id: number) => invoke<void>("delete_invoice_draft", { id }),
  issueInvoice: (id: number) => invoke<void>("issue_invoice", { id }),
  voidInvoice: (id: number) => invoke<void>("void_invoice", { id }),
  recordPayment: (invoiceId: number, amountMinor: number, method: string, reference: string) =>
    invoke<number>("record_payment", { invoiceId, amountMinor, method, reference }),
  invoicePayments: (invoiceId: number) => invoke<Payment[]>("invoice_payments", { invoiceId }),
  deletePayment: (id: number) => invoke<void>("delete_payment", { id }),

  // dashboard
  dashboardSummary: () => invoke<DashboardSummary>("dashboard_summary"),

  // quotes
  listQuotes: () => invoke<QuoteRow[]>("list_quotes"),
  getQuote: (id: number) => invoke<QuoteDetail | null>("get_quote", { id }),
  createQuote: (customerId: number, lines: LineInput[], validUntil: string | null, notes: string) =>
    invoke<number>("create_quote", { customerId, lines, validUntil, notes }),
  updateQuoteDraft: (
    id: number,
    customerId: number,
    lines: LineInput[],
    validUntil: string | null,
    notes: string,
  ) => invoke<void>("update_quote_draft", { id, customerId, lines, validUntil, notes }),
  setQuoteStatus: (id: number, status: string) => invoke<void>("set_quote_status", { id, status }),
  deleteQuote: (id: number) => invoke<void>("delete_quote", { id }),
  convertQuoteToInvoice: (id: number) => invoke<number>("convert_quote_to_invoice", { id }),
  convertQuoteToJob: (quoteId: number) => invoke<number>("convert_quote_to_job", { quoteId }),

  // jobs + timekeeping
  listJobs: () => invoke<Job[]>("list_jobs"),
  getJob: (id: number) => invoke<JobDetail | null>("get_job", { id }),
  createJob: (input: JobInput) => invoke<number>("create_job", { input }),
  setJobStatus: (id: number, status: string) => invoke<void>("set_job_status", { id, status }),
  deleteJob: (id: number) => invoke<void>("delete_job", { id }),
  addTimeEntry: (jobId: number, entry: TimeEntryInput) =>
    invoke<number>("add_time_entry", { jobId, entry }),
  addJobMaterial: (jobId: number, material: JobMaterialInput) =>
    invoke<number>("add_job_material", { jobId, material }),
  deleteTimeEntry: (id: number) => invoke<void>("delete_time_entry", { id }),
  deleteJobMaterial: (id: number) => invoke<void>("delete_job_material", { id }),
  invoiceJob: (id: number) => invoke<number>("invoice_job", { id }),

  // recurring invoices
  listRecurring: () => invoke<RecurringListRow[]>("list_recurring"),
  getRecurring: (id: number) => invoke<RecurringDetail | null>("get_recurring", { id }),
  createRecurring: (input: RecurringInput, lines: LineInput[]) =>
    invoke<number>("create_recurring", { input, lines }),
  updateRecurring: (id: number, input: RecurringInput, lines: LineInput[]) =>
    invoke<void>("update_recurring", { id, input, lines }),
  setRecurringActive: (id: number, active: boolean) =>
    invoke<void>("set_recurring_active", { id, active }),
  deleteRecurring: (id: number) => invoke<void>("delete_recurring", { id }),
  runRecurringNow: () => invoke<number>("run_recurring_now"),

  // reports + csv exports
  reportTaxSummary: (from: string | null, to: string | null) =>
    invoke<TaxSummaryRow[]>("report_tax_summary", { from, to }),
  reportSalesMonthly: (from: string | null, to: string | null) =>
    invoke<MonthlySalesRow[]>("report_sales_monthly", { from, to }),
  reportSalesCustomers: (from: string | null, to: string | null) =>
    invoke<CustomerSalesRow[]>("report_sales_customers", { from, to }),
  exportInvoicesCsv: (dest: string, from: string | null, to: string | null) =>
    invoke<void>("export_invoices_csv", { dest, from, to }),
  exportPaymentsCsv: (dest: string, from: string | null, to: string | null) =>
    invoke<void>("export_payments_csv", { dest, from, to }),
  exportCustomersCsv: (dest: string) => invoke<void>("export_customers_csv", { dest }),

  // pdf export (dest path chosen via the dialog plugin in the UI)
  exportInvoicePdf: (id: number, dest: string) => invoke<void>("export_invoice_pdf", { id, dest }),
  exportQuotePdf: (id: number, dest: string) => invoke<void>("export_quote_pdf", { id, dest }),
  exportReceiptPdf: (id: number, dest: string) => invoke<void>("export_receipt_pdf", { id, dest }),
};
