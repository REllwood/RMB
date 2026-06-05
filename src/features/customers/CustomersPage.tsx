import { useState } from "react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { Customer, CustomerInput } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, ErrorState, Loading } from "@/components/ui/states";

const EMPTY: CustomerInput = { name: "", email: "", phone: "", billing_address: "", notes: "" };

export function CustomersPage() {
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<{ id: number | null; input: CustomerInput } | null>(null);

  const listQ = useIpcQuery(["customers", search], () => ipc.listCustomers(search || undefined));
  const createMut = useIpcMutation((i: CustomerInput) => ipc.createCustomer(i), [["customers"]]);
  const updateMut = useIpcMutation(
    (v: { id: number; input: CustomerInput }) => ipc.updateCustomer(v.id, v.input),
    [["customers"]],
  );
  const deleteMut = useIpcMutation((id: number) => ipc.deleteCustomer(id), [["customers"]]);

  function startCreate() {
    setEditing({ id: null, input: { ...EMPTY } });
  }
  function startEdit(c: Customer) {
    setEditing({ id: c.id, input: { name: c.name, email: c.email, phone: c.phone, billing_address: c.billing_address, notes: c.notes } });
  }
  async function onSubmit() {
    if (!editing || !editing.input.name.trim()) return;
    if (editing.id === null) await createMut.mutateAsync(editing.input);
    else await updateMut.mutateAsync({ id: editing.id, input: editing.input });
    setEditing(null);
  }
  function onDelete(c: Customer) {
    if (window.confirm(`Delete ${c.name}?`)) deleteMut.mutate(c.id);
  }
  function field<K extends keyof CustomerInput>(key: K, value: CustomerInput[K]) {
    setEditing((e) => (e ? { ...e, input: { ...e.input, [key]: value } } : e));
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold tracking-tight">Customers</h1>
        <Button onClick={startCreate}>Add customer</Button>
      </div>

      {editing && (
        <Card>
          <CardContent className="grid gap-4 pt-6 sm:grid-cols-2">
            <Field label="Name" required>
              {(p) => <Input {...p} value={editing.input.name} onChange={(e) => field("name", e.target.value)} />}
            </Field>
            <Field label="Email">
              {(p) => <Input {...p} type="email" value={editing.input.email} onChange={(e) => field("email", e.target.value)} />}
            </Field>
            <Field label="Phone">
              {(p) => <Input {...p} value={editing.input.phone} onChange={(e) => field("phone", e.target.value)} />}
            </Field>
            <Field label="Billing address">
              {(p) => <Input {...p} value={editing.input.billing_address} onChange={(e) => field("billing_address", e.target.value)} />}
            </Field>
            <div className="sm:col-span-2">
              <Field label="Notes">
                {(p) => <Textarea {...p} value={editing.input.notes} onChange={(e) => field("notes", e.target.value)} />}
              </Field>
            </div>
            <div className="flex gap-2 sm:col-span-2">
              <Button onClick={onSubmit} disabled={!editing.input.name.trim()}>
                {editing.id === null ? "Create" : "Save"}
              </Button>
              <Button variant="ghost" onClick={() => setEditing(null)}>Cancel</Button>
            </div>
          </CardContent>
        </Card>
      )}

      <div className="max-w-sm">
        <label htmlFor="cust-search" className="sr-only">Search customers</label>
        <Input id="cust-search" placeholder="Search name or email…" value={search} onChange={(e) => setSearch(e.target.value)} />
      </div>

      {listQ.isLoading ? (
        <Loading />
      ) : listQ.error ? (
        <ErrorState error={listQ.error} onRetry={() => listQ.refetch()} />
      ) : listQ.data && listQ.data.length === 0 ? (
        <EmptyState title="No customers yet" description="Add your first customer to start quoting and invoicing." action={<Button onClick={startCreate}>Add customer</Button>} />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Email</TableHead>
              <TableHead>Phone</TableHead>
              <TableHead><span className="sr-only">Actions</span></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {listQ.data?.map((c) => (
              <TableRow key={c.id}>
                <TableCell className="font-medium">{c.name}</TableCell>
                <TableCell>{c.email}</TableCell>
                <TableCell>{c.phone}</TableCell>
                <TableCell className="text-right whitespace-nowrap">
                  <Button variant="ghost" size="sm" onClick={() => startEdit(c)}>Edit</Button>
                  <Button variant="ghost" size="sm" onClick={() => onDelete(c)}>Delete</Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
