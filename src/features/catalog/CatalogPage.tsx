import { useEffect, useRef, useState } from "react";
import { Plus } from "lucide-react";

import { useUnsavedEdits } from "@/app/nav";
import { formatTimestamp } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { useIpcMutation, useIpcQuery } from "@/lib/useIpc";
import type { Item, ItemInput } from "@/lib/types";
import { minorToInput, parseMoney, parseWholeNumber, useMoneyFormat } from "@/lib/money";
import { taxLabel, toChoice } from "@/features/shared/lines";
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

/** Item being edited; price and reorder point stay as typed text until they're parsed on save. */
type Draft = {
  id: number | null;
  input: ItemInput;
  price: string;
  reorder: string;
  /** Stock has been recorded, so the item must stay a tracked product. */
  hasStock: boolean;
};

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
  reorder: "",
  hasStock: false,
};

function draftFor(it: Item): Draft {
  return {
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
    reorder: it.reorder_point === null ? "" : String(it.reorder_point),
    hasStock: it.has_movements,
  };
}

const ARCHIVED_KEY = ["items", { archived: true }];

export function CatalogPage() {
  const [search, setSearch] = useState("");
  // Each opened form gets a fresh key, so switching items starts from that item's values.
  const [editing, setEditing] = useState<{ key: number; draft: Draft } | null>(null);
  const [adjusting, setAdjusting] = useState<Item | null>(null);
  const [archiving, setArchiving] = useState<Item | null>(null);
  const [history, setHistory] = useState<Item | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const money = useMoneyFormat();

  const listQ = useIpcQuery(["items", search], () => ipc.listItems(search || undefined));
  const archivedQ = useIpcQuery(ARCHIVED_KEY, () => ipc.listArchivedItems());
  const archiveMut = useIpcMutation(
    (id: number) => ipc.deleteItem(id),
    [["items"], ["dashboard"]],
    { successMessage: "Item archived" },
  );
  const restoreMut = useIpcMutation(
    (id: number) => ipc.restoreItem(id),
    [["items"], ["dashboard"]],
    { successMessage: "Item restored" },
  );
  const adjustMut = useIpcMutation(
    (v: { id: number; delta: number; note: string }) => ipc.adjustStock(v.id, v.delta, v.note),
    [["items"], ["dashboard"]],
    { successMessage: "Stock adjusted" },
  );

  function open(draft: Draft) {
    setEditing((current) => ({ key: (current?.key ?? 0) + 1, draft }));
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Catalog"
        description="The products and services you sell — with live stock for tracked products."
        actions={
          <Button onClick={() => open({ ...EMPTY })}>
            <Plus className="size-4" /> Add item
          </Button>
        }
      />

      {editing && (
        <ItemForm key={editing.key} initial={editing.draft} onDone={() => setEditing(null)} />
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
      ) : listQ.data && listQ.data.length === 0 && search.trim() ? (
        <EmptyState
          title={`No items match “${search.trim()}”`}
          description="Check the spelling, or search by part of the name or SKU."
        />
      ) : listQ.data && listQ.data.length === 0 ? (
        <EmptyState
          title="No items yet"
          description="Add the products and services you sell."
          action={
            <Button onClick={() => open({ ...EMPTY })}>
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
                <TableHead>SKU</TableHead>
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
                const low =
                  it.tracked && it.reorder_point !== null && it.qty_on_hand <= it.reorder_point;
                return (
                  <TableRow key={it.id}>
                    <TableCell className="pl-4 font-medium">{it.name}</TableCell>
                    <TableCell className="font-mono text-xs">
                      {it.sku || <span className="text-muted-foreground">—</span>}
                    </TableCell>
                    <TableCell>{it.kind === "product" ? "Product" : "Service"}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {money(it.default_price_minor)}
                    </TableCell>
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
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Adjust stock for ${it.name}`}
                          onClick={() => setAdjusting(it)}
                        >
                          Adjust
                        </Button>
                      )}
                      {(it.tracked || it.has_movements) && (
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Stock history for ${it.name}`}
                          onClick={() => setHistory(it)}
                        >
                          History
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="sm"
                        aria-label={`Edit ${it.name}`}
                        onClick={() => open(draftFor(it))}
                      >
                        Edit
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        aria-label={`Archive ${it.name}`}
                        onClick={() => setArchiving(it)}
                      >
                        Archive
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Card>
      )}

      {archivedQ.data && archivedQ.data.length > 0 && (
        <div>
          <Button
            variant="link"
            className="h-auto px-0"
            aria-expanded={showArchived}
            aria-controls="archived-items"
            onClick={() => setShowArchived((v) => !v)}
          >
            {showArchived ? "Hide" : "Show"} archived items ({archivedQ.data.length})
          </Button>
          {showArchived && (
            <ul id="archived-items" className="mt-2 divide-y rounded-lg border bg-card">
              {archivedQ.data.map((it) => (
                <li key={it.id} className="flex items-center justify-between gap-3 px-4 py-2">
                  <span className="text-sm">
                    {it.name}
                    {it.sku && (
                      <span className="ml-2 font-mono text-xs text-muted-foreground">{it.sku}</span>
                    )}
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    aria-label={`Restore ${it.name}`}
                    onClick={() => restoreMut.mutate(it.id)}
                    loading={restoreMut.isPending && restoreMut.variables === it.id}
                    loadingLabel="Restoring…"
                  >
                    Restore
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {adjusting && (
        <AdjustStockDialog
          item={adjusting}
          pending={adjustMut.isPending}
          onClose={() => setAdjusting(null)}
          onSubmit={async (delta, note) => {
            try {
              await adjustMut.mutateAsync({ id: adjusting.id, delta, note });
              setAdjusting(null);
            } catch {
              // useIpcMutation has already shown the backend error.
            }
          }}
        />
      )}

      {history && <StockHistoryDialog item={history} onClose={() => setHistory(null)} />}

      <ConfirmDialog
        open={archiving !== null}
        onClose={() => !archiveMut.isPending && setArchiving(null)}
        onConfirm={async () => {
          if (!archiving) return;
          try {
            await archiveMut.mutateAsync(archiving.id);
            setArchiving(null);
          } catch {
            // Keep the confirmation open so the user can act on the backend explanation.
          }
        }}
        title={`Archive ${archiving?.name ?? "item"}?`}
        description="It leaves the catalog and can't be added to new lines; documents that already use it keep their details. It needs zero stock and no open drafts, quotes, jobs or schedules. You can restore it later."
        confirmLabel="Archive item"
        destructive
        pending={archiveMut.isPending}
      />
    </div>
  );
}

function ItemForm({ initial, onDone }: { initial: Draft; onDone: () => void }) {
  const taxQ = useIpcQuery(["tax-rates"], () => ipc.listTaxRates());
  const createMut = useIpcMutation(
    (i: ItemInput) => ipc.createItem(i),
    [["items"], ["dashboard"]],
    {
      successMessage: "Item added",
    },
  );
  const updateMut = useIpcMutation(
    (v: { id: number; input: ItemInput }) => ipc.updateItem(v.id, v.input),
    [["items"], ["dashboard"]],
    { successMessage: "Item saved" },
  );
  const [draft, setDraft] = useState<Draft>(initial);
  useUnsavedEdits(draft);
  const formRef = useRef<HTMLFormElement>(null);

  useEffect(() => {
    // The form opens above the list, so bring it into view and start at its first field.
    const form = formRef.current;
    form?.scrollIntoView?.({ block: "nearest" });
    form
      ?.querySelector<HTMLElement>("input:not([disabled]), select:not([disabled])")
      ?.focus({ preventScroll: true });
  }, []);

  const draftPrice = parseMoney(draft.price);
  const priceError =
    draftPrice === null || draftPrice < 0
      ? "Enter a non-negative amount with at most two decimal places"
      : undefined;
  const tracksStock = draft.input.kind === "product";
  const draftReorder =
    draft.reorder.trim() !== ""
      ? parseWholeNumber(draft.reorder, { min: 0, max: 1_000_000_000 })
      : null;
  const reorderError =
    tracksStock && draft.reorder.trim() !== "" && draftReorder === null
      ? "Enter a whole number of units (0 or more), or leave blank for no alert"
      : undefined;
  const valid =
    Boolean(draft.input.name.trim()) &&
    Boolean(draft.input.unit.trim()) &&
    !priceError &&
    !reorderError;
  const pending = createMut.isPending || updateMut.isPending;

  async function onSubmit() {
    if (!valid || pending || draftPrice === null) return;
    const input: ItemInput = {
      ...draft.input,
      default_price_minor: draftPrice,
      reorder_point: tracksStock ? draftReorder : null,
    };
    try {
      if (draft.id === null) await createMut.mutateAsync(input);
      else await updateMut.mutateAsync({ id: draft.id, input });
      onDone();
    } catch {
      // useIpcMutation has already shown the backend error.
    }
  }
  function field<K extends keyof ItemInput>(key: K, value: ItemInput[K]) {
    setDraft((d) => ({ ...d, input: { ...d.input, [key]: value } }));
  }

  return (
    <Card>
      <form
        ref={formRef}
        noValidate
        aria-label={draft.id === null ? "New item" : `Edit ${initial.input.name}`}
        className="scroll-mt-6"
        onSubmit={(e) => {
          e.preventDefault();
          void onSubmit();
        }}
      >
        <CardContent className="grid gap-4 pt-6 sm:grid-cols-2">
          <Field
            label="Type"
            hint={
              draft.hasStock
                ? "Stock has been recorded for this product, so it stays a tracked product."
                : undefined
            }
          >
            {(p) => (
              <Select
                {...p}
                disabled={draft.hasStock}
                value={draft.input.kind}
                onChange={(e) => {
                  const kind = e.target.value;
                  setDraft((current) => ({
                    ...current,
                    input: {
                      ...current.input,
                      kind,
                      tracked: kind === "product",
                      reorder_point: kind === "product" ? current.input.reorder_point : null,
                    },
                  }));
                }}
              >
                <option value="product">Product (tracked stock)</option>
                <option value="service">Service</option>
              </Select>
            )}
          </Field>
          <Field label="Name" required>
            {(p) => (
              <Input
                {...p}
                value={draft.input.name}
                onChange={(e) => field("name", e.target.value)}
              />
            )}
          </Field>
          <Field label="SKU">
            {(p) => (
              <Input
                {...p}
                value={draft.input.sku}
                onChange={(e) => field("sku", e.target.value)}
              />
            )}
          </Field>
          <Field label="Unit" required>
            {(p) => (
              <Input
                {...p}
                value={draft.input.unit}
                onChange={(e) => field("unit", e.target.value)}
              />
            )}
          </Field>
          <Field label="Default price" error={priceError}>
            {(p) => (
              <Input
                {...p}
                inputMode="decimal"
                value={draft.price}
                onChange={(e) => setDraft((d) => ({ ...d, price: e.target.value }))}
              />
            )}
          </Field>
          <Field label="Default tax">
            {(p) => (
              <Select
                {...p}
                value={draft.input.default_tax_rate_id ?? ""}
                onChange={(e) =>
                  field("default_tax_rate_id", e.target.value ? Number(e.target.value) : null)
                }
              >
                <option value="">— None —</option>
                {taxQ.data?.map((r) => (
                  <option key={r.id} value={r.id}>
                    {taxLabel(toChoice(r))}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          {tracksStock && (
            <Field
              label="Reorder point"
              hint="Low-stock alert at or below this. Leave blank for no alert."
              error={reorderError}
            >
              {(p) => (
                <Input
                  {...p}
                  inputMode="numeric"
                  value={draft.reorder}
                  onChange={(e) => setDraft((d) => ({ ...d, reorder: e.target.value }))}
                />
              )}
            </Field>
          )}
          <div className="flex gap-2 sm:col-span-2">
            <Button type="submit" disabled={!valid} loading={pending} loadingLabel="Saving…">
              {draft.id === null ? "Create" : "Save"}
            </Button>
            <Button variant="ghost" onClick={onDone}>
              Cancel
            </Button>
          </div>
        </CardContent>
      </form>
    </Card>
  );
}

const MOVEMENT_REASONS: Record<string, string> = {
  sale: "Sold",
  return: "Returned (invoice voided)",
  adjustment: "Adjusted",
  receipt: "Received",
};

function StockHistoryDialog({ item, onClose }: { item: Item; onClose: () => void }) {
  const historyQ = useIpcQuery(["items", item.id, "movements"], () => ipc.itemMovements(item.id));
  return (
    <Dialog
      open
      onClose={onClose}
      title={`Stock history — ${item.name}`}
      description={`${item.qty_on_hand} on hand now. Newest changes first.`}
      className="max-w-2xl"
      footer={
        <Button variant="outline" onClick={onClose}>
          Close
        </Button>
      }
    >
      {historyQ.isLoading ? (
        <Loading label="Loading history…" />
      ) : historyQ.error ? (
        <ErrorState error={historyQ.error} onRetry={() => historyQ.refetch()} />
      ) : historyQ.data && historyQ.data.length === 0 ? (
        <p className="text-sm text-muted-foreground">No stock changes recorded yet.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>When</TableHead>
              <TableHead className="text-right">Change</TableHead>
              <TableHead>Reason</TableHead>
              <TableHead>Details</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {historyQ.data?.map((m) => (
              <TableRow key={m.id}>
                <TableCell className="whitespace-nowrap tabular-nums">
                  {formatTimestamp(m.occurred_at)}
                </TableCell>
                <TableCell className="text-right tabular-nums">
                  {m.qty_delta > 0 ? `+${m.qty_delta}` : m.qty_delta}
                </TableCell>
                <TableCell>{MOVEMENT_REASONS[m.reason] ?? m.reason}</TableCell>
                <TableCell className="max-w-56 text-muted-foreground">
                  {[m.invoice_number ? `Invoice ${m.invoice_number}` : null, m.note || null]
                    .filter(Boolean)
                    .join(" · ") || "—"}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </Dialog>
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
  const parsed = parseWholeNumber(delta, { min: -1_000_000_000, max: 1_000_000_000 });
  const valid = parsed !== null && parsed !== 0;
  const error =
    delta.trim() && !valid ? "Enter a whole number other than zero, e.g. 10 or -3" : undefined;

  return (
    <Dialog
      open
      onClose={() => !pending && onClose()}
      title={`Adjust stock — ${item.name}`}
      description={`Currently ${item.qty_on_hand} on hand. Use a negative number to remove stock.`}
      onSubmit={() => {
        if (valid && !pending && parsed !== null) onSubmit(parsed, note);
      }}
      footer={
        <>
          <Button variant="outline" onClick={onClose} disabled={pending}>
            Cancel
          </Button>
          <Button type="submit" disabled={!valid} loading={pending} loadingLabel="Saving…">
            Adjust stock
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <Field
          label="Change"
          required
          error={error}
          hint={
            valid && parsed !== null
              ? `New quantity: ${item.qty_on_hand + parsed}`
              : "e.g. 10 received, or -3 damaged"
          }
        >
          {(p) => (
            <Input
              {...p}
              inputMode="numeric"
              value={delta}
              onChange={(e) => setDelta(e.target.value)}
            />
          )}
        </Field>
        <Field label="Note">
          {(p) => <Input {...p} value={note} onChange={(e) => setNote(e.target.value)} />}
        </Field>
      </div>
    </Dialog>
  );
}
