import { useState } from "react";
import { Mail, MapPin, Pencil, Phone, Plus, Trash2 } from "lucide-react";

import { useView } from "@/app/nav";
import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { Customer, CustomerInput } from "@/lib/types";
import { useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { PageHeader } from "@/components/ui/page-header";
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
import { invoiceStatus } from "@/features/invoices/status";

const EMPTY: CustomerInput = { name: "", email: "", phone: "", billing_address: "", notes: "" };

type View = { mode: "list" } | { mode: "detail"; id: number };

export function CustomersPage() {
  const [view, setView] = useView<View>({ mode: "list" });

  return (
    <div className="space-y-6">
      {view.mode === "list" ? (
        <CustomerList onOpen={(id) => setView({ mode: "detail", id })} />
      ) : (
        <CustomerDetail id={view.id} onBack={() => setView({ mode: "list" })} />
      )}
    </div>
  );
}

function CustomerForm({
  initial,
  pending,
  onSubmit,
  onCancel,
}: {
  initial: CustomerInput;
  pending: boolean;
  onSubmit: (input: CustomerInput) => void;
  onCancel: () => void;
}) {
  const [input, setInput] = useState<CustomerInput>(initial);
  function field<K extends keyof CustomerInput>(key: K, value: CustomerInput[K]) {
    setInput((i) => ({ ...i, [key]: value }));
  }
  return (
    <Card>
      <CardContent className="grid gap-4 pt-6 sm:grid-cols-2">
        <Field label="Name" required>
          {(p) => (
            <Input {...p} value={input.name} onChange={(e) => field("name", e.target.value)} />
          )}
        </Field>
        <Field label="Email">
          {(p) => (
            <Input
              {...p}
              type="email"
              value={input.email}
              onChange={(e) => field("email", e.target.value)}
            />
          )}
        </Field>
        <Field label="Phone">
          {(p) => (
            <Input {...p} value={input.phone} onChange={(e) => field("phone", e.target.value)} />
          )}
        </Field>
        <Field label="Billing address">
          {(p) => (
            <Input
              {...p}
              value={input.billing_address}
              onChange={(e) => field("billing_address", e.target.value)}
            />
          )}
        </Field>
        <div className="sm:col-span-2">
          <Field label="Notes">
            {(p) => (
              <Textarea
                {...p}
                value={input.notes}
                onChange={(e) => field("notes", e.target.value)}
              />
            )}
          </Field>
        </div>
        <div className="flex gap-2 sm:col-span-2">
          <Button
            onClick={() => onSubmit(input)}
            disabled={!input.name.trim()}
            loading={pending}
            loadingLabel="Saving…"
          >
            Save
          </Button>
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function CustomerList({ onOpen }: { onOpen: (id: number) => void }) {
  const [search, setSearch] = useState("");
  const [creating, setCreating] = useState(false);

  const listQ = useIpcQuery(["customers", search], () => ipc.listCustomers(search || undefined));
  const createMut = useIpcMutation((i: CustomerInput) => ipc.createCustomer(i), [["customers"]]);

  return (
    <>
      <PageHeader
        title="Customers"
        description="Everyone you quote, work for, and invoice."
        actions={
          <Button onClick={() => setCreating(true)}>
            <Plus className="size-4" /> Add customer
          </Button>
        }
      />

      {creating && (
        <CustomerForm
          initial={{ ...EMPTY }}
          pending={createMut.isPending}
          onSubmit={async (input) => {
            try {
              const id = await createMut.mutateAsync(input);
              setCreating(false);
              onOpen(id);
            } catch {
              // useIpcMutation has already shown the backend error.
            }
          }}
          onCancel={() => setCreating(false)}
        />
      )}

      <div className="max-w-sm">
        <label htmlFor="cust-search" className="sr-only">
          Search customers
        </label>
        <Input
          id="cust-search"
          placeholder="Search name or email…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {listQ.isLoading ? (
        <Loading />
      ) : listQ.error ? (
        <ErrorState error={listQ.error} onRetry={() => listQ.refetch()} />
      ) : listQ.data && listQ.data.length === 0 ? (
        <EmptyState
          title="No customers yet"
          description="Add your first customer to start quoting and invoicing."
          action={
            <Button onClick={() => setCreating(true)}>
              <Plus className="size-4" /> Add customer
            </Button>
          }
        />
      ) : (
        <Card className="overflow-hidden py-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="pl-4">Name</TableHead>
                <TableHead>Email</TableHead>
                <TableHead>Phone</TableHead>
                <TableHead>
                  <span className="sr-only">Open</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {listQ.data?.map((c) => (
                <TableRow key={c.id} className="cursor-pointer" onClick={() => onOpen(c.id)}>
                  <TableCell className="pl-4 font-medium">{c.name}</TableCell>
                  <TableCell>{c.email}</TableCell>
                  <TableCell>{c.phone}</TableCell>
                  <TableCell className="pr-4 text-right whitespace-nowrap">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        onOpen(c.id);
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
      )}
    </>
  );
}

function CustomerDetail({ id, onBack }: { id: number; onBack: () => void }) {
  const money = useMoneyFormat();
  const q = useIpcQuery(["customer", id], () => ipc.getCustomer(id));
  const historyQ = useIpcQuery(["customer-history", id], () => ipc.customerHistory(id));
  const updateMut = useIpcMutation(
    (v: { id: number; input: CustomerInput }) => ipc.updateCustomer(v.id, v.input),
    [["customers"], ["customer", id]],
    { successMessage: "Customer updated" },
  );
  const deleteMut = useIpcMutation((cid: number) => ipc.deleteCustomer(cid), [["customers"]], {
    successMessage: "Customer deleted",
  });
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  if (q.isLoading) return <Loading />;
  if (q.error) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  if (!q.data) return <EmptyState title="Customer not found" />;
  const c: Customer = q.data;
  const h = historyQ.data;

  return (
    <>
      <PageHeader
        title={c.name}
        description={`Customer since ${c.created_at.slice(0, 10)}`}
        actions={
          <>
            <Button variant="ghost" onClick={onBack}>
              ← Back to list
            </Button>
            <Button variant="outline" onClick={() => setEditing((e) => !e)}>
              <Pencil className="size-4" /> Edit
            </Button>
            <Button variant="ghost" onClick={() => setConfirmDelete(true)}>
              <Trash2 className="size-4" /> Delete
            </Button>
          </>
        }
      />

      {editing ? (
        <CustomerForm
          initial={{
            name: c.name,
            email: c.email,
            phone: c.phone,
            billing_address: c.billing_address,
            notes: c.notes,
          }}
          pending={updateMut.isPending}
          onSubmit={async (input) => {
            try {
              await updateMut.mutateAsync({ id, input });
              setEditing(false);
            } catch {
              // useIpcMutation has already shown the backend error.
            }
          }}
          onCancel={() => setEditing(false)}
        />
      ) : (
        <Card>
          <CardContent className="grid gap-3 pt-6 text-sm sm:grid-cols-2">
            <p className="flex items-center gap-2">
              <Mail className="size-4 text-muted-foreground" aria-hidden />
              {c.email || <span className="text-muted-foreground">No email</span>}
            </p>
            <p className="flex items-center gap-2">
              <Phone className="size-4 text-muted-foreground" aria-hidden />
              {c.phone || <span className="text-muted-foreground">No phone</span>}
            </p>
            <p className="flex items-center gap-2 sm:col-span-2">
              <MapPin className="size-4 text-muted-foreground" aria-hidden />
              {c.billing_address || (
                <span className="text-muted-foreground">No billing address</span>
              )}
            </p>
            {c.notes && <p className="text-muted-foreground sm:col-span-2">{c.notes}</p>}
          </CardContent>
        </Card>
      )}

      {historyQ.isLoading && <Loading label="Loading history…" />}
      {h && (
        <div className="grid gap-4 lg:grid-cols-2">
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle className="text-base">Invoices</CardTitle>
            </CardHeader>
            <CardContent>
              {h.invoices.length === 0 ? (
                <p className="text-sm text-muted-foreground">No invoices for this customer yet.</p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Number</TableHead>
                      <TableHead>Date</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Total</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {h.invoices.map((inv) => {
                      const s = invoiceStatus(inv);
                      return (
                        <TableRow key={inv.id}>
                          <TableCell className="font-medium">
                            {inv.number ?? `Draft #${inv.id}`}
                          </TableCell>
                          <TableCell className="text-muted-foreground">
                            {inv.issue_date ?? inv.created_at.slice(0, 10)}
                          </TableCell>
                          <TableCell>
                            <Badge variant={s.variant}>{s.label}</Badge>
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            {money(inv.total_minor)}
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-base">Quotes</CardTitle>
            </CardHeader>
            <CardContent>
              {h.quotes.length === 0 ? (
                <p className="text-sm text-muted-foreground">No quotes yet.</p>
              ) : (
                <ul className="divide-y">
                  {h.quotes.map((quote) => (
                    <li key={quote.id} className="flex items-center justify-between py-2 text-sm">
                      <span className="font-medium">{quote.number ?? `#${quote.id}`}</span>
                      <span className="flex items-center gap-3">
                        <span className="tabular-nums">{money(quote.total_minor)}</span>
                        <Badge variant="outline">{quote.status}</Badge>
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-base">Jobs</CardTitle>
            </CardHeader>
            <CardContent>
              {h.jobs.length === 0 ? (
                <p className="text-sm text-muted-foreground">No jobs yet.</p>
              ) : (
                <ul className="divide-y">
                  {h.jobs.map((job) => (
                    <li key={job.id} className="flex items-center justify-between py-2 text-sm">
                      <span className="font-medium">{job.title}</span>
                      <Badge variant="outline">{job.status.replace("_", "-")}</Badge>
                    </li>
                  ))}
                </ul>
              )}
            </CardContent>
          </Card>
        </div>
      )}

      <ConfirmDialog
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={async () => {
          try {
            await deleteMut.mutateAsync(id);
            onBack();
          } catch {
            // Keep the dialog open so the active-work validation is visible.
          }
        }}
        title={`Delete ${c.name}?`}
        description="Customers with draft or active work cannot be deleted. Completed documents keep their historical details."
        confirmLabel="Delete customer"
        destructive
        pending={deleteMut.isPending}
      />
    </>
  );
}
