import { EmptyState } from "@/components/ui/states";

export function DashboardPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Dashboard</h1>
      <EmptyState
        title="Your business at a glance"
        description="Money owed, recent activity, and low-stock items will appear here."
      />
    </div>
  );
}
