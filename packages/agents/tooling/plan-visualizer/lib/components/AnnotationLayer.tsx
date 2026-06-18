/**
 * AnnotationLayer — the annotate-then-copy UI. Two parts:
 *
 *  - The composer: a focused dialog that opens when a surface is clicked. The
 *    architect writes a note and saves. It shows exactly what target the note
 *    is attached to so there's no ambiguity about scope.
 *  - The handoff dock: a fixed panel listing every note, each with a one-click
 *    copy that yields self-contained text — the concrete target, its context,
 *    and the note — for pasting into an agent that never saw this document.
 *    "Copy all" yields the whole review as one handoff packet.
 */
import { useEffect, useRef, useState } from "react";
import { useAnnotations } from "../annotations";

function copyToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    return navigator.clipboard.writeText(text);
  }
  // Fallback for non-secure contexts / headless browsers.
  return new Promise((resolve) => {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    resolve();
  });
}

function Composer() {
  const { composing, close, save } = useAnnotations();
  const ref = useRef<HTMLTextAreaElement>(null);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    setDraft("");
    if (composing) {
      const t = setTimeout(() => ref.current?.focus(), 20);
      return () => clearTimeout(t);
    }
  }, [composing]);

  if (!composing) return null;

  return (
    <div className="composer-scrim" onClick={close}>
      <div
        className="composer"
        role="dialog"
        aria-label={`Comment on ${composing.label}`}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") close();
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) save(draft);
        }}
      >
        <header className="composer__head">
          <span className="composer__kind">{composing.kind}</span>
          <span className="composer__label">{composing.label}</span>
        </header>
        {composing.facts && composing.facts.length > 0 && (
          <ul className="composer__facts">
            {composing.facts.slice(0, 4).map((f, i) => (
              <li key={i}>{f}</li>
            ))}
          </ul>
        )}
        <textarea
          ref={ref}
          className="composer__input"
          placeholder="Your note for the agent that will act on this…"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          rows={4}
        />
        <footer className="composer__foot">
          <span className="composer__hint">⌘↵ to save</span>
          <div className="composer__actions">
            <button type="button" className="btn btn--ghost" onClick={close}>
              Cancel
            </button>
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => save(draft)}
              disabled={!draft.trim()}
            >
              Save comment
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

function HandoffDock() {
  const { annotations, buildCopyText, buildAllCopyText, remove, open } =
    useAnnotations();
  const [openDock, setOpenDock] = useState(true);
  const [copied, setCopied] = useState<string | null>(null);
  const list = Object.values(annotations);

  const flash = (id: string) => {
    setCopied(id);
    setTimeout(() => setCopied((c) => (c === id ? null : c)), 1600);
  };

  if (list.length === 0) return null;

  return (
    <aside className="dock" data-open={openDock}>
      <button
        type="button"
        className="dock__toggle"
        aria-expanded={openDock}
        onClick={() => setOpenDock((v) => !v)}
      >
        <span className="dock__toggle-count">{list.length}</span>
        <span className="dock__toggle-label">
          comment{list.length === 1 ? "" : "s"}
        </span>
        <span className="dock__toggle-caret" aria-hidden="true" />
      </button>

      {openDock && (
        <div className="dock__body">
          <header className="dock__head">
            <h2 className="dock__title">Comments to hand off</h2>
            <button
              type="button"
              className="btn btn--primary btn--sm"
              onClick={async () => {
                await copyToClipboard(buildAllCopyText());
                flash("__all__");
              }}
            >
              {copied === "__all__" ? "Copied all ✓" : "Copy all"}
            </button>
          </header>
          <ul className="dock__list">
            {list.map((ann) => (
              <li key={ann.surfaceId} className="dock-item">
                <div className="dock-item__head">
                  <span className="dock-item__kind">{ann.kind}</span>
                  <button
                    type="button"
                    className="dock-item__target"
                    onClick={() => open(ann)}
                    title="Edit this comment"
                  >
                    {ann.label}
                  </button>
                </div>
                <p className="dock-item__note">{ann.note}</p>
                <div className="dock-item__actions">
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={async () => {
                      await copyToClipboard(buildCopyText(ann.surfaceId));
                      flash(ann.surfaceId);
                    }}
                  >
                    {copied === ann.surfaceId ? "Copied ✓" : "Copy for agent"}
                  </button>
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm btn--danger"
                    onClick={() => remove(ann.surfaceId)}
                  >
                    Remove
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </aside>
  );
}

export function AnnotationLayer() {
  return (
    <>
      <Composer />
      <HandoffDock />
    </>
  );
}
