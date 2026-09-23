// TypeScript mirrors of the Rust DTOs. Field names are snake_case to match serde (these objects
// are (de)serialized by serde; only top-level command argument keys are camelCased by Tauri).

export interface Settings {
  business_name: string;
  address: string;
  email: string;
  phone: string;
  logo_path: string | null;
  currency: string;
  tax_label: string;
  tax_number: string;
  prices_tax_inclusive: boolean;
  invoice_prefix: string;
  invoice_next_seq: number;
  quote_prefix: string;
  quote_next_seq: number;
  number_pad: number;
  /** Tax rate new lines start with; null falls back to the first non-zero rate. */
  default_tax_rate_id: number | null;
  /** Payment terms for new drafts without a due date (days after issue); null for none. */
  default_due_days: number | null;
  /** Read-only: an invoice has been issued, so the currency can no longer change. */
  currency_locked: boolean;
}

export interface TaxRate {
  id: number;
  name: string;
  rate_bp: number;
  inclusive: boolean;
  archived: boolean;
}

export interface Customer {
  id: number;
  name: string;
  email: string;
  phone: string;
  billing_address: string;
  notes: string;
  created_at: string;
}

export interface CustomerInput {
  name: string;
  email: string;
  phone: string;
  billing_address: string;
  notes: string;
}

export interface Item {
  id: number;
  kind: string; // "product" | "service"
  name: string;
  sku: string;
  unit: string;
  default_price_minor: number;
  default_tax_rate_id: number | null;
  tracked: boolean;
  qty_on_hand: number;
  reorder_point: number | null;
  /** Stock has been recorded, so it must stay a tracked product. */
  has_movements: boolean;
}

export interface ItemInput {
  kind: string;
  name: string;
  sku: string;
  unit: string;
  default_price_minor: number;
  default_tax_rate_id: number | null;
  tracked: boolean;
  reorder_point: number | null;
}

export interface StockMovement {
  id: number;
  qty_delta: number;
  reason: string;
  occurred_at: string; // UTC, "YYYY-MM-DD HH:MM:SS"
  note: string;
  /** The invoice a sale (or its reversal on void) belongs to. */
  invoice_number: string | null;
}

export interface LineInput {
  item_id: number | null;
  description: string;
  quantity: string; // exact decimal as string
  unit_price_minor: number;
  tax_rate_name: string;
  tax_rate_bp: number;
  tax_inclusive: boolean;
}

export interface InvoiceRow {
  id: number;
  customer_id: number;
  number: string | null;
  status: string;
  issue_date: string | null;
  due_date: string | null;
  subtotal_minor: number;
  tax_minor: number;
  total_minor: number;
  notes: string;
  created_at: string;
  source_quote_id: number | null;
  source_job_id: number | null;
  /** Local date it was voided, if it was. */
  void_date: string | null;
  /** Draft payment terms: due this many days after issue. */
  due_days: number | null;
  /** Name frozen on the issued invoice (current name for drafts); kept for deleted customers. */
  customer_name: string;
}

export interface InvoiceLineRow {
  id: number;
  item_id: number | null;
  description: string;
  quantity: string;
  unit_price_minor: number;
  tax_rate_name: string;
  tax_rate_bp: number;
  tax_inclusive: boolean;
  net_minor: number;
  tax_minor: number;
  gross_minor: number;
}

export interface InvoiceDetail {
  invoice: InvoiceRow;
  lines: InvoiceLineRow[];
  amount_paid_minor: number;
  business_snapshot: string | null;
  customer_snapshot: string | null;
  tax_summary: string;
}

export interface Payment {
  id: number;
  date: string;
  amount_minor: number;
  method: string;
  reference: string;
}

export interface LowStockItem {
  id: number;
  name: string;
  qty_on_hand: number;
  reorder_point: number | null;
}

export interface DashboardSummary {
  outstanding_minor: number;
  draft_count: number;
  unpaid_count: number;
  overdue_count: number;
  paid_count: number;
  low_stock: LowStockItem[];
}

export interface QuoteRow {
  id: number;
  customer_id: number;
  number: string | null;
  status: string;
  valid_until: string | null;
  subtotal_minor: number;
  tax_minor: number;
  total_minor: number;
  notes: string;
  converted_invoice_id: number | null;
  created_at: string;
  /** Kept for deleted customers. */
  customer_name: string;
  /** Number of the invoice it converted to, once issued. */
  converted_invoice_number: string | null;
}

