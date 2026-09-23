import { useMemo, useState } from "react";
import { LoaderCircle, Plus, Trash2, X } from "lucide-react";

import { useView } from "@/app/nav";
import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { JobMaterialInput, TimeEntryInput } from "@/lib/types";
import { todayLocalISO } from "@/lib/format";
import {
  labourAmountMinor,
  lineAmountMinor,
  minorToInput,
  parseMoney,
  parseWholeNumber,
  useMoneyFormat,
} from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { PageHeader } from "@/components/ui/page-header";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
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
import {
  defaultTax,
  isWholeQuantity,
  NO_TAX,
  normaliseQuantity,
  taxKey,
  taxLabel,
  taxOptions,
  toChoice,
  type TaxChoice,
} from "@/features/shared/lines";

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
  const [view, setView] = useView<View>({ mode: "list" });
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
    return (
      <EmptyState
        title="No jobs yet"
        description="Track time and materials against a job, then invoice it."
      />
    );
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
    try {
      const id = await create.mutateAsync({
        customer_id: customerId,
        title: title.trim(),
        description,
      });
      onCreated(id);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  if (customersQ.isLoading) return <Loading label="Loading customers…" />;
  if (customersQ.error)
    return <ErrorState error={customersQ.error} onRetry={() => customersQ.refetch()} />;
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
            <Select
              {...p}
              value={customerId ?? ""}
              onChange={(e) => setCustomerId(e.target.value ? Number(e.target.value) : null)}
            >
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
          {(p) => (
            <Input
              {...p}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="e.g. Rewire kitchen"
            />
          )}
        </Field>
        <Field label="Description">
          {(p) => (
            <Textarea {...p} value={description} onChange={(e) => setDescription(e.target.value)} />
          )}
        </Field>
        <div>
          <Button
            onClick={onSave}
            disabled={customerId === null || !title.trim()}
            loading={create.isPending}
            loadingLabel="Creating…"
          >
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
  const settingsQ = useIpcQuery(["settings"], () => ipc.getSettings());
  const invalidate: unknown[][] = [["job", id], ["jobs"]];
  const addTime = useIpcMutation((e: TimeEntryInput) => ipc.addTimeEntry(id, e), invalidate);
  const addMaterial = useIpcMutation(
    (m: JobMaterialInput) => ipc.addJobMaterial(id, m),
    invalidate,
  );
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
  // null until edited: the rate starts from the job's last entry, the tax from the business default.
  const [tRate, setTRate] = useState<string | null>(null);
  const [tDesc, setTDesc] = useState("");
  const [tTax, setTTax] = useState<string | null>(null);
  const [mItem, setMItem] = useState<number | null>(null);
  const [mDesc, setMDesc] = useState("");
  const [mQty, setMQty] = useState("1");
  const [mPrice, setMPrice] = useState("0.00");
  const [mTax, setMTax] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<null | "invoice" | "delete">(null);
  const [removing, setRemoving] = useState<null | {
    kind: "time" | "material";
    id: number;
    label: string;
  }>(null);

  if (q.isLoading || taxQ.isLoading || itemsQ.isLoading || settingsQ.isLoading) return <Loading />;
  const loadError = q.error ?? taxQ.error ?? itemsQ.error ?? settingsQ.error;
  if (loadError)
    return (
      <ErrorState
        error={loadError}
        onRetry={() =>
          Promise.all([q.refetch(), taxQ.refetch(), itemsQ.refetch(), settingsQ.refetch()])
        }
      />
    );
  if (!q.data) return <EmptyState title="Job not found" />;
  const { job, time_entries, materials } = q.data;
  const options = taxOptions(taxes);
  const defaultKey = taxKey(defaultTax(taxes, settingsQ.data?.default_tax_rate_id));
  const taxOf = (key: string | null): TaxChoice =>
    options.find((o) => taxKey(o) === (key ?? defaultKey)) ?? NO_TAX;
  const billable = job.status !== "invoiced";
  const hasUnbilled =
    time_entries.some((entry) => !entry.invoiced) ||
    materials.some((material) => !material.invoiced);
  const lastRate = time_entries[time_entries.length - 1]?.rate_minor;
  const rateText = tRate ?? (lastRate === undefined ? "" : minorToInput(lastRate));
  const minutes = parseWholeNumber(tMinutes, { min: 1, max: 1_000_000 });
  const hourlyRate = parseMoney(rateText);
  const timeValid = Boolean(tDate) && minutes !== null && hourlyRate !== null && hourlyRate >= 0;
  const materialQuantity = normaliseQuantity(mQty);
  const materialPrice = parseMoney(mPrice);
  const selectedMaterial = mItem === null ? undefined : items.find((item) => item.id === mItem);
  const materialValid =
    Boolean(mDesc.trim()) &&
    materialQuantity !== null &&
    materialPrice !== null &&
    (!selectedMaterial?.tracked || isWholeQuantity(materialQuantity));

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
    const tax = taxes.find((t) => t.id === it.default_tax_rate_id);
    setMTax(taxKey(tax ? toChoice(tax) : NO_TAX));
  }

  async function onAddTime() {
    if (!timeValid || hourlyRate === null || minutes === null) return;
    const t = taxOf(tTax);
    try {
      await addTime.mutateAsync({
        date: tDate,
        minutes,
        rate_minor: hourlyRate,
        description: tDesc,
        tax_rate_name: t.name,
        tax_rate_bp: t.bp,
        tax_inclusive: t.inclusive,
      });
      setTDesc("");
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }
  async function onAddMaterial() {
    if (!materialValid || materialQuantity === null || materialPrice === null) return;
    const t = taxOf(mTax);
    try {
      await addMaterial.mutateAsync({
        item_id: mItem,
        description: mDesc.trim(),
        quantity: materialQuantity,
        unit_price_minor: materialPrice,
        tax_rate_name: t.name,
        tax_rate_bp: t.bp,
        tax_inclusive: t.inclusive,
      });
      setMItem(null);
      setMTax(null);
      setMDesc("");
      setMQty("1");
      setMPrice("0.00");
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  // Bill from exact minutes; rounding displayed hours must never change the amount charged.
  const rowLabour = labourAmountMinor;

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
                  disabled={setStatus.isPending}
                  onChange={(e) => setStatus.mutate(e.target.value)}
                >
                  <option value="open">Open</option>
                  <option value="in_progress">In progress</option>
                  <option value="done">Done</option>
                </Select>
                {setStatus.isPending && (
                  <span
                    role="status"
                    className="flex items-center gap-2 text-sm text-muted-foreground"
                  >
                    <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
                    Updating…
                  </span>
                )}
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
                      <TableCell className="text-right tabular-nums">
                        {(Math.round((t.minutes / 60) * 100) / 100).toFixed(2)}
                      </TableCell>
                      <TableCell className="text-right tabular-nums">
                        {money(t.rate_minor)}/h
                      </TableCell>
                      <TableCell className="text-right tabular-nums">
                        {money(rowLabour(t.rate_minor, t.minutes))}
                      </TableCell>
                      <TableCell className="text-right">
                        {!t.invoiced && (
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() =>
                              setRemoving({
                                kind: "time",
                                id: t.id,
                                label: `${t.date}, ${(t.minutes / 60).toFixed(2)} h`,
                              })
                            }
                            aria-label={`Remove time entry ${t.date}, ${(t.minutes / 60).toFixed(2)} hours`}
                          >
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
                  {(p) => (
                    <Input
                      {...p}
                      type="date"
                      className="w-40"
                      value={tDate}
                      onChange={(e) => setTDate(e.target.value)}
                    />
                  )}
                </Field>
                <Field
                  label="Minutes"
                  error={
                    tMinutes && minutes === null
                      ? "Enter whole minutes from 1 to 1,000,000"
                      : undefined
                  }
                  hint={minutes !== null ? `${(minutes / 60).toFixed(2)} h` : undefined}
                >
                  {(p) => (
                    <Input
                      {...p}
                      inputMode="numeric"
                      className="w-24"
                      value={tMinutes}
                      onChange={(e) => setTMinutes(e.target.value)}
                    />
                  )}
                </Field>
                <Field
                  label="Rate/hour"
                  required
                  error={
                    rateText && (hourlyRate === null || hourlyRate < 0)
                      ? "Enter a valid amount with at most two decimal places"
                      : undefined
                  }
                  hint={hourlyRate === 0 ? "This time will be billed at zero" : undefined}
                >
                  {(p) => (
                    <Input
                      {...p}
                      inputMode="decimal"
                      className="w-28"
                      value={rateText}
                      onChange={(e) => setTRate(e.target.value)}
                    />
                  )}
                </Field>
                <Field label="Description">
                  {(p) => (
                    <Input
                      {...p}
                      className="w-48"
                      value={tDesc}
                      onChange={(e) => setTDesc(e.target.value)}
                    />
                  )}
                </Field>
                <Field label="Tax">
                  {(p) => (
                    <Select
                      {...p}
                      className="w-44"
                      value={tTax ?? defaultKey}
                      onChange={(e) => setTTax(e.target.value)}
                    >
                      {options.map((o) => (
                        <option key={taxKey(o)} value={taxKey(o)}>
                          {taxLabel(o)}
                        </option>
                      ))}
                    </Select>
                  )}
                </Field>
                <Button
                  variant="outline"
                  onClick={onAddTime}
                  disabled={!timeValid}
                  loading={addTime.isPending}
                  loadingLabel="Adding…"
                >
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
                      <TableCell className="text-right tabular-nums">
                        {money(m.unit_price_minor)}
                      </TableCell>
                      <TableCell className="text-right tabular-nums">
                        {money(
                          lineAmountMinor(m.unit_price_minor, normaliseQuantity(m.quantity) ?? "0"),
                        )}
                      </TableCell>
                      <TableCell className="text-right">
                        {!m.invoiced && (
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() =>
                              setRemoving({
                                kind: "material",
                                id: m.id,
                                label: `${m.quantity} × ${m.description}`,
                              })
                            }
                            aria-label={`Remove material ${m.description}`}
                          >
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
                    <Select
                      {...p}
                      className="w-44"
                      value={mItem ?? ""}
                      onChange={(e) => pickMaterialItem(e.target.value)}
                    >
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
                  {(p) => (
                    <Input
                      {...p}
                      className="w-44"
                      value={mDesc}
                      onChange={(e) => setMDesc(e.target.value)}
                    />
                  )}
                </Field>
                <Field
                  label="Qty"
                  error={
                    mQty && materialQuantity === null
                      ? "Enter a quantity above zero"
                      : selectedMaterial?.tracked &&
                          materialQuantity !== null &&
                          !isWholeQuantity(materialQuantity)
                        ? "Tracked products need a whole quantity"
                        : undefined
                  }
                >
                  {(p) => (
                    <Input
                      {...p}
                      inputMode="decimal"
                      className="w-20"
                      value={mQty}
                      onChange={(e) => setMQty(e.target.value)}
                    />
                  )}
                </Field>
                <Field
                  label="Price"
                  error={
                    mPrice && materialPrice === null
                      ? "Enter a valid amount with at most two decimal places"
                      : undefined
                  }
                >
                  {(p) => (
                    <Input
                      {...p}
                      inputMode="decimal"
                      className="w-28"
                      value={mPrice}
                      onChange={(e) => setMPrice(e.target.value)}
                    />
                  )}
                </Field>
                <Field label="Tax">
                  {(p) => (
                    <Select
                      {...p}
                      className="w-44"
                      value={mTax ?? defaultKey}
                      onChange={(e) => setMTax(e.target.value)}
                    >
                      {options.map((o) => (
                        <option key={taxKey(o)} value={taxKey(o)}>
                          {taxLabel(o)}
                        </option>
                      ))}
                    </Select>
                  )}
                </Field>
                <Button
                  variant="outline"
                  onClick={onAddMaterial}
                  disabled={!materialValid}
                  loading={addMaterial.isPending}
                  loadingLabel="Adding…"
                >
                  <Plus className="size-4" /> Add material
                </Button>
              </div>
            )}
          </section>

          <div className="ml-auto grid max-w-xs gap-1 border-t pt-4 text-sm">
            <Row label="Subtotal" value={money(q.data.subtotal_minor)} />
            <Row label="Tax" value={money(q.data.tax_minor)} />
            <Row label="Total" value={money(q.data.total_minor)} strong />
            {billable && (
              <Row label="Still to invoice" value={money(q.data.unbilled_total_minor)} />
            )}
          </div>

          <div className="flex flex-wrap items-center gap-3 border-t pt-4">
            <Button
              onClick={() => setConfirm("invoice")}
              disabled={!billable || !hasUnbilled}
              loading={invoiceJob.isPending}
              loadingLabel="Creating invoice…"
            >
              {billable ? "Create invoice from job" : "Invoiced"}
            </Button>
            {billable && time_entries.length === 0 && materials.length === 0 && (
              <Button variant="ghost" onClick={() => setConfirm("delete")}>
                <Trash2 className="size-4" /> Delete job
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      <ConfirmDialog
        open={removing !== null}
        onClose={() => setRemoving(null)}
        onConfirm={async () => {
          if (!removing) return;
          try {
            if (removing.kind === "time") await delTime.mutateAsync(removing.id);
            else await delMaterial.mutateAsync(removing.id);
            setRemoving(null);
          } catch {
            // Keep the dialog open; the error is shown above it.
          }
        }}
        title={removing?.kind === "time" ? "Remove this time entry?" : "Remove this material?"}
        description={removing ? `${removing.label} will be removed from the job.` : undefined}
        confirmLabel="Remove"
        destructive
        pending={delTime.isPending || delMaterial.isPending}
      />
      <ConfirmDialog
        open={confirm === "invoice"}
        onClose={() => setConfirm(null)}
        onConfirm={async () => {
          try {
            await invoiceJob.mutateAsync(undefined);
            setConfirm(null);
          } catch {
            // Keep the dialog open so the backend explanation remains visible.
          }
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
          try {
            await deleteJob.mutateAsync(undefined);
            onDeleted();
          } catch {
            // Keep the dialog open so the backend explanation remains visible.
          }
        }}
        title="Delete this job?"
        description="Only an empty, unbilled job can be deleted. Invoiced jobs remain part of the financial record."
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
