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
  occurred_at: string;
  note: string;
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
  paid_count: number;
  low_stock: LowStockItem[];
}
