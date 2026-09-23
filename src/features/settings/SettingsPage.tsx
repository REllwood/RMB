import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { Settings, TaxRate } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { PageHeader } from "@/components/ui/page-header";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ErrorState, Loading } from "@/components/ui/states";
import { parsePercentToBp, parseWholeNumber } from "@/lib/money";
import { formatNumberPreview } from "@/lib/format";

const CURRENCIES = ["USD", "GBP", "EUR", "AUD", "NZD", "CAD"];
const COUNTRIES: [string, string][] = [
  ["UK", "United Kingdom — VAT"],
  ["AU", "Australia — GST"],
  ["NZ", "New Zealand — GST"],
  ["CA", "Canada — GST/HST"],
  ["US", "United States — configure state/local rates"],
];

type TaxDraft = { id: number; name: string; percent: string; inclusive: boolean };

export function SettingsPage() {
  const toast = useToast();
  const settingsQ = useIpcQuery(["settings"], () => ipc.getSettings());
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const [form, setForm] = useState<Settings | null>(null);
  const [status, setStatus] = useState("");

  useEffect(() => {
    if (settingsQ.data) setForm((f) => f ?? settingsQ.data);
  }, [settingsQ.data]);

  const saveMut = useIpcMutation((v: Settings) => ipc.updateSettings(v), [["settings"]]);
  const setLogoMut = useIpcMutation((src: string) => ipc.setLogo(src), [["settings"]], {
    successMessage: "Logo updated — it now appears on your PDFs",
  });
  const clearLogoMut = useIpcMutation(() => ipc.clearLogo(), [["settings"]], {
    successMessage: "Logo removed",
  });
  const presetMut = useIpcMutation(
    (c: string) => ipc.applyTaxPreset(c),
    [["tax-rates"], ["settings"]],
    { successMessage: "Preset rates added" },
  );
  const defaultTaxMut = useIpcMutation(
    (id: number | null) => {
      const saved = settingsQ.data;
      if (!saved) throw new Error("Settings are still loading");
      return ipc.updateSettings({ ...saved, default_tax_rate_id: id });
    },
    [["settings"]],
    { successMessage: "Default tax rate saved" },
  );
  const addTaxMut = useIpcMutation(
    (r: { name: string; bp: number; inc: boolean }) => ipc.createTaxRate(r.name, r.bp, r.inc),
    [["tax-rates"]],
  );
  const editTaxMut = useIpcMutation(
    (r: { id: number; name: string; bp: number; inc: boolean }) =>
      ipc.updateTaxRate(r.id, r.name, r.bp, r.inc),
    [["tax-rates"]],
    { successMessage: "Tax rate updated" },
  );
  const archiveMut = useIpcMutation((id: number) => ipc.archiveTaxRate(id), [["tax-rates"]]);
  const backupMut = useIpcMutation((path: string) => ipc.backupDatabase(path), [], {
    successMessage: "Backup written",
  });
  const restoreMut = useIpcMutation((path: string) => ipc.restoreDatabase(path));

  const [taxName, setTaxName] = useState("");
  const [taxPct, setTaxPct] = useState("");
  const [taxInclusive, setTaxInclusive] = useState<boolean | null>(null);
  const [padText, setPadText] = useState<string | null>(null);
  const [taxDraft, setTaxDraft] = useState<TaxDraft | null>(null);
  const [presetCountry, setPresetCountry] = useState("");
  const [confirmRestore, setConfirmRestore] = useState<string | null>(null);
  const [choosingBackup, setChoosingBackup] = useState(false);
  const [choosingRestore, setChoosingRestore] = useState(false);
  const [choosingLogo, setChoosingLogo] = useState(false);

  if (settingsQ.isLoading || !form) return <Loading label="Loading settings…" />;
  if (settingsQ.error)
    return <ErrorState error={settingsQ.error} onRetry={() => settingsQ.refetch()} />;

  const padError =
    padText !== null && parseWholeNumber(padText, { min: 1, max: 12 }) === null
      ? "Enter a whole number from 1 to 12"
      : undefined;

  function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    setForm((f) => (f ? { ...f, [key]: value } : f));
  }

  async function onSave() {
    if (!form) return;
    setStatus("");
    try {
      await saveMut.mutateAsync(form);
      setPadText(null);
      setStatus("Settings saved.");
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  async function onBackup() {
    setChoosingBackup(true);
    let path: string | null = null;
    try {
      const now = new Date();
      const two = (value: number) => String(value).padStart(2, "0");
      const stamp = `${now.getFullYear()}${two(now.getMonth() + 1)}${two(now.getDate())}-${two(now.getHours())}${two(now.getMinutes())}${two(now.getSeconds())}`;
      path = await save({
        defaultPath: `rmb-backup-${stamp}.sqlite`,
        filters: [{ name: "SQLite", extensions: ["sqlite"] }],
      });
    } catch (error) {
      toast("error", error instanceof Error ? error.message : String(error));
    } finally {
      setChoosingBackup(false);
    }
    if (path) {
      try {
        await backupMut.mutateAsync(path);
      } catch {
        // useIpcMutation has already shown the backend error.
      }
    }
  }

  async function onRestore() {
    setChoosingRestore(true);
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "SQLite", extensions: ["sqlite"] }],
      });
      if (typeof path === "string") setConfirmRestore(path);
    } catch (error) {
      toast("error", error instanceof Error ? error.message : String(error));
    } finally {
      setChoosingRestore(false);
    }
  }

  const newRateBp = parsePercentToBp(taxPct);
  const newRateError =
    taxPct.trim() && newRateBp === null
      ? "Enter a percentage from 0 to 1,000 with at most two decimal places"
      : undefined;
  // New rates follow the saved "prices include tax" setting unless changed here.
  const newRateInclusive = taxInclusive ?? settingsQ.data?.prices_tax_inclusive ?? false;

  async function onAddTax() {
    if (!taxName.trim() || newRateBp === null) return;
    try {
      await addTaxMut.mutateAsync({
        name: taxName.trim(),
        bp: newRateBp,
        inc: newRateInclusive,
      });
      setTaxName("");
      setTaxPct("");
      setTaxInclusive(null);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  async function onApplyPreset(country: string) {
    try {
      await presetMut.mutateAsync(country);
      setPresetCountry("");
      // The first preset also chooses the default rate for new lines.
      const refreshed = await settingsQ.refetch();
      if (refreshed.data) set("default_tax_rate_id", refreshed.data.default_tax_rate_id);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  function startTaxEdit(rate: TaxRate) {
    setTaxDraft({
      id: rate.id,
      name: rate.name,
      percent: (rate.rate_bp / 100).toString(),
      inclusive: rate.inclusive,
    });
  }

  const draftRateBp = taxDraft ? parsePercentToBp(taxDraft.percent) : null;
  const draftRateError =
    taxDraft && draftRateBp === null
      ? "Enter a percentage from 0 to 1,000 with at most two decimal places"
      : taxDraft && !taxDraft.name.trim()
        ? "Enter a name"
        : undefined;

  async function onSaveTax() {
    if (!taxDraft || draftRateError || draftRateBp === null) return;
    try {
      await editTaxMut.mutateAsync({
        id: taxDraft.id,
        name: taxDraft.name.trim(),
        bp: draftRateBp,
        inc: taxDraft.inclusive,
      });
      setTaxDraft(null);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  async function onChooseLogo() {
    setChoosingLogo(true);
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg"] }],
      });
      if (typeof path === "string") {
        try {
          const stored = await setLogoMut.mutateAsync(path);
          set("logo_path", stored);
        } catch {
          // useIpcMutation has already shown the backend error.
        }
      }
    } catch (error) {
      toast("error", error instanceof Error ? error.message : String(error));
    } finally {
      setChoosingLogo(false);
    }
  }
  async function onClearLogo() {
    try {
      await clearLogoMut.mutateAsync(undefined);
      set("logo_path", null);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
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
          <Field
            label="Business name"
            required
            error={
              !form.business_name.trim() ? "Required before an invoice can be issued" : undefined
            }
          >
            {(p) => (
              <Input
                {...p}
                value={form.business_name}
                onChange={(e) => set("business_name", e.target.value)}
              />
            )}
          </Field>
          <Field label="Email">
            {(p) => (
              <Input
                {...p}
                type="email"
                value={form.email}
                onChange={(e) => set("email", e.target.value)}
              />
            )}
          </Field>
          <Field label="Phone">
            {(p) => (
              <Input {...p} value={form.phone} onChange={(e) => set("phone", e.target.value)} />
            )}
          </Field>
          <Field
            label="Currency"
            hint={
              form.currency_locked
                ? "Locked once an invoice has been issued, so totals never mix currencies."
                : "Choose before you issue your first invoice."
            }
          >
            {(p) => (
              <Select
                {...p}
                disabled={form.currency_locked}
                value={form.currency}
                onChange={(e) => set("currency", e.target.value)}
              >
                {CURRENCIES.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          <Field label="Address">
            {(p) => (
              <Textarea
                {...p}
                value={form.address}
                onChange={(e) => set("address", e.target.value)}
              />
            )}
          </Field>
          <Field label="Tax label" hint="e.g. VAT, GST, Sales Tax">
            {(p) => (
              <Input
                {...p}
                value={form.tax_label}
                onChange={(e) => set("tax_label", e.target.value)}
              />
            )}
          </Field>
          <Field label="Tax number">
            {(p) => (
              <Input
                {...p}
                value={form.tax_number}
                onChange={(e) => set("tax_number", e.target.value)}
              />
            )}
          </Field>
          <Field label="Invoice number prefix">
            {(p) => (
              <Input
                {...p}
                value={form.invoice_prefix}
                onChange={(e) => set("invoice_prefix", e.target.value)}
              />
            )}
          </Field>
          <Field label="Quote number prefix">
            {(p) => (
              <Input
                {...p}
                value={form.quote_prefix}
                onChange={(e) => set("quote_prefix", e.target.value)}
              />
            )}
          </Field>
          <Field
            label="Number padding"
            error={padError}
            hint={
              padError
                ? undefined
                : `Digits after the prefix. Next invoice: ${formatNumberPreview(form.invoice_prefix, form.invoice_next_seq, form.number_pad)}`
            }
          >
            {(p) => (
              <Input
                {...p}
                inputMode="numeric"
                className="w-24"
                value={padText ?? String(form.number_pad)}
                onChange={(e) => {
                  setPadText(e.target.value);
                  const pad = parseWholeNumber(e.target.value, { min: 1, max: 12 });
                  if (pad !== null) set("number_pad", pad);
                }}
              />
            )}
          </Field>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={form.prices_tax_inclusive}
              onChange={(e) => set("prices_tax_inclusive", e.target.checked)}
            />
            New tax rates are tax-inclusive (prices already include the tax)
          </label>
          <div className="flex flex-wrap items-center gap-3 sm:col-span-2">
            <span className="text-sm font-medium">Logo</span>
            {form.logo_path ? (
              <>
                <span className="max-w-56 truncate text-sm text-muted-foreground">
                  {form.logo_path.split(/[\\/]/).pop()}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onClearLogo}
                  loading={clearLogoMut.isPending}
                  loadingLabel="Removing…"
                >
                  Remove
                </Button>
              </>
            ) : (
              <Button
                variant="outline"
                size="sm"
                onClick={onChooseLogo}
                loading={choosingLogo || setLogoMut.isPending}
                loadingLabel={choosingLogo ? "Opening…" : "Importing…"}
              >
                Choose logo…
              </Button>
            )}
            <span className="text-xs text-muted-foreground">
              PNG or JPEG — shown on your quote &amp; invoice PDFs
            </span>
          </div>
        </CardContent>
        <CardContent className="flex items-center gap-3 border-t pt-4">
          <Button
            onClick={onSave}
            disabled={Boolean(padError)}
            loading={saveMut.isPending}
            loadingLabel="Saving…"
          >
            Save settings
          </Button>
          {status && (
            <span role="status" className="text-sm text-muted-foreground">
              {status}
            </span>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Tax rates</CardTitle>
          <CardDescription>
            Configure your own, or add a current starting preset. Confirm the rates that apply to
            your location and business with your tax authority or adviser.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-end gap-2">
            <Field label="Add a country preset">
              {(p) => (
                <Select
                  {...p}
                  value={presetCountry}
                  disabled={presetMut.isPending}
                  onChange={(e) => setPresetCountry(e.target.value)}
                  className="w-72"
                >
                  <option value="" disabled>
                    Choose a country…
                  </option>
                  {COUNTRIES.map(([code, label]) => (
                    <option key={code} value={code}>
                      {label}
                    </option>
                  ))}
                </Select>
              )}
            </Field>
            <Button
              variant="outline"
              onClick={() => presetCountry && void onApplyPreset(presetCountry)}
              disabled={!presetCountry}
              loading={presetMut.isPending}
              loadingLabel="Adding rates…"
            >
              Add preset rates
            </Button>
            {taxQ.data && taxQ.data.length > 0 && (
              <Field
                label="Default for new lines"
                hint="The tax new invoice, quote and job lines start with"
              >
                {(p) => (
                  <Select
                    {...p}
                    className="w-56"
                    value={form.default_tax_rate_id ?? ""}
                    disabled={defaultTaxMut.isPending}
                    onChange={async (e) => {
                      const id = e.target.value ? Number(e.target.value) : null;
                      try {
                        await defaultTaxMut.mutateAsync(id);
                        set("default_tax_rate_id", id);
                      } catch {
                        // useIpcMutation has already shown the backend error.
                      }
                    }}
                  >
                    <option value="">Automatic (first non-zero rate)</option>
                    {taxQ.data.map((r) => (
                      <option key={r.id} value={r.id}>
                        {r.name}
                      </option>
                    ))}
                  </Select>
                )}
              </Field>
            )}
          </div>

          {taxQ.isLoading ? (
            <Loading label="Loading tax rates…" />
          ) : taxQ.error ? (
            <ErrorState error={taxQ.error} onRetry={() => taxQ.refetch()} />
          ) : taxQ.data && taxQ.data.length > 0 ? (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Rate</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead>
                    <span className="sr-only">Actions</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {taxQ.data.map((r) =>
                  taxDraft?.id === r.id ? (
                    <TableRow key={r.id}>
                      <TableCell>
                        <Input
                          aria-label="Tax rate name"
                          value={taxDraft.name}
                          onChange={(e) => setTaxDraft({ ...taxDraft, name: e.target.value })}
                        />
                      </TableCell>
                      <TableCell>
                        <Input
                          aria-label="Tax rate percentage"
                          aria-invalid={Boolean(draftRateError)}
                          aria-describedby={draftRateError ? "tax-edit-error" : undefined}
                          className="w-24"
                          inputMode="decimal"
                          value={taxDraft.percent}
                          onChange={(e) => setTaxDraft({ ...taxDraft, percent: e.target.value })}
                        />
                        {draftRateError && (
                          <p
                            id="tax-edit-error"
                            role="alert"
                            className="mt-1 text-xs text-destructive"
                          >
                            {draftRateError}
                          </p>
                        )}
                      </TableCell>
                      <TableCell>
                        <label className="flex items-center gap-2 text-sm">
                          <input
                            type="checkbox"
                            checked={taxDraft.inclusive}
                            onChange={(e) =>
                              setTaxDraft({ ...taxDraft, inclusive: e.target.checked })
                            }
                          />
                          Inclusive
                        </label>
                      </TableCell>
                      <TableCell className="text-right whitespace-nowrap">
                        <Button
                          size="sm"
                          onClick={onSaveTax}
                          disabled={Boolean(draftRateError)}
                          loading={editTaxMut.isPending}
                          loadingLabel="Saving…"
                        >
                          Save
                        </Button>
                        <Button variant="ghost" size="sm" onClick={() => setTaxDraft(null)}>
                          Cancel
                        </Button>
                      </TableCell>
                    </TableRow>
                  ) : (
                    <TableRow key={r.id}>
                      <TableCell>{r.name}</TableCell>
                      <TableCell>{(r.rate_bp / 100).toFixed(2)}%</TableCell>
                      <TableCell>
                        <Badge variant={r.inclusive ? "secondary" : "outline"}>
                          {r.inclusive ? "Inclusive" : "Exclusive"}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right whitespace-nowrap">
                        <Button variant="ghost" size="sm" onClick={() => startTaxEdit(r)}>
                          Edit
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => archiveMut.mutate(r.id)}
                          loading={archiveMut.isPending && archiveMut.variables === r.id}
                          loadingLabel="Archiving…"
                        >
                          Archive
                        </Button>
                      </TableCell>
                    </TableRow>
                  ),
                )}
              </TableBody>
            </Table>
          ) : (
            <p className="rounded-lg border border-dashed px-4 py-5 text-sm text-muted-foreground">
              No active tax rates. Add one below or use a country preset.
            </p>
          )}

          <div className="flex flex-wrap items-end gap-2 border-t pt-4">
            <Field label="New rate name">
              {(p) => (
                <Input
                  {...p}
                  className="w-44"
                  value={taxName}
                  onChange={(e) => setTaxName(e.target.value)}
                  placeholder="VAT 20%"
                />
              )}
            </Field>
            <Field label="Rate %" error={newRateError}>
              {(p) => (
                <Input
                  {...p}
                  className="w-24"
                  inputMode="decimal"
                  value={taxPct}
                  onChange={(e) => setTaxPct(e.target.value)}
                  placeholder="20"
                />
              )}
            </Field>
            <label className="flex h-9 items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={newRateInclusive}
                onChange={(e) => setTaxInclusive(e.target.checked)}
              />
              Prices include this tax
            </label>
            <Button
              variant="outline"
              onClick={onAddTax}
              disabled={!taxName.trim() || newRateBp === null}
              loading={addTaxMut.isPending}
              loadingLabel="Adding…"
            >
              Add rate
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Backup &amp; restore</CardTitle>
          <CardDescription>
            Your data lives in a single local file. Back it up regularly.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex gap-3">
          <Button
            variant="outline"
            onClick={onBackup}
            loading={choosingBackup || backupMut.isPending}
            loadingLabel={choosingBackup ? "Choosing location…" : "Writing backup…"}
          >
            Back up…
          </Button>
          <Button
            variant="outline"
            onClick={onRestore}
            loading={choosingRestore}
            loadingLabel="Choosing backup…"
          >
            Restore…
          </Button>
        </CardContent>
      </Card>

      <ConfirmDialog
        open={confirmRestore !== null}
        onClose={() => !restoreMut.isPending && setConfirmRestore(null)}
        onConfirm={async () => {
          if (!confirmRestore) return;
          try {
            await restoreMut.mutateAsync(confirmRestore);
            setConfirmRestore(null);
          } catch {
            // useIpcMutation has already shown the validation or restore error.
          }
        }}
        title="Restore from backup?"
        description="All current data is replaced by the backup and the app restarts. Back up first if unsure."
        confirmLabel="Restore"
        destructive
        pending={restoreMut.isPending}
      />
    </div>
  );
}
