import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useEffect } from "react";

import userEvent from "@testing-library/user-event";

import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { ToastProvider, useToast } from "@/components/ui/toast";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";

// jsdom has no top layer; model just enough of <dialog> for the component's behaviour.
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function ErrorWhileOpen() {
  const toast = useToast();
  useEffect(() => toast("error", "customer cannot be deleted"), [toast]);
  return null;
}

describe("Dialog", () => {
  it("renders error toasts inside the open dialog so they are never hidden behind it", () => {
    render(
      <ToastProvider>
        <Dialog open onClose={() => {}} title="Delete customer?">
          <ErrorWhileOpen />
        </Dialog>
      </ToastProvider>,
    );
    const dialog = screen.getByRole("dialog", { hidden: true });
    const message = screen.getByText("customer cannot be deleted");
    expect(dialog).toContainElement(message);
    expect(screen.getByRole("button", { name: "Dismiss notification" })).toBeInTheDocument();
  });

  it("names and describes itself and focuses the first field", () => {
    render(
      <Dialog open onClose={() => {}} title="Record payment" description="Up to the balance.">
        <Field label="Amount" required>
          {(p) => <Input {...p} />}
        </Field>
      </Dialog>,
    );
    const dialog = screen.getByRole("dialog", { hidden: true });
    expect(dialog).toHaveAccessibleName("Record payment");
    expect(dialog).toHaveAccessibleDescription("Up to the balance.");
    const amount = screen.getByLabelText(/Amount/);
    expect(amount).toHaveFocus();
    expect(amount).toHaveAttribute("aria-required", "true");
  });

  it("ignores a drag that ends on the backdrop but closes on a real backdrop click", () => {
    const onClose = vi.fn();
    render(
      <Dialog open onClose={onClose} title="Adjust stock">
        <Field label="Change">{(p) => <Input {...p} />}</Field>
      </Dialog>,
    );
    const dialog = screen.getByRole("dialog", { hidden: true });
    fireEvent.mouseDown(screen.getByLabelText("Change"));
    fireEvent.click(dialog);
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.mouseDown(dialog);
    fireEvent.click(dialog);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("submits from Enter in a field, but never from its other buttons", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    const onOther = vi.fn();
    render(
      <Dialog
        open
        onClose={() => {}}
        title="Adjust stock"
        onSubmit={onSubmit}
        footer={<Button type="submit">Adjust stock</Button>}
      >
        <Field label="Change">{(p) => <Input {...p} />}</Field>
        <Button onClick={onOther}>Suggest</Button>
      </Dialog>,
    );
    await user.click(screen.getByRole("button", { name: "Suggest", hidden: true }));
    expect(onOther).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
    await user.type(screen.getByLabelText("Change"), "5{Enter}");
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("keeps success toasts briefly and errors until dismissed", () => {
    vi.useFakeTimers();
    function Push() {
      const toast = useToast();
      useEffect(() => {
        toast("success", "Saved");
        toast("error", "Failed");
      }, [toast]);
      return null;
    }
    render(
      <ToastProvider>
        <Push />
      </ToastProvider>,
    );
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    expect(screen.queryByText("Saved")).not.toBeInTheDocument();
    expect(screen.getByText("Failed")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss notification" }));
    expect(screen.queryByText("Failed")).not.toBeInTheDocument();
  });
});
