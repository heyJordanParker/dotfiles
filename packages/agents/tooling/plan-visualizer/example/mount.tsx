/**
 * Shared mount — every example calls this with its own model. One renderer,
 * three proposals. Keeping the mount here means each example entry is two lines
 * (import the model, mount it), and the singlefile build inlines the same
 * library + styles into each standalone .html.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ReviewDocument } from "@lib/index";
import type { ReviewModel } from "@lib/index";
import "@lib/styles/index.css";

export function mount(model: ReviewModel) {
  const el = document.getElementById("root");
  if (!el) throw new Error("missing #root");
  // The document title — the browser tab and the title a reader sees — is the
  // plan's own headline, not the run/file name the static HTML shell carries as
  // a placeholder. The model owns it; the build can't (the shell is written
  // before the model is loaded), so set it here where the model is in hand.
  document.title = model.title;
  createRoot(el).render(
    <StrictMode>
      <ReviewDocument model={model} />
    </StrictMode>
  );
}