export type QuoteLineRow = InvoiceLineRow;

export interface QuoteDetail {
  quote: QuoteRow;
  lines: QuoteLineRow[];
  tax_summary: string;
}

export interface Job {
  id: number;
  customer_id: number;
  title: string;
  description: string;
  status: string;
  source_quote_id: number | null;
  created_at: string;
  /** Kept for deleted customers. */
  customer_name: string;
  /** The live invoice that billed this job, if any. */
  invoice_id: number | null;
  invoice_number: string | null;
}

export interface JobInput {
  customer_id: number;
  title: string;
  description: string;
}

export interface TimeEntry {
  id: number;
  date: string;
  minutes: number;
  rate_minor: number;
  description: string;
  tax_rate_name: string;
  tax_rate_bp: number;
  tax_inclusive: boolean;
  invoiced: boolean;
}

export interface TimeEntryInput {
  date: string;
  minutes: number;
  rate_minor: number;
  description: string;
  tax_rate_name: string;
  tax_rate_bp: number;
  tax_inclusive: boolean;
}

export interface JobMaterial {
  id: number;
  item_id: number | null;
  description: string;
  quantity: string;
  unit_price_minor: number;
  tax_rate_name: string;
  tax_rate_bp: number;
  tax_inclusive: boolean;
  invoiced: boolean;
}

export interface JobMaterialInput {
  item_id: number | null;
  description: string;
  quantity: string;
  unit_price_minor: number;
  tax_rate_name: string;
  tax_rate_bp: number;
  tax_inclusive: boolean;
}

export interface JobDetail {
  job: Job;
  time_entries: TimeEntry[];
  materials: JobMaterial[];
  labour_total_minor: number;
  materials_total_minor: number;
  /** The whole job with tax applied per line, as its invoice would be. */
  subtotal_minor: number;
  tax_minor: number;
  total_minor: number;
  /** Total (with tax) of the work not yet invoiced. */
  unbilled_total_minor: number;
}

export interface CustomerHistory {
  quotes: QuoteRow[];
  jobs: Job[];
  invoices: InvoiceRow[];
}

export interface RecurringInvoice {
  id: number;
  customer_id: number;
  frequency: string; // weekly|fortnightly|monthly|quarterly|yearly
  next_date: string;
  end_date: string | null;
  due_days: number | null;
  anchor_day: number;
  notes: string;
  active: boolean;
  created_at: string;
}

/** List row: schedule fields flattened + computed template total. */
export type RecurringListRow = RecurringInvoice & {
  total_minor: number;
  /** Why the schedule can't generate as stored; editing it fixes this. */
  problem: string | null;
  /** Past its end date — it won't generate again unless the end date moves. */
  ended: boolean;
  customer_name: string;
};

/** Result of generating due recurring invoices. */
export interface RunReport {
  created: number[];
  /** One message per schedule that couldn't generate. */
  problems: string[];
}

/** What resuming a paused schedule would do. */
export interface ResumePreview {
  /** Invoices missed while paused. */
  missed: number;
  /** Where the schedule continues if the missed invoices are skipped. */
  skip_to: string;
}

export interface RecurringLineRow {
  id: number;
  item_id: number | null;
  description: string;
  quantity: string;
  unit_price_minor: number;
  tax_rate_name: string;
  tax_rate_bp: number;
  tax_inclusive: boolean;
}

export interface RecurringDetail {
  schedule: RecurringInvoice;
  lines: RecurringLineRow[];
}

export interface RecurringInput {
  customer_id: number;
  frequency: string;
  next_date: string;
  end_date: string | null;
  due_days: number | null;
  notes: string;
}

export interface TaxSummaryRow {
  tax_rate_name: string;
  tax_rate_bp: number;
  net_minor: number;
  tax_minor: number;
  gross_minor: number;
}

export interface MonthlySalesRow {
  month: string; // YYYY-MM
  /** Invoices issued in the month (including any voided later). */
  invoice_count: number;
  /** Invoices voided in the month; subtracted from its totals. */
  voided_count: number;
  net_minor: number;
  tax_minor: number;
  gross_minor: number;
}

export interface CustomerSalesRow {
  customer_id: number;
  name: string;
  invoice_count: number;
  gross_minor: number;
  paid_minor: number;
}
