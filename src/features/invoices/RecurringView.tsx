import { useState } from "react";
import { Pause, Pencil, Play, Plus, RefreshCw, Trash2 } from "lucide-react";

import { useNav, useUnsavedEdits, useView } from "@/app/nav";
import { ipc } from "@/lib/ipc";
import { DOCUMENT_KEYS } from "@/lib/query";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { RecurringInput, RecurringListRow, ResumePreview } from "@/lib/types";
import { parseWholeNumber, useMoneyFormat } from "@/lib/money";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog, Dialog } from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
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
import { LineEditor } from "@/features/shared/LineEditor";
import {
  emptyLine,
  fromRows,
  hasLineIssues,
  toLineInputs,
  validateEditLines,
  type EditLine,
} from "@/features/shared/lines";
import { todayLocalISO } from "@/lib/format";

const FREQUENCIES: [string, string][] = [
  ["weekly", "Weekly"],
  ["fortnightly", "Fortnightly"],
  ["monthly", "Monthly"],
  ["quarterly", "Quarterly"],
  ["yearly", "Yearly"],
];

type Mode = { kind: "list" } | { kind: "create" } | { kind: "edit"; id: number };

/** Recurring invoice schedules — reached from the Invoices page. */
export function RecurringView() {
  const [mode, setMode] = useView<Mode>(() => ({ kind: "list" }));
  return (
    <div className="space-y-4">
      {mode.kind === "list" && (
        <ScheduleList
          onCreate={() => setMode({ kind: "create" })}
          onEdit={(id) => setMode({ kind: "edit", id })}
        />
      )}
      {mode.kind === "create" && <ScheduleForm onDone={() => setMode({ kind: "list" })} />}
      {mode.kind === "edit" && (
        <ScheduleEdit id={mode.id} onDone={() => setMode({ kind: "list" })} />
      )}
    </div>
  );
}

