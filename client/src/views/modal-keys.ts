// Keyboard dismissal for the POS-loop modals (boleta, caja, devolución,
// cliente). A real cashier drives the counter keyboard-only — the modals used to
// be dismissable only by clicking the backdrop or the Cancel button, trapping a
// mouseless operator. `bindModalKeys` wires Escape-to-close (and an optional
// Enter-to-confirm) on the document for a modal's lifetime and returns a detach
// fn the caller folds into its own close() so the listener never outlives the
// modal (no stacking, no leak).
export function bindModalKeys(close: () => void, onEnter?: () => void): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Enter" && onEnter) {
      e.preventDefault();
      onEnter();
    }
  };
  document.addEventListener("keydown", onKey);
  return () => document.removeEventListener("keydown", onKey);
}
