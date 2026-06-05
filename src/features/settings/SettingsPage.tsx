import { EmptyState } from "@/components/ui/states";

export function SettingsPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
      <EmptyState title="Settings" description="Business details, tax, and backup will appear here." />
    </div>
  );
}
