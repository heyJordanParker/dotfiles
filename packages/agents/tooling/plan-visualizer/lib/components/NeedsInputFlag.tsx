/**
 * NeedsInputFlag — the honest gap marker. A generating agent needs a specific
 * (a real file path, a column name, a chosen value) but the source proposal only
 * names a concept. Rather than invent the value, it flags exactly what is
 * missing for the orchestrator to fill. One amber chip + one line naming the
 * missing specific — never a placeholder dressed as a fact.
 *
 * Amber is the `changed` hue, reused deliberately: a gap is the shape still
 * moving. It stays inside the one-color-one-meaning vocabulary; no new hue.
 */
import type { NeedsInput } from "../model";

export function NeedsInputFlag({ gap }: { gap: NeedsInput }) {
  return (
    <div className="needs-input" role="note">
      <span className="needs-input__chip">needs orchestrator input</span>
      <p className="needs-input__body">
        <span className="needs-input__missing">{gap.missing}</span>
        {gap.knownInstead && (
          <span className="needs-input__known">
            {" "}
            — the proposal pins only {gap.knownInstead}
          </span>
        )}
      </p>
    </div>
  );
}
