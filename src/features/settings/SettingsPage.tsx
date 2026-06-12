import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { Settings } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
import { ErrorState, Loading } from "@/components/ui/states";

const CURRENCIES = ["USD", "GBP", "EUR", "AUD", "NZD", "CAD"];
const COUNTRIES: [string, string][] = [
  ["UK", "United Kingdom — VAT"],
  ["AU", "Australia — GST"],
  ["NZ", "New Zealand — GST"],
  ["CA", "Canada — GST"],
  ["US", "United States"],
];

export function SettingsPage() {
  const settingsQ = useIpcQuery(["settings"], () => ipc.getSettings());
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const [form, setForm] = useState<Settings | null>(null);
  const [status, setStatus] = useState("");

  useEffect(() => {
    if (settingsQ.data) setForm((f) => f ?? settingsQ.data);
  }, [settingsQ.data]);

  const saveMut = useIpcMutation((v: Settings) => ipc.updateSettings(v), [["settings"]]);
  const presetMut = useIpcMutation((c: string) => ipc.applyTaxPreset(c), [["tax-rates"]]);
  const addTaxMut = useIpcMutation(
    (r: { name: string; bp: number; inc: boolean }) => ipc.createTaxRate(r.name, r.bp, r.inc),
    [["tax-rates"]],
  );
  const archiveMut = useIpcMutation((id: number) => ipc.archiveTaxRate(id), [["tax-rates"]]);

  const [taxName, setTaxName] = useState("");
  const [taxPct, setTaxPct] = useState("");
  const [confirmRestore, setConfirmRestore] = useState<string | null>(null);

  if (settingsQ.isLoading || !form) return <Loading label="Loading settings…" />;
  if (settingsQ.error) return <ErrorState error={settingsQ.error} onRetry={() => settingsQ.refetch()} />;

  function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    setForm((f) => (f ? { ...f, [key]: value } : f));
  }

  async function onSave() {
    if (!form) return;
    setStatus("");
    await saveMut.mutateAsync(form);
    setStatus("Saved.");
  }

  async function onBackup() {
    const path = await save({ defaultPath: "rmb-backup.sqlite", filters: [{ name: "SQLite", extensions: ["sqlite"] }] });
    if (path) {
      await ipc.backupDatabase(path);
      setStatus("Backup written.");
    }
  }

  async function onRestore() {
    const path = await open({ multiple: false, filters: [{ name: "SQLite", extensions: ["sqlite"] }] });
    if (typeof path === "string") setConfirmRestore(path);
  }

  function onAddTax() {
    const pct = Number(taxPct);
    if (!taxName.trim() || !Number.isFinite(pct)) return;
    void addTaxMut.mutateAsync({ name: taxName.trim(), bp: Math.round(pct * 100), inc: form?.prices_tax_inclusive ?? false });
    setTaxName("");
    setTaxPct("");
  }

  return (
    <div className="space-y-6">
      <PageHeader title="Settings" description="Your business details, tax setup, and data." />

      <Card>
        <CardHeader>
          <CardTitle>Business</CardTitle>
          <CardDescription>Appears on your quotes and invoices.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <Field label="Business name">
            {(p) => <Input {...p} value={form.business_name} onChange={(e) => set("business_name", e.target.value)} />}
          </Field>
          <Field label="Email">
            {(p) => <Input {...p} type="email" value={form.email} onChange={(e) => set("email", e.target.value)} />}
          </Field>
          <Field label="Phone">
            {(p) => <Input {...p} value={form.phone} onChange={(e) => set("phone", e.target.value)} />}
          </Field>
          <Field label="Currency">
            {(p) => (
              <Select {...p} value={form.currency} onChange={(e) => set("currency", e.target.value)}>
                {CURRENCIES.map((c) => (
                  <option key={c} value={c}>{c}</option>
                ))}
              </Select>
            )}
          </Field>
          <Field label="Address">
            {(p) => <Input {...p} value={form.address} onChange={(e) => set("address", e.target.value)} />}
          </Field>
          <Field label="Tax label" hint="e.g. VAT, GST, Sales Tax">
            {(p) => <Input {...p} value={form.tax_label} onChange={(e) => set("tax_label", e.target.value)} />}
          </Field>
          <Field label="Tax number">
            {(p) => <Input {...p} value={form.tax_number} onChange={(e) => set("tax_number", e.target.value)} />}
          </Field>
          <Field label="Invoice number prefix">
            {(p) => <Input {...p} value={form.invoice_prefix} onChange={(e) => set("invoice_prefix", e.target.value)} />}
          </Field>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={form.prices_tax_inclusive}
              onChange={(e) => set("prices_tax_inclusive", e.target.checked)}
            />
            Prices include tax by default
          </label>
        </CardContent>
        <CardContent className="flex items-center gap-3 border-t pt-4">
          <Button onClick={onSave} disabled={saveMut.isPending}>
            {saveMut.isPending ? "Saving…" : "Save settings"}
          </Button>
          {status && <span role="status" className="text-sm text-muted-foreground">{status}</span>}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Tax rates</CardTitle>
          <CardDescription>Configure your own, or seed a country preset.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-end gap-2">
            <Field label="Seed a preset">
              {(p) => (
                <Select
                  {...p}
                  defaultValue=""
                  onChange={(e) => e.target.value && presetMut.mutate(e.target.value)}
                  className="w-56"
                >
                  <option value="" disabled>Choose a country…</option>
                  {COUNTRIES.map(([code, label]) => (
                    <option key={code} value={code}>{label}</option>
                  ))}
                </Select>
              )}
            </Field>
          </div>

          {taxQ.data && taxQ.data.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Rate</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead><span className="sr-only">Actions</span></TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {taxQ.data.map((r) => (
                  <TableRow key={r.id}>
                    <TableCell>{r.name}</TableCell>
                    <TableCell>{(r.rate_bp / 100).toFixed(2)}%</TableCell>
                    <TableCell>
                      <Badge variant={r.inclusive ? "secondary" : "outline"}>
                        {r.inclusive ? "Inclusive" : "Exclusive"}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-right">
                      <Button variant="ghost" size="sm" onClick={() => archiveMut.mutate(r.id)}>
                        Archive
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}

          <div className="flex flex-wrap items-end gap-2 border-t pt-4">
            <Field label="New rate name">
              {(p) => <Input {...p} className="w-44" value={taxName} onChange={(e) => setTaxName(e.target.value)} placeholder="VAT 20%" />}
            </Field>
            <Field label="Rate %">
              {(p) => <Input {...p} className="w-24" inputMode="decimal" value={taxPct} onChange={(e) => setTaxPct(e.target.value)} placeholder="20" />}
            </Field>
            <Button variant="outline" onClick={onAddTax}>Add rate</Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Backup &amp; restore</CardTitle>
          <CardDescription>Your data lives in a single local file. Back it up regularly.</CardDescription>
        </CardHeader>
        <CardContent className="flex gap-3">
          <Button variant="outline" onClick={onBackup}>Back up…</Button>
          <Button variant="outline" onClick={onRestore}>Restore…</Button>
        </CardContent>
      </Card>

      <ConfirmDialog
        open={confirmRestore !== null}
        onClose={() => setConfirmRestore(null)}
        onConfirm={async () => {
          if (confirmRestore) await ipc.restoreDatabase(confirmRestore);
          setConfirmRestore(null);
        }}
        title="Restore from backup?"
        description="All current data is replaced by the backup and the app restarts. Back up first if unsure."
        confirmLabel="Restore"
        destructive
      />
    </div>
  );
}
