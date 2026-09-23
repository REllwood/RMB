import { useMemo, useState } from "react";
import { Pause, Pencil, Play, Plus, RefreshCw, Trash2 } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { RecurringInput } from "@/lib/types";
import { useMoneyFormat } from "@/lib/money";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
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
  const [mode, setMode] = useState<Mode>({ kind: "list" });
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
  const namesQ = useIpcQuery(["customers", ""], () => ipc.listCustomers());
  const names = useMemo(
    () => new Map((namesQ.data ?? []).map((c) => [c.id, c.name])),
    [namesQ.data],
  );
  const q = useIpcQuery(["recurring"], () => ipc.listRecurring());
  const toggle = useIpcMutation(
    (v: { id: number; active: boolean }) => ipc.setRecurringActive(v.id, v.active),
    [["recurring"]],
  );
  const del = useIpcMutation((id: number) => ipc.deleteRecurring(id), [["recurring"]], {
    successMessage: "Schedule deleted",
  });
  const runNow = useIpcMutation(
    () => ipc.runRecurringNow(),
    [["recurring"], ["invoices"], ["dashboard"]],
  );
  const [deleting, setDeleting] = useState<number | null>(null);

  async function onRunNow() {
    try {
      const count = await runNow.mutateAsync(undefined);
      toast(
        "success",
        count > 0
          ? `Created ${count} draft invoice${count === 1 ? "" : "s"}`
          : "Nothing due — all schedules are up to date",
      );
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }

  if (q.isLoading || namesQ.isLoading) return <Loading />;
  const loadError = q.error ?? namesQ.error;
  if (loadError)
    return (
      <ErrorState error={loadError} onRetry={() => Promise.all([q.refetch(), namesQ.refetch()])} />
    );

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
                <TableHead className="text-right">Amount</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {q.data.map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-medium">{names.get(r.customer_id) ?? "—"}</TableCell>
                  <TableCell className="capitalize">{r.frequency}</TableCell>
                  <TableCell className="text-muted-foreground">{r.next_date}</TableCell>
                  <TableCell className="text-right tabular-nums">{money(r.total_minor)}</TableCell>
                  <TableCell>
                    <Badge variant={r.active ? "success" : "outline"}>
                      {r.active ? "active" : "paused"}
                    </Badge>
                    {r.problem && (
                      <p className="mt-1 max-w-56 text-xs text-destructive">
                        Needs attention: {r.problem}. Edit the schedule to fix it.
                      </p>
                    )}
                  </TableCell>
                  <TableCell className="text-right whitespace-nowrap">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => toggle.mutate({ id: r.id, active: !r.active })}
                      loading={toggle.isPending && toggle.variables?.id === r.id}
                      loadingLabel={r.active ? "Pausing…" : "Resuming…"}
                      aria-label={r.active ? "Pause schedule" : "Resume schedule"}
                    >
                      {r.active ? <Pause className="size-4" /> : <Play className="size-4" />}
                      {r.active ? "Pause" : "Resume"}
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => onEdit(r.id)}>
                      <Pencil className="size-4" /> Edit
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => setDeleting(r.id)}>
                      <Trash2 className="size-4" /> Delete
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </CardContent>

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

  const s = initial?.schedule;
  const [customerId, setCustomerId] = useState<number | null>(s?.customer_id ?? null);
  const [frequency, setFrequency] = useState(s?.frequency ?? "monthly");
  const [nextDate, setNextDate] = useState(s?.next_date ?? todayLocalISO());
  const [endDate, setEndDate] = useState(s?.end_date ?? "");
  const [dueDays, setDueDays] = useState(s?.due_days != null ? String(s.due_days) : "14");
  const [notes, setNotes] = useState(s?.notes ?? "");
  const [edited, setEdited] = useState<EditLine[] | null>(initial ? fromRows(initial.lines) : null);

  const save = useIpcMutation(
    async (v: { input: RecurringInput; lines: ReturnType<typeof toLineInputs> }) => {
      if (initial) await ipc.updateRecurring(initial.schedule.id, v.input, v.lines);
      else await ipc.createRecurring(v.input, v.lines);
    },
    [["recurring"]],
    { successMessage: initial ? "Schedule updated" : "Schedule created" },
  );

  if (customersQ.isLoading || taxQ.isLoading || itemsQ.isLoading) return <Loading />;
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
      <EmptyState title="Add a customer first" description="Schedules belong to a customer." />
    );

  const taxes = taxQ.data ?? [];
  const items = itemsQ.data ?? [];
  const lines = edited ?? [emptyLine(taxes)];
  const lineIssues = validateEditLines(lines, items);
  const hasLineIssues = lineIssues.some(
    (issue) => issue.description || issue.quantity || issue.price,
  );
  const payload = toLineInputs(lines);
  const parsedDueDays = dueDays.trim() === "" ? null : Number(dueDays);
  const dueDaysValid =
    parsedDueDays === null ||
    (Number.isInteger(parsedDueDays) && parsedDueDays >= 0 && parsedDueDays <= 3_650);
  const datesValid = Boolean(nextDate) && (!endDate || endDate >= nextDate);
  const valid =
    customerId !== null && payload.length > 0 && datesValid && dueDaysValid && !hasLineIssues;

  async function onSave() {
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
            hint="Month-end schedules stay anchored to month end"
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
          issues={lineIssues}
        />

        <div className="flex gap-2">
          <Button
            onClick={onSave}
            disabled={!valid}
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
