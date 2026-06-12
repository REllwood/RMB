import type { InvoiceRow } from "@/lib/types";
import { todayLocalISO } from "@/lib/format";

export type BadgeVariant =
  | "default"
  | "secondary"
  | "success"
  | "warning"
  | "outline"
  | "destructive";

/** Display status, deriving `overdue` for unpaid invoices past their due date (local time). */
export function invoiceStatus(inv: Pick<InvoiceRow, "status" | "due_date">): {
  label: string;
  variant: BadgeVariant;
} {
  const unpaid = inv.status === "issued" || inv.status === "part_paid";
  if (unpaid && inv.due_date && inv.due_date < todayLocalISO())
    return { label: "overdue", variant: "destructive" };
  const map: Record<string, { label: string; variant: BadgeVariant }> = {
    draft: { label: "draft", variant: "outline" },
    issued: { label: "issued", variant: "secondary" },
    part_paid: { label: "part-paid", variant: "warning" },
    paid: { label: "paid", variant: "success" },
    void: { label: "void", variant: "destructive" },
  };
  return map[inv.status] ?? { label: inv.status, variant: "outline" };
}
