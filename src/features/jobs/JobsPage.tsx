import { useState } from "react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { JobMaterialInput, TimeEntryInput } from "@/lib/types";
import { parseMoney, useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
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

export function JobsPage() {
  const [view, setView] = useState<View>({ mode: "list" });
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold tracking-tight">Jobs</h1>
        {view.mode === "list" ? (
          <Button onClick={() => setView({ mode: "create" })}>New job</Button>
        ) : (
          <Button variant="ghost" onClick={() => setView({ mode: "list" })}>← Back to list</Button>
        )}
      </div>
      {view.mode === "list" && <JobList onOpen={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "create" && <JobCreate onCreated={(id) => setView({ mode: "detail", id })} />}
      {view.mode === "detail" && <JobDetailView id={view.id} />}
    </div>
  );
}

function JobList({ onOpen }: { onOpen: (id: number) => void }) {
  const q = useIpcQuery(["jobs"], () => ipc.listJobs());
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data || q.data.length === 0)
    return <EmptyState title="No jobs yet" description="Track time and materials against a job, then invoice it." />;
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Title</TableHead>
          <TableHead>Status</TableHead>
          <TableHead><span className="sr-only">Open</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {q.data.map((j) => (
          <TableRow key={j.id}>
            <TableCell className="font-medium">{j.title}</TableCell>
            <TableCell><Badge variant={j.status === "invoiced" ? "success" : "outline"}>{j.status.replace("_", "-")}</Badge></TableCell>
            <TableCell className="text-right"><Button variant="ghost" size="sm" onClick={() => onOpen(j.id)}>Open</Button></TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
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
      <CardHeader><CardTitle>New job</CardTitle></CardHeader>
      <CardContent className="grid max-w-xl gap-4">
        <Field label="Customer" required>
          {(p) => (
            <Select {...p} value={customerId ?? ""} onChange={(e) => setCustomerId(e.target.value ? Number(e.target.value) : null)}>
              <option value="" disabled>Choose a customer…</option>
              {customersQ.data?.map((c) => (<option key={c.id} value={c.id}>{c.name}</option>))}
            </Select>
          )}
        </Field>
        <Field label="Title" required>
          {(p) => <Input {...p} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Rewire kitchen" />}
        </Field>
        <Field label="Description">
          {(p) => <Input {...p} value={description} onChange={(e) => setDescription(e.target.value)} />}
        </Field>
        <Button onClick={onSave} disabled={customerId === null || !title.trim() || create.isPending}>Create job</Button>
      </CardContent>
    </Card>
  );
}

function JobDetailView({ id }: { id: number }) {
  const money = useMoneyFormat();
  const q = useIpcQuery(["job", id], () => ipc.getJob(id));
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const invalidate: unknown[][] = [["job", id], ["jobs"]];
  const addTime = useIpcMutation((e: TimeEntryInput) => ipc.addTimeEntry(id, e), invalidate);
  const addMaterial = useIpcMutation((m: JobMaterialInput) => ipc.addJobMaterial(id, m), invalidate);
  const delTime = useIpcMutation((t: number) => ipc.deleteTimeEntry(t), invalidate);
  const delMaterial = useIpcMutation((m: number) => ipc.deleteJobMaterial(m), invalidate);
  const invoiceJob = useIpcMutation(() => ipc.invoiceJob(id), [["job", id], ["jobs"], ["invoices"]]);

  const taxes = taxQ.data ?? [];
  const [tMinutes, setTMinutes] = useState("60");
  const [tRate, setTRate] = useState("0.00");
  const [tDesc, setTDesc] = useState("");
  const [tTax, setTTax] = useState(-1);
  const [mDesc, setMDesc] = useState("");
  const [mQty, setMQty] = useState("1");
  const [mPrice, setMPrice] = useState("0.00");
  const [mTax, setMTax] = useState(-1);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Job not found" />;
  const { job, time_entries, materials, labour_total_minor, materials_total_minor } = q.data;
  const taxOf = (idx: number) =>
    idx >= 0 && taxes[idx]
      ? { name: taxes[idx].name, bp: taxes[idx].rate_bp, inc: taxes[idx].inclusive }
      : { name: "No Tax", bp: 0, inc: false };

  function onAddTime() {
    const minutes = Number(tMinutes);
    const rate = parseMoney(tRate) ?? 0;
    if (!Number.isInteger(minutes) || minutes <= 0) return;
    const t = taxOf(tTax);
    addTime.mutate({ date: new Date().toISOString().slice(0, 10), minutes, rate_minor: rate, description: tDesc, tax_rate_name: t.name, tax_rate_bp: t.bp, tax_inclusive: t.inc });
    setTDesc("");
  }
  function onAddMaterial() {
    if (!mDesc.trim()) return;
    const t = taxOf(mTax);
    addMaterial.mutate({ item_id: null, description: mDesc.trim(), quantity: mQty || "1", unit_price_minor: parseMoney(mPrice) ?? 0, tax_rate_name: t.name, tax_rate_bp: t.bp, tax_inclusive: t.inc });
    setMDesc("");
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>{job.title}</CardTitle>
          <Badge variant={job.status === "invoiced" ? "success" : "outline"}>{job.status.replace("_", "-")}</Badge>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Time */}
          <section className="space-y-2">
            <h2 className="font-medium">Time</h2>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Date</TableHead>
                  <TableHead>Description</TableHead>
                  <TableHead className="text-right">Hours</TableHead>
                  <TableHead className="text-right">Rate</TableHead>
                  <TableHead className="text-right">Total</TableHead>
                  <TableHead></TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {time_entries.map((t) => (
                  <TableRow key={t.id}>
                    <TableCell>{t.date}</TableCell>
                    <TableCell>{t.description || "Labour"}</TableCell>
                    <TableCell className="text-right">{(t.minutes / 60).toFixed(2)}</TableCell>
                    <TableCell className="text-right">{money(t.rate_minor)}/h</TableCell>
                    <TableCell className="text-right">{money(Math.round((t.rate_minor * t.minutes) / 60))}</TableCell>
                    <TableCell className="text-right">{!t.invoiced && <Button variant="ghost" size="sm" onClick={() => delTime.mutate(t.id)} aria-label="Remove time entry">✕</Button>}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <div className="flex flex-wrap items-end gap-2">
              <Field label="Minutes">{(p) => <Input {...p} inputMode="numeric" className="w-24" value={tMinutes} onChange={(e) => setTMinutes(e.target.value)} />}</Field>
              <Field label="Rate/hour">{(p) => <Input {...p} inputMode="decimal" className="w-28" value={tRate} onChange={(e) => setTRate(e.target.value)} />}</Field>
              <Field label="Description">{(p) => <Input {...p} className="w-48" value={tDesc} onChange={(e) => setTDesc(e.target.value)} />}</Field>
              <Field label="Tax">{(p) => <Select {...p} className="w-36" value={tTax} onChange={(e) => setTTax(Number(e.target.value))}><option value={-1}>No Tax</option>{taxes.map((t, idx) => <option key={t.id} value={idx}>{t.name}</option>)}</Select>}</Field>
              <Button variant="outline" onClick={onAddTime}>Add time</Button>
            </div>
          </section>

          {/* Materials */}
          <section className="space-y-2">
            <h2 className="font-medium">Materials</h2>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Description</TableHead>
                  <TableHead className="text-right">Qty</TableHead>
                  <TableHead className="text-right">Price</TableHead>
                  <TableHead className="text-right">Total</TableHead>
                  <TableHead></TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {materials.map((m) => (
                  <TableRow key={m.id}>
                    <TableCell>{m.description}</TableCell>
                    <TableCell className="text-right">{m.quantity}</TableCell>
                    <TableCell className="text-right">{money(m.unit_price_minor)}</TableCell>
                    <TableCell className="text-right">{money(Math.round(m.unit_price_minor * (Number(m.quantity) || 0)))}</TableCell>
                    <TableCell className="text-right">{!m.invoiced && <Button variant="ghost" size="sm" onClick={() => delMaterial.mutate(m.id)} aria-label="Remove material">✕</Button>}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <div className="flex flex-wrap items-end gap-2">
              <Field label="Description">{(p) => <Input {...p} className="w-48" value={mDesc} onChange={(e) => setMDesc(e.target.value)} />}</Field>
              <Field label="Qty">{(p) => <Input {...p} inputMode="decimal" className="w-20" value={mQty} onChange={(e) => setMQty(e.target.value)} />}</Field>
              <Field label="Price">{(p) => <Input {...p} inputMode="decimal" className="w-28" value={mPrice} onChange={(e) => setMPrice(e.target.value)} />}</Field>
              <Field label="Tax">{(p) => <Select {...p} className="w-36" value={mTax} onChange={(e) => setMTax(Number(e.target.value))}><option value={-1}>No Tax</option>{taxes.map((t, idx) => <option key={t.id} value={idx}>{t.name}</option>)}</Select>}</Field>
              <Button variant="outline" onClick={onAddMaterial}>Add material</Button>
            </div>
          </section>

          <div className="ml-auto grid max-w-xs gap-1 border-t pt-4 text-sm">
            <Row label="Labour" value={money(labour_total_minor)} />
            <Row label="Materials" value={money(materials_total_minor)} />
            <Row label="Total" value={money(labour_total_minor + materials_total_minor)} strong />
          </div>

          <div className="flex items-center gap-3 border-t pt-4">
            <Button onClick={() => invoiceJob.mutate(undefined)} disabled={invoiceJob.isPending || job.status === "invoiced"}>
              {job.status === "invoiced" ? "Invoiced" : "Create invoice from job"}
            </Button>
            {invoiceJob.isSuccess && <span className="text-sm text-muted-foreground" role="status">Draft invoice created — see the Invoices tab.</span>}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-muted-foreground">{label}</span>
      <span className={strong ? "font-semibold" : ""}>{value}</span>
    </div>
  );
}
