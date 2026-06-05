import { EmptyState } from "@/components/ui/states";

export function CustomersPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Customers</h1>
      <EmptyState title="Customers" description="Customer records will appear here." />
    </div>
  );
}
