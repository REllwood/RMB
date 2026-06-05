import { EmptyState } from "@/components/ui/states";

export function JobsPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Jobs</h1>
      <EmptyState title="Jobs" description="Jobs, time, and materials will appear here." />
    </div>
  );
}
