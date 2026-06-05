import { EmptyState } from "@/components/ui/states";

export function InvoicesPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Invoices</h1>
      <EmptyState title="Invoices" description="Invoices and payments will appear here." />
    </div>
  );
}
