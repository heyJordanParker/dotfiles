/**
 * Phase — the document-wide before/after axis. This is the one mechanism that
 * makes the review readable component-by-component: every before/after surface
 * reads ONE shared phase, so a reviewer flips the whole document to "before" or
 * "after" once and then walks each component and layer seeing the matching state
 * in place, on the same diagram and the same explanation — never a linear
 * before-section followed by an after-section.
 *
 * The global phase is the default each facet starts on. A facet may also flip
 * itself locally (per component, per layer) without moving the global axis, so a
 * reviewer can pin one component to "after" while the rest of the document reads
 * "before". Setting the global phase re-syncs every facet that has not been
 * pinned away from it — the facet's own effect handles that.
 */
import { createContext, useContext, useState, type ReactNode } from "react";

export type Phase = "before" | "after";

export const PHASE_LABEL: Record<Phase, string> = {
  before: "Before this run",
  after: "After this run",
};

interface PhaseState {
  readonly phase: Phase;
  readonly setPhase: (phase: Phase) => void;
}

const PhaseContext = createContext<PhaseState | null>(null);

export function PhaseProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>("after");
  return (
    <PhaseContext.Provider value={{ phase, setPhase }}>
      {children}
    </PhaseContext.Provider>
  );
}

/** The document-wide phase. Falls back to "after" outside a provider. */
export function usePhase(): PhaseState {
  return useContext(PhaseContext) ?? { phase: "after", setPhase: () => {} };
}

/**
 * The segmented before/after control. Reads and drives the document-wide phase.
 * Rendered in the hero (the loud one) and in the sticky rail (the always-there
 * one) — both drive the same context, so either flips the whole document.
 */
export function PhaseToggle({
  variant = "hero",
}: {
  variant?: "hero" | "rail";
}) {
  const { phase, setPhase } = usePhase();
  return (
    <div
      className="phase-toggle"
      data-variant={variant}
      role="group"
      aria-label="Show the architecture before or after this run"
    >
      {(["before", "after"] as const).map((p) => (
        <button
          key={p}
          type="button"
          className="phase-toggle__btn"
          data-phase={p}
          data-active={phase === p}
          aria-pressed={phase === p}
          onClick={() => setPhase(p)}
        >
          {p === "before" ? "Before" : "After"}
        </button>
      ))}
    </div>
  );
}