function ScheduleList({
  onCreate,
  onEdit,
}: {
  onCreate: () => void;
  onEdit: (id: number) => void;
}) {
  const money = useMoneyFormat();
  const toast = useToast();
  const q = useIpcQuery(["recurring"], () => ipc.listRecurring());
  const toggle = useIpcMutation(
    (v: { id: number; active: boolean; skipMissed: boolean }) =>
      ipc.setRecurringActive(v.id, v.active, v.skipMissed),
    [["recurring"]],
  );
  const del = useIpcMutation((id: number) => ipc.deleteRecurring(id), [["recurring"]], {
    successMessage: "Schedule deleted",
  });
  const runNow = useIpcMutation(() => ipc.runRecurringNow(), DOCUMENT_KEYS);
  const [deleting, setDeleting] = useState<number | null>(null);
  const [resuming, setResuming] = useState<{
    row: RecurringListRow;
    preview: ResumePreview;
  } | null>(null);
  const [checking, setChecking] = useState<number | null>(null);

  async function onRunNow() {
    try {
      const report = await runNow.mutateAsync(undefined);
      const count = report.created.length;
      toast(
        "success",
        count > 0
          ? `Created ${count} draft invoice${count === 1 ? "" : "s"}`
          : "Nothing due — all schedules are up to date",
      );
      for (const problem of report.problems) toast("error", problem);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  async function resume(row: RecurringListRow, skipMissed: boolean) {
    try {
      await toggle.mutateAsync({ id: row.id, active: true, skipMissed });
      setResuming(null);
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  async function onToggle(row: RecurringListRow) {
    if (row.active) {
      toggle.mutate({ id: row.id, active: false, skipMissed: false });
      return;
    }
    // Resuming after a pause could back-fill every missed period — ask first.
    setChecking(row.id);
    try {
      const preview = await ipc.recurringResumePreview(row.id);
      if (preview.missed > 0) setResuming({ row, preview });
      else await resume(row, false);
    } catch (error) {
      toast("error", error instanceof Error ? error.message : String(error));
    } finally {
      setChecking(null);
    }
  }

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <div>
          <CardTitle className="text-base">Recurring schedules</CardTitle>
          <p className="mt-1 text-sm text-muted-foreground">
            Due schedules become draft invoices automatically when the app starts — you still review
            and issue them.
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={onRunNow}
            loading={runNow.isPending}
            loadingLabel="Generating…"
          >
            <RefreshCw className="size-4" /> Generate due now
          </Button>
          <Button onClick={onCreate}>
            <Plus className="size-4" /> New schedule
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {!q.data || q.data.length === 0 ? (
          <EmptyState
            title="No recurring schedules"
            description="Set up weekly, monthly, or yearly invoices for retainers and regular rounds."
            action={
              <Button onClick={onCreate}>
                <Plus className="size-4" /> New schedule
              </Button>
            }
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Customer</TableHead>
                <TableHead>Frequency</TableHead>
                <TableHead>Next</TableHead>
                <TableHead>Ends</TableHead>
                <TableHead className="text-right">Amount</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {q.data.map((r) => {
                const who = r.customer_name || "Deleted customer";
                return (
                  <TableRow key={r.id}>
                    <TableCell className="font-medium">{who}</TableCell>
                    <TableCell className="capitalize">{r.frequency}</TableCell>
                    <TableCell className="text-muted-foreground">
                      {r.ended ? "—" : r.next_date}
                    </TableCell>
                    <TableCell className="text-muted-foreground">{r.end_date ?? "Never"}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {money(r.total_minor)}
                    </TableCell>
                    <TableCell>
                      <Badge variant={r.ended ? "outline" : r.active ? "success" : "outline"}>
                        {r.ended ? "ended" : r.active ? "active" : "paused"}
                      </Badge>
                      {r.problem && (
                        <p className="mt-1 max-w-56 text-xs text-destructive">
                          Needs attention: {r.problem}. Edit the schedule to fix it.
                        </p>
                      )}
                    </TableCell>
                    <TableCell className="text-right whitespace-nowrap">
                      {!r.ended && (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => onToggle(r)}
                          loading={
                            checking === r.id || (toggle.isPending && toggle.variables?.id === r.id)
                          }
                          loadingLabel={r.active ? "Pausing…" : "Resuming…"}
                          aria-label={`${r.active ? "Pause" : "Resume"} the ${r.frequency} schedule for ${who}`}
                        >
                          {r.active ? <Pause className="size-4" /> : <Play className="size-4" />}
                          {r.active ? "Pause" : "Resume"}
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onEdit(r.id)}
                        aria-label={`Edit the ${r.frequency} schedule for ${who}`}
                      >
                        <Pencil className="size-4" /> Edit
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setDeleting(r.id)}
                        aria-label={`Delete the ${r.frequency} schedule for ${who}`}
                      >
                        <Trash2 className="size-4" /> Delete
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </CardContent>

      <Dialog
        open={resuming !== null}
        onClose={() => !toggle.isPending && setResuming(null)}
        title="Resume this schedule?"
        description={
          resuming
            ? `While it was paused, ${resuming.preview.missed} invoice${resuming.preview.missed === 1 ? " was" : "s were"} missed (from ${resuming.row.next_date}). Create a draft for each, or skip them and continue from ${resuming.preview.skip_to}?`
            : undefined
        }
        footer={
          resuming && (
            <>
              <Button
                variant="outline"
                onClick={() => setResuming(null)}
                disabled={toggle.isPending}
              >
                Cancel
              </Button>
              <Button
                variant="outline"
                onClick={() => resume(resuming.row, false)}
                disabled={toggle.isPending}
              >
                Create {resuming.preview.missed} draft{resuming.preview.missed === 1 ? "" : "s"}
              </Button>
              <Button
                onClick={() => resume(resuming.row, true)}
                loading={toggle.isPending}
                loadingLabel="Resuming…"
              >
                Skip to {resuming.preview.skip_to}
              </Button>
            </>
          )
        }
      />

      <ConfirmDialog
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        onConfirm={async () => {
          if (deleting === null) return;
          try {
            await del.mutateAsync(deleting);
            setDeleting(null);
          } catch {
            // Keep the dialog open so the backend explanation remains visible.
          }
        }}
        title="Delete this schedule?"
        description="Already-generated invoices are kept; only the schedule stops."
        confirmLabel="Delete schedule"
        destructive
        pending={del.isPending}
      />
    </Card>
  );
}

function ScheduleEdit({ id, onDone }: { id: number; onDone: () => void }) {
  const q = useIpcQuery(["recurring", id], () => ipc.getRecurring(id));
  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Schedule not found" />;
  return <ScheduleForm initial={q.data} onDone={onDone} />;
}

function ScheduleForm({
  initial,
  onDone,
}: {
  initial?: { schedule: { id: number } & RecurringInput; lines: Parameters<typeof fromRows>[0] };
  onDone: () => void;
}) {
  const customersQ = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const itemsQ = useIpcQuery(["items", ""], () => ipc.listItems());
  const settingsQ = useIpcQuery(["settings"], () => ipc.getSettings());
  const goTo = useNav();

  const s = initial?.schedule;
  const [customerId, setCustomerId] = useState<number | null>(s?.customer_id ?? null);
  const [frequency, setFrequency] = useState(s?.frequency ?? "monthly");
  const [nextDate, setNextDate] = useState(s?.next_date ?? todayLocalISO());
  const [endDate, setEndDate] = useState(s?.end_date ?? "");
  // New schedules default to 14-day terms; an existing schedule keeps "no due date" as blank.
  const [dueDays, setDueDays] = useState(s ? (s.due_days == null ? "" : String(s.due_days)) : "14");
  const [notes, setNotes] = useState(s?.notes ?? "");
  const [edited, setEdited] = useState<EditLine[] | null>(initial ? fromRows(initial.lines) : null);
  const [attempted, setAttempted] = useState(false);
  useUnsavedEdits({ customerId, frequency, nextDate, endDate, dueDays, notes, edited });

  const save = useIpcMutation(
    async (v: { input: RecurringInput; lines: ReturnType<typeof toLineInputs> }) => {
      if (initial) await ipc.updateRecurring(initial.schedule.id, v.input, v.lines);
      else await ipc.createRecurring(v.input, v.lines);
    },
    [["recurring"]],
    { successMessage: initial ? "Schedule updated" : "Schedule created" },
  );

  if (customersQ.isLoading || taxQ.isLoading || itemsQ.isLoading || settingsQ.isLoading)
    return <Loading />;
  const loadError = customersQ.error ?? taxQ.error ?? itemsQ.error;
  if (loadError)
    return (
      <ErrorState
        error={loadError}
        onRetry={() => Promise.all([customersQ.refetch(), taxQ.refetch(), itemsQ.refetch()])}
      />
    );
  if (customersQ.data && customersQ.data.length === 0)
    return (
      <EmptyState
        title="Add a customer first"
        description="Schedules belong to a customer."
        action={<Button onClick={() => goTo("customers")}>Go to Customers</Button>}
      />
    );

  const taxes = taxQ.data ?? [];
  const items = itemsQ.data ?? [];
  const defaultTaxRateId = settingsQ.data?.default_tax_rate_id ?? null;
  const lines = edited ?? [emptyLine(taxes, defaultTaxRateId)];
  const lineIssues = validateEditLines(lines, items);
  const lineProblems = hasLineIssues(lineIssues);
  const payload = toLineInputs(lines);
  const parsedDueDays =
    dueDays.trim() === "" ? null : parseWholeNumber(dueDays, { min: 0, max: 3_650 });
  const dueDaysValid = dueDays.trim() === "" || parsedDueDays !== null;
  const datesValid = Boolean(nextDate) && (!endDate || endDate >= nextDate);
  const customerMissing =
    customerId !== null && !(customersQ.data ?? []).some((c) => c.id === customerId);
  const valid =
    customerId !== null &&
    !customerMissing &&
    payload.length > 0 &&
    datesValid &&
    dueDaysValid &&
    !lineProblems;

  async function onSave() {
    setAttempted(true);
    if (!valid || customerId === null) return;
    try {
      await save.mutateAsync({
        input: {
          customer_id: customerId,
          frequency,
          next_date: nextDate,
          end_date: endDate || null,
          due_days: parsedDueDays,
          notes,
        },
        lines: payload,
      });
      onDone();
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">
          {initial ? "Edit schedule" : "New recurring schedule"}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid max-w-3xl gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <Field
            label="Customer"
            required
            error={
              customerMissing
                ? "This customer has been deleted — choose another customer"
                : attempted && customerId === null
                  ? "Choose a customer"
                  : undefined
            }
          >
            {(p) => (
              <Select
                {...p}
                value={customerId ?? ""}
                onChange={(e) => setCustomerId(e.target.value ? Number(e.target.value) : null)}
              >
                <option value="" disabled>
                  Choose a customer…
                </option>
                {customerMissing && customerId !== null && (
                  <option value={customerId} disabled>
                    Deleted customer
                  </option>
                )}
                {customersQ.data?.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          <Field label="Frequency">
            {(p) => (
              <Select {...p} value={frequency} onChange={(e) => setFrequency(e.target.value)}>
                {FREQUENCIES.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          <Field
            label="Next invoice date"
            required
            hint="Up to a year in the past: a draft is created for each missed period. Month-end dates stay at month end."
          >
            {(p) => (
              <Input
                {...p}
                type="date"
                value={nextDate}
                onChange={(e) => setNextDate(e.target.value)}
              />
            )}
          </Field>
          <Field
            label="End date"
            hint="Optional — stops after this date"
            error={
              endDate && endDate < nextDate ? "End date cannot be before the next date" : undefined
            }
          >
            {(p) => (
              <Input
                {...p}
                type="date"
                value={endDate}
                onChange={(e) => setEndDate(e.target.value)}
              />
            )}
          </Field>
          <Field
            label="Due in (days)"
            hint="Blank for no due date"
            error={!dueDaysValid ? "Enter a whole number from 0 to 3,650" : undefined}
          >
            {(p) => (
              <Input
                {...p}
                inputMode="numeric"
                value={dueDays}
                onChange={(e) => setDueDays(e.target.value)}
              />
            )}
          </Field>
          <Field label="Notes">
            {(p) => <Textarea {...p} value={notes} onChange={(e) => setNotes(e.target.value)} />}
          </Field>
        </div>

        <LineEditor
          lines={lines}
          onChange={setEdited}
          taxes={taxes}
          items={items}
          idPrefix="rec"
          issues={attempted ? lineIssues : []}
          defaultTaxRateId={defaultTaxRateId}
        />

        <div className="flex gap-2">
          <Button
            onClick={onSave}
            disabled={payload.length === 0}
            loading={save.isPending}
            loadingLabel="Saving…"
          >
            {initial ? "Save changes" : "Create schedule"}
          </Button>
          <Button variant="ghost" onClick={onDone}>
            Cancel
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
