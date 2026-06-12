import { useState } from "react";
import { Plus } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { Item, ItemInput } from "@/lib/types";
import { minorToInput, parseMoney, useMoneyFormat } from "@/lib/money";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { ConfirmDialog, Dialog } from "@/components/ui/dialog";
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

type Draft = { id: number | null; input: ItemInput; price: string };

const EMPTY: Draft = {
  id: null,
  input: {
    kind: "product",
    name: "",
    sku: "",
    unit: "each",
    default_price_minor: 0,
    default_tax_rate_id: null,
    tracked: true,
    reorder_point: null,
  },
  price: "0.00",
};

export function CatalogPage() {
  const [search, setSearch] = useState("");
  const [draft, setDraft] = useState<Draft | null>(null);
  const [adjusting, setAdjusting] = useState<Item | null>(null);
  const [deleting, setDeleting] = useState<Item | null>(null);
  const money = useMoneyFormat();

  const listQ = useIpcQuery(["items", search], () => ipc.listItems(search || undefined));
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const createMut = useIpcMutation((i: ItemInput) => ipc.createItem(i), [["items"]]);
  const updateMut = useIpcMutation(
    (v: { id: number; input: ItemInput }) => ipc.updateItem(v.id, v.input),
    [["items"]],
  );
  const deleteMut = useIpcMutation((id: number) => ipc.deleteItem(id), [["items"]], {
    successMessage: "Item deleted",
  });
  const adjustMut = useIpcMutation(
    (v: { id: number; delta: number; note: string }) => ipc.adjustStock(v.id, v.delta, v.note),
    [["items"], ["dashboard"]],
    { successMessage: "Stock adjusted" },
  );

  function startEdit(it: Item) {
    setDraft({
      id: it.id,
      input: {
        kind: it.kind,
        name: it.name,
        sku: it.sku,
        unit: it.unit,
        default_price_minor: it.default_price_minor,
        default_tax_rate_id: it.default_tax_rate_id,
        tracked: it.tracked,
        reorder_point: it.reorder_point,
      },
      price: minorToInput(it.default_price_minor),
    });
  }
  async function onSubmit() {
    if (!draft || !draft.input.name.trim()) return;
    const input: ItemInput = { ...draft.input, default_price_minor: parseMoney(draft.price) ?? 0 };
    if (draft.id === null) await createMut.mutateAsync(input);
    else await updateMut.mutateAsync({ id: draft.id, input });
    setDraft(null);
  }
  function field<K extends keyof ItemInput>(key: K, value: ItemInput[K]) {
    setDraft((d) => (d ? { ...d, input: { ...d.input, [key]: value } } : d));
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Catalog"
        description="The products and services you sell — with live stock for tracked products."
        actions={
          <Button onClick={() => setDraft({ ...EMPTY })}>
            <Plus className="size-4" /> Add item
          </Button>
        }
      />

      {draft && (
        <Card>
          <CardContent className="grid gap-4 pt-6 sm:grid-cols-2">
            <Field label="Type">
              {(p) => (
                <Select {...p} value={draft.input.kind} onChange={(e) => field("kind", e.target.value)}>
                  <option value="product">Product (tracked stock)</option>
                  <option value="service">Service</option>
                </Select>
              )}
            </Field>
            <Field label="Name" required>
              {(p) => <Input {...p} value={draft.input.name} onChange={(e) => field("name", e.target.value)} />}
            </Field>
            <Field label="SKU">
              {(p) => <Input {...p} value={draft.input.sku} onChange={(e) => field("sku", e.target.value)} />}
            </Field>
            <Field label="Unit">
              {(p) => <Input {...p} value={draft.input.unit} onChange={(e) => field("unit", e.target.value)} />}
            </Field>
            <Field label="Default price">
              {(p) => (
                <Input
                  {...p}
                  inputMode="decimal"
                  value={draft.price}
                  onChange={(e) => setDraft((d) => (d ? { ...d, price: e.target.value } : d))}
                />
              )}
            </Field>
            <Field label="Default tax">
              {(p) => (
                <Select
                  {...p}
                  value={draft.input.default_tax_rate_id ?? ""}
                  onChange={(e) => field("default_tax_rate_id", e.target.value ? Number(e.target.value) : null)}
                >
                  <option value="">— None —</option>
                  {taxQ.data?.map((r) => (
                    <option key={r.id} value={r.id}>
                      {r.name}
                    </option>
                  ))}
                </Select>
              )}
            </Field>
            {draft.input.kind === "product" && (
              <Field label="Reorder point" hint="Low-stock alert at or below this">
                {(p) => (
                  <Input
                    {...p}
                    inputMode="numeric"
                    value={draft.input.reorder_point ?? ""}
                    onChange={(e) => field("reorder_point", e.target.value ? Number(e.target.value) : null)}
                  />
                )}
              </Field>
            )}
            <div className="flex gap-2 sm:col-span-2">
              <Button onClick={onSubmit} disabled={!draft.input.name.trim()}>
                {draft.id === null ? "Create" : "Save"}
              </Button>
              <Button variant="ghost" onClick={() => setDraft(null)}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      <div className="max-w-sm">
        <label htmlFor="item-search" className="sr-only">
          Search catalog
        </label>
        <Input
          id="item-search"
          placeholder="Search name or SKU…"
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
          title="No items yet"
          description="Add the products and services you sell."
          action={
            <Button onClick={() => setDraft({ ...EMPTY })}>
              <Plus className="size-4" /> Add item
            </Button>
          }
        />
      ) : (
        <Card className="overflow-hidden py-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="pl-4">Name</TableHead>
                <TableHead>Type</TableHead>
                <TableHead className="text-right">Price</TableHead>
                <TableHead>Stock</TableHead>
                <TableHead>
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {listQ.data?.map((it) => {
                const low = it.tracked && it.reorder_point !== null && it.qty_on_hand <= it.reorder_point;
                return (
                  <TableRow key={it.id}>
                    <TableCell className="pl-4 font-medium">{it.name}</TableCell>
                    <TableCell>{it.kind === "product" ? "Product" : "Service"}</TableCell>
                    <TableCell className="text-right tabular-nums">{money(it.default_price_minor)}</TableCell>
                    <TableCell>
                      {it.tracked ? (
                        <span className="flex items-center gap-2">
                          {it.qty_on_hand}
                          {low && <Badge variant="warning">Low</Badge>}
                        </span>
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </TableCell>
                    <TableCell className="pr-4 text-right whitespace-nowrap">
                      {it.tracked && (
                        <Button variant="ghost" size="sm" onClick={() => setAdjusting(it)}>
                          Adjust
                        </Button>
                      )}
                      <Button variant="ghost" size="sm" onClick={() => startEdit(it)}>
                        Edit
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => setDeleting(it)}>
                        Delete
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Card>
      )}

      {adjusting && (
        <AdjustStockDialog
          item={adjusting}
          pending={adjustMut.isPending}
          onClose={() => setAdjusting(null)}
          onSubmit={async (delta, note) => {
            await adjustMut.mutateAsync({ id: adjusting.id, delta, note });
            setAdjusting(null);
          }}
        />
      )}

      <ConfirmDialog
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        onConfirm={async () => {
          if (deleting) await deleteMut.mutateAsync(deleting.id);
          setDeleting(null);
        }}
        title={`Delete ${deleting?.name ?? "item"}?`}
        description="Existing documents keep their lines; the item just leaves the catalog."
        confirmLabel="Delete item"
        destructive
        pending={deleteMut.isPending}
      />
    </div>
  );
}

function AdjustStockDialog({
  item,
  pending,
  onClose,
  onSubmit,
}: {
  item: Item;
  pending: boolean;
  onClose: () => void;
  onSubmit: (delta: number, note: string) => void;
}) {
  const [delta, setDelta] = useState("");
  const [note, setNote] = useState("manual adjustment");
  const parsed = Number(delta);
  const valid = Number.isInteger(parsed) && parsed !== 0;

  return (
    <Dialog
      open
      onClose={onClose}
      title={`Adjust stock — ${item.name}`}
      description={`Currently ${item.qty_on_hand} on hand. Use a negative number to remove stock.`}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button disabled={!valid || pending} onClick={() => valid && onSubmit(parsed, note)}>
            {pending ? "Saving…" : "Adjust stock"}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <Field label="Change" hint="e.g. 10 received, or -3 damaged" required>
          {(p) => <Input {...p} inputMode="numeric" value={delta} onChange={(e) => setDelta(e.target.value)} />}
        </Field>
        <Field label="Note">
          {(p) => <Input {...p} value={note} onChange={(e) => setNote(e.target.value)} />}
        </Field>
      </div>
    </Dialog>
  );
}
