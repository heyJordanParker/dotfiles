/**
 * DecisionList — decisions carried by code, not prose. Each shows the tangible
 * change AS CODE through Pierre (a block or a before/after diff) and, where it
 * helps, a small relationship diagram. The only words are the title, a one-line
 * prior-ruling, and a one-line impact-if-overruled. The only colors are the one
 * semantic state system — a flagged decision reads through the amber `changed`
 * accent and the flag chip, never a per-label rainbow.
 */
import type { Decision, NeedsInput, ReviewModel } from "../model";
import { groupBy } from "../group";
import { Annotatable } from "./primitives";
import { CodeBlock } from "./CodeBlock";
import { CodeDiff } from "./CodeDiff";
import { RelationshipGraph } from "./RelationshipGraph";
import { NeedsInputFlag } from "./NeedsInputFlag";

/** The decision's gap, when it names one (vs the bare `true` flag). */
function decisionGap(decision: Decision): NeedsInput | null {
  return decision.needsInput && typeof decision.needsInput === "object"
    ? decision.needsInput
    : null;
}

function DecisionChange({ decision }: { decision: Decision }) {
  if (!decision.change) return null;
  if (decision.change.mode === "diff") {
    return <CodeDiff change={decision.change.diff} />;
  }
  return (
    <CodeBlock
      code={decision.change.block.code}
      language={decision.change.block.language}
      label={decision.change.block.label}
    />
  );
}

function DecisionCard({
  decision,
  model,
}: {
  decision: Decision;
  model: ReviewModel;
}) {
  const diagramNodes = (decision.diagramNodeIds ?? [])
    .map((id) => model.nodes[id])
    .filter((n): n is NonNullable<typeof n> => Boolean(n));
  const gap = decisionGap(decision);

  return (
    <Annotatable
      target={{
        surfaceId: `decision:${decision.id}`,
        label: decision.title,
        kind: "decision",
        facts: [
          `Prior ruling: ${decision.priorRuling}`,
          `Impact if overruled: ${decision.impactIfOverruled}`,
          ...(typeof decision.confidence === "number"
            ? [`Confidence: ${decision.confidence}%`]
            : []),
          ...(decision.validation
            ? [`Proven by: ${decision.validation}`]
            : []),
          ...(gap
            ? [`Needs orchestrator input: ${gap.missing}`]
            : decision.needsInput
              ? ["Flagged: needs the architect's input"]
              : []),
        ],
      }}
    >
      <article
        className="decision"
        data-needs-input={Boolean(decision.needsInput)}
      >
        <header className="decision__head">
          <h3 className="decision__title">{decision.title}</h3>
          {typeof decision.confidence === "number" && (
            <span className="decision__confidence">
              {decision.confidence}%
            </span>
          )}
          {decision.needsInput && (
            <span className="decision__flag">needs your input</span>
          )}
        </header>

        <p className="decision__line">
          <span className="decision__lead">was</span> {decision.priorRuling}
        </p>
        <p className="decision__line">
          <span className="decision__lead">if overruled</span>{" "}
          {decision.impactIfOverruled}
        </p>
        {decision.validation && (
          <p className="decision__line decision__line--proven">
            <span className="decision__lead">proven by</span>{" "}
            {decision.validation}
          </p>
        )}

        {gap && <NeedsInputFlag gap={gap} />}

        {diagramNodes.length > 0 && (
          <RelationshipGraph nodes={diagramNodes} compact model={model} />
        )}
        <DecisionChange decision={decision} />
      </article>
    </Annotatable>
  );
}

export function DecisionList({
  decisions,
  model,
}: {
  decisions: readonly Decision[];
  model: ReviewModel;
}) {
  const bands = groupBy(decisions, (d) => d.group);
  if (bands.length === 1) {
    return (
      <div className="decisions">
        {decisions.map((d) => (
          <DecisionCard key={d.id} decision={d} model={model} />
        ))}
      </div>
    );
  }
  return (
    <div className="decisions decisions--banded">
      {bands.map((band, i) => (
        <div key={band.caption ?? i} className="decisions__band">
          {band.caption && (
            <span className="decisions__caption">{band.caption}</span>
          )}
          <div className="decisions">
            {band.items.map((d) => (
              <DecisionCard key={d.id} decision={d} model={model} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
