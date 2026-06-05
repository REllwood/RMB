import { EmptyState } from "@/components/ui/states";

export function CatalogPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Catalog</h1>
      <EmptyState title="Products & services" description="Your catalog will appear here." />
    </div>
  );
}
