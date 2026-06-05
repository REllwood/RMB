import type { ComponentType } from "react";
import {
  FileText,
  Hammer,
  LayoutDashboard,
  Package,
  Receipt,
  Settings,
  Users,
} from "lucide-react";

export type SectionId =
  | "dashboard"
  | "customers"
  | "catalog"
  | "quotes"
  | "jobs"
  | "invoices"
  | "settings";

type IconType = ComponentType<{ className?: string }>;

export const SECTIONS: ReadonlyArray<{ id: SectionId; label: string; icon: IconType }> = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "customers", label: "Customers", icon: Users },
  { id: "catalog", label: "Catalog", icon: Package },
  { id: "quotes", label: "Quotes", icon: FileText },
  { id: "jobs", label: "Jobs", icon: Hammer },
  { id: "invoices", label: "Invoices", icon: Receipt },
  { id: "settings", label: "Settings", icon: Settings },
];
