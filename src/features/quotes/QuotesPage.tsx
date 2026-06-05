import { EmptyState } from "@/components/ui/states";

export function QuotesPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Quotes</h1>
      <EmptyState title="Quotes" description="Estimates and quotes will appear here." />
    </div>
  );
}
