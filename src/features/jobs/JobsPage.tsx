import { useMemo, useState } from "react";
import { Plus, Trash2, X } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { JobMaterialInput, TimeEntryInput } from "@/lib/types";
import { todayLocalISO } from "@/lib/format";
import { minorToInput, parseMoney, useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { PageHeader } from "@/components/ui/page-header";
import { Select } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";

type View = { mode: "list" } | { mode: "create" } | { mode: "detail"; id: number };

const STATUS_VARIANT: Record<string, "success" | "secondary" | "outline" | "warning"> = {
  open: "outline",
  in_progress: "secondary",
  done: "warning",
  invoiced: "success",
};

function JobStatusBadge({ status }: { status: string }) {
  return <Badge variant={STATUS_VARIANT[status] ?? "outline"}>{status.replace("_", "-")}</Badge>;
}

export function JobsPage() {
  const [view, setView] = useState<View>({ mode: "list" });
  return (
    <div className="space-y-6">
      <PageHeader
        title="Jobs"
        description="Track time and materials against the work, then invoice it in one step."
        actions={
          view.mode === "list" ? (
            <Button onClick={() => setView({ mode: "create" })}>
              <Plus className="size-4" /> New job
            </Button>
          ) : (
            <Button variant="ghost" onClick={() => setView({ mode: "list" })}>
              ← Back to list
            </Button>
          )
        }
      />
      {view.mode === "list" && <JobList onOpen={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "create" && <JobCreate onCreated={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "detail" && (
        <JobDetailView id={view.id} onDeleted={() => setView({ mode: "list" })} />
      )}
    </div>
  );
}

function useCustomerNames(): Map<number, string> {
  const q = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  return useMemo(() => new Map((q.data ?? []).map((c) => [c.id, c.name])), [q.data]);
}

function JobList({ onOpen }: { onOpen: (id: number) => void }) {
  const names = useCustomerNames();
  const q = useIpcQuery(["jobs"], () => ipc.listJobs());
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data || q.data.length === 0)
    return <EmptyState title="No jobs yet" description="Track time and materials against a job, then invoice it." />;
  return (
    <Card className="overflow-hidden py-0">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="pl-4">Title</TableHead>
            <TableHead>Customer</TableHead>
            <TableHead>Created</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>
              <span className="sr-only">Open</span>
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {q.data.map((j) => (
            <TableRow key={j.id} className="cursor-pointer" onClick={() => onOpen(j.id)}>
              <TableCell className="pl-4 font-medium">{j.title}</TableCell>
              <TableCell>{names.get(j.customer_id) ?? "—"}</TableCell>
              <TableCell className="text-muted-foreground">{j.created_at.slice(0, 10)}</TableCell>
              <TableCell>
                <JobStatusBadge status={j.status} />
              </TableCell>
              <TableCell className="pr-4 text-right">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    onOpen(j.id);
                  }}
                >
                  Open
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Card>
  );
}

function JobCreate({ onCreated }: { onCreated: (id: number) => void }) {
  const customersQ = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  const create = useIpcMutation(
    (v: { customer_id: number; title: string; description: string }) => ipc.createJob(v),
    [["jobs"]],
  );
  const [customerId, setCustomerId] = useState<number | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");

  async function onSave() {
    if (customerId === null || !title.trim()) return;
    const id = await create.mutateAsync({ customer_id: customerId, title: title.trim(), description });
    onCreated(id);
  }

  if (customersQ.data && customersQ.data.length === 0)
    return <EmptyState title="Add a customer first" description="Jobs belong to a customer." />;

  return (
    <Card>
      <CardHeader>
        <CardTitle>New job</CardTitle>
      </CardHeader>
      <CardContent className="grid max-w-xl gap-4">
        <Field label="Customer" required>
          {(p) => (
            <Select {...p} value={customerId ?? ""} onChange={(e) => setCustomerId(e.target.value ? Number(e.target.value) : null)}>
              <option value="" disabled>
                Choose a customer…
              </option>
              {customersQ.data?.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </Select>
          )}
        </Field>
        <Field label="Title" required>
          {(p) => <Input {...p} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Rewire kitchen" />}
        </Field>
        <Field label="Description">
          {(p) => <Input {...p} value={description} onChange={(e) => setDescription(e.target.value)} />}
        </Field>
        <div>
          <Button onClick={onSave} disabled={customerId === null || !title.trim() || create.isPending}>
            Create job
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function JobDetailView({ id, onDeleted }: { id: number; onDeleted: () => void }) {
  const money = useMoneyFormat();
  const names = useCustomerNames();
  const q = useIpcQuery(["job", id], () => ipc.getJob(id));
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const itemsQ = useIpcQuery(["items", ""], () => ipc.listItems());
  const invalidate: unknown[][] = [["job", id], ["jobs"]];
  const addTime = useIpcMutation((e: TimeEntryInput) => ipc.addTimeEntry(id, e), invalidate);
  const addMaterial = useIpcMutation((m: JobMaterialInput) => ipc.addJobMaterial(id, m), invalidate);
  const delTime = useIpcMutation((t: number) => ipc.deleteTimeEntry(t), invalidate);
  const delMaterial = useIpcMutation((m: number) => ipc.deleteJobMaterial(m), invalidate);
  const setStatus = useIpcMutation((s: string) => ipc.setJobStatus(id, s), invalidate);
  const deleteJob = useIpcMutation(() => ipc.deleteJob(id), [["jobs"]], {
    successMessage: "Job deleted",
  });
  const invoiceJob = useIpcMutation(() => ipc.invoiceJob(id), [...invalidate, ["invoices"]], {
    successMessage: "Draft invoice created — see Invoices",
  });

  const taxes = taxQ.data ?? [];
  const items = itemsQ.data ?? [];
  const [tDate, setTDate] = useState(todayLocalISO());
  const [tMinutes, setTMinutes] = useState("60");
  const [tRate, setTRate] = useState("0.00");
  const [tDesc, setTDesc] = useState("");
  const [tTax, setTTax] = useState(-1);
  const [mItem, setMItem] = useState<number | null>(null);
  const [mDesc, setMDesc] = useState("");
  const [mQty, setMQty] = useState("1");
  const [mPrice, setMPrice] = useState("0.00");
  const [mTax, setMTax] = useState(-1);
  const [confirm, setConfirm] = useState<null | "invoice" | "delete">(null);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Job not found" />;
  const { job, time_entries, materials, labour_total_minor, materials_total_minor } = q.data;
  const taxOf = (idx: number) =>
    idx >= 0 && taxes[idx]
      ? { name: taxes[idx].name, bp: taxes[idx].rate_bp, inc: taxes[idx].inclusive }
      : { name: "No Tax", bp: 0, inc: false };
  const billable = job.status !== "invoiced";

  function pickMaterialItem(value: string) {
    if (!value) {
      setMItem(null);
      return;
    }
    const it = items.find((x) => x.id === Number(value));
    if (!it) return;
    setMItem(it.id);
    setMDesc(it.name);
    setMPrice(minorToInput(it.default_price_minor));
    const idx = taxes.findIndex((t) => t.id === it.default_tax_rate_id);
    setMTax(idx);
  }

  function onAddTime() {
    const minutes = Number(tMinutes);
    const rate = parseMoney(tRate) ?? 0;
    if (!Number.isInteger(minutes) || minutes <= 0 || !tDate) return;
    const t = taxOf(tTax);
    addTime.mutate({
      date: tDate,
      minutes,
      rate_minor: rate,
      description: tDesc,
      tax_rate_name: t.name,
      tax_rate_bp: t.bp,
      tax_inclusive: t.inc,
    });
    setTDesc("");
  }
  function onAddMaterial() {
    if (!mDesc.trim()) return;
    const t = taxOf(mTax);
    addMaterial.mutate({
      item_id: mItem,
      description: mDesc.trim(),
      quantity: mQty || "1",
      unit_price_minor: parseMoney(mPrice) ?? 0,
      tax_rate_name: t.name,
      tax_rate_bp: t.bp,
      tax_inclusive: t.inc,
    });
    setMItem(null);
    setMDesc("");
    setMQty("1");
    setMPrice("0.00");
  }

  // Same 2dp-hours rounding the backend bills with — rows always reconcile to the totals.
  const rowLabour = (rate: number, minutes: number) =>
    Math.round(rate * (Math.round((minutes / 60) * 100) / 100));

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex-row items-start justify-between">
          <div className="space-y-1">
            <CardTitle>{job.title}</CardTitle>
            <p className="text-sm text-muted-foreground">
              {names.get(job.customer_id) ?? "—"}
              {job.description && ` · ${job.description}`}
            </p>
          </div>
          <div className="flex items-center gap-2">
            {billable ? (
              <>
                <label htmlFor="job-status" className="sr-only">
                  Job status
                </label>
                <Select
                  id="job-status"
                  className="w-36"
                  value={job.status}
                  onChange={(e) => setStatus.mutate(e.target.value)}
                >
                  <option value="open">Open</option>
                  <option value="in_progress">In progress</option>
                  <option value="done">Done</option>
                </Select>
              </>
            ) : (
              <JobStatusBadge status={job.status} />
            )}
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Time */}
          <section className="space-y-2">
            <h2 className="font-medium">Time</h2>
            {time_entries.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Date</TableHead>
                    <TableHead>Description</TableHead>
                    <TableHead className="text-right">Hours</TableHead>
                    <TableHead className="text-right">Rate</TableHead>
                    <TableHead className="text-right">Total</TableHead>
                    <TableHead>
                      <span className="sr-only">Actions</span>
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {time_entries.map((t) => (
                    <TableRow key={t.id}>
                      <TableCell>{t.date}</TableCell>
                      <TableCell>{t.description || "Labour"}</TableCell>
                      <TableCell className="text-right tabular-nums">{(Math.round((t.minutes / 60) * 100) / 100).toFixed(2)}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(t.rate_minor)}/h</TableCell>
                      <TableCell className="text-right tabular-nums">{money(rowLabour(t.rate_minor, t.minutes))}</TableCell>
                      <TableCell className="text-right">
                        {!t.invoiced && (
                          <Button variant="ghost" size="icon" onClick={() => delTime.mutate(t.id)} aria-label="Remove time entry">
                            <X className="size-4" />
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
            {billable && (
              <div className="flex flex-wrap items-end gap-2">
                <Field label="Date">
                  {(p) => <Input {...p} type="date" className="w-40" value={tDate} onChange={(e) => setTDate(e.target.value)} />}
                </Field>
                <Field label="Minutes">
                  {(p) => <Input {...p} inputMode="numeric" className="w-24" value={tMinutes} onChange={(e) => setTMinutes(e.target.value)} />}
                </Field>
                <Field label="Rate/hour">
                  {(p) => <Input {...p} inputMode="decimal" className="w-28" value={tRate} onChange={(e) => setTRate(e.target.value)} />}
                </Field>
                <Field label="Description">
                  {(p) => <Input {...p} className="w-48" value={tDesc} onChange={(e) => setTDesc(e.target.value)} />}
                </Field>
                <Field label="Tax">
                  {(p) => (
                    <Select {...p} className="w-36" value={tTax} onChange={(e) => setTTax(Number(e.target.value))}>
                      <option value={-1}>No Tax</option>
                      {taxes.map((t, idx) => (
                        <option key={t.id} value={idx}>
                          {t.name}
                        </option>
                      ))}
                    </Select>
                  )}
                </Field>
                <Button variant="outline" onClick={onAddTime}>
                  <Plus className="size-4" /> Add time
                </Button>
              </div>
            )}
          </section>

          {/* Materials */}
          <section className="space-y-2">
            <h2 className="font-medium">Materials</h2>
            {materials.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Description</TableHead>
                    <TableHead className="text-right">Qty</TableHead>
                    <TableHead className="text-right">Price</TableHead>
                    <TableHead className="text-right">Total</TableHead>
                    <TableHead>
                      <span className="sr-only">Actions</span>
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {materials.map((m) => (
                    <TableRow key={m.id}>
                      <TableCell>{m.description}</TableCell>
                      <TableCell className="text-right tabular-nums">{m.quantity}</TableCell>
                      <TableCell className="text-right tabular-nums">{money(m.unit_price_minor)}</TableCell>
                      <TableCell className="text-right tabular-nums">
                        {money(Math.round(m.unit_price_minor * (Number(m.quantity) || 0)))}
                      </TableCell>
                      <TableCell className="text-right">
                        {!m.invoiced && (
                          <Button variant="ghost" size="icon" onClick={() => delMaterial.mutate(m.id)} aria-label="Remove material">
                            <X className="size-4" />
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
            {billable && (
              <div className="flex flex-wrap items-end gap-2">
                <Field label="Item" hint="Pick from the catalog to track stock">
                  {(p) => (
                    <Select {...p} className="w-44" value={mItem ?? ""} onChange={(e) => pickMaterialItem(e.target.value)}>
                      <option value="">Custom</option>
                      {items.map((it) => (
                        <option key={it.id} value={it.id}>
                          {it.tracked ? `${it.name} (${it.qty_on_hand} in stock)` : it.name}
                        </option>
                      ))}
                    </Select>
                  )}
                </Field>
                <Field label="Description">
                  {(p) => <Input {...p} className="w-44" value={mDesc} onChange={(e) => setMDesc(e.target.value)} />}
                </Field>
                <Field label="Qty">
                  {(p) => <Input {...p} inputMode="decimal" className="w-20" value={mQty} onChange={(e) => setMQty(e.target.value)} />}
                </Field>
                <Field label="Price">
                  {(p) => <Input {...p} inputMode="decimal" className="w-28" value={mPrice} onChange={(e) => setMPrice(e.target.value)} />}
                </Field>
                <Field label="Tax">
                  {(p) => (
                    <Select {...p} className="w-36" value={mTax} onChange={(e) => setMTax(Number(e.target.value))}>
                      <option value={-1}>No Tax</option>
                      {taxes.map((t, idx) => (
                        <option key={t.id} value={idx}>
                          {t.name}
                        </option>
                      ))}
                    </Select>
                  )}
                </Field>
                <Button variant="outline" onClick={onAddMaterial}>
                  <Plus className="size-4" /> Add material
                </Button>
              </div>
            )}
          </section>

          <div className="ml-auto grid max-w-xs gap-1 border-t pt-4 text-sm">
            <Row label="Labour" value={money(labour_total_minor)} />
            <Row label="Materials" value={money(materials_total_minor)} />
            <Row label="Total" value={money(labour_total_minor + materials_total_minor)} strong />
          </div>

          <div className="flex flex-wrap items-center gap-3 border-t pt-4">
            <Button onClick={() => setConfirm("invoice")} disabled={invoiceJob.isPending || !billable}>
              {billable ? "Create invoice from job" : "Invoiced"}
            </Button>
            <Button variant="ghost" onClick={() => setConfirm("delete")}>
              <Trash2 className="size-4" /> Delete job
            </Button>
          </div>
        </CardContent>
      </Card>

      <ConfirmDialog
        open={confirm === "invoice"}
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          await invoiceJob.mutateAsync(undefined);
          setConfirm(null);
        }}
        title="Invoice this job?"
        description="Un-invoiced time and materials become a draft invoice and are marked billed. You can still review the draft before issuing."
        confirmLabel="Create draft invoice"
        pending={invoiceJob.isPending}
      />
      <ConfirmDialog
        open={confirm === "delete"}
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          await deleteJob.mutateAsync(undefined);
          onDeleted();
        }}
        title="Delete this job?"
        description="The job is removed from your lists. Any invoice already created from it is unaffected."
        confirmLabel="Delete job"
        destructive
        pending={deleteJob.isPending}
      />
    </div>
  );
}

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-muted-foreground">{label}</span>
      <span className={strong ? "font-semibold tabular-nums" : "tabular-nums"}>{value}</span>
    </div>
  );
}
