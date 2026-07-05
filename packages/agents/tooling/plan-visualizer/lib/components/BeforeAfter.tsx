/**
 * BeforeAfter — the section that maps the old architecture onto the new one
 * component by component, IN PLACE. Each facet is one component or layer (the
 * store, the check, the audiences, a gate, the field read, a link tier). It
 * renders ONE heading, ONE explanation slot, and ONE body slot; an in-place
 * toggle flips those slots between the component's before-state and its
 * after-state on the same spot. There is no linear before-block then after-block
 * — the reviewer sits on a component and flips its two states.
 *
 * Each facet's local phase defaults to the document-wide phase and re-syncs when
 * the global phase moves, so flipping the loud hero/rail control flips every
 * facet at once; a reviewer can also pin one facet to a phase locally without
 * moving the rest of the document.
 *
 * The body swaps like for like: a before-code flips to after-code through the
 * same viewer, a before-DDL to after-DDL, a before-graph to after-graph — so the
 * diagram/DDL/code stays the same surface and only its contents change.
 */
import { useEffect, useState } from "react";
import type {
  BeforeAfterFacet,
  PhaseVariant,
  ReviewModel,
} from "../model";
import { usePhase, PHASE_LABEL, type Phase } from "../phase";
import { groupBy } from "../group";
import { Annotatable, StateBadge } from "./primitives";
import { CodeBlock } from "./CodeBlock";
import { RelationshipGraph } from "./RelationshipGraph";
import { NeedsInputFlag } from "./NeedsInputFlag";

/** The body of one phase variant — code, DDL, or a small relationship diagram. */
function VariantBody({
  variant,
  model,
}: {
  variant: PhaseVariant;
  model: ReviewModel;
}) {
  const diagram = (variant.diagramNodeIds ?? [])
    .map((id) => model.nodes[id])
    .filter((n): n is NonNullable<typeof n> => Boolean(n));

  return (
    <div className="ba-facet__variant">
      <p className="ba-facet__explain">{variant.explain}</p>
      {variant.notes && variant.notes.length > 0 && (
        <ul className="ba-facet__notes">
          {variant.notes.map((n, i) => (
            <li key={i}>{n}</li>
          ))}
        </ul>
      )}
      {diagram.length > 0 && (
        <div className="ba-facet__diagram">
          <RelationshipGraph nodes={diagram} compact model={model} />
        </div>
      )}
      {variant.code && (
        <CodeBlock
          code={variant.code.code}
          language={variant.code.language}
          label={variant.code.label}
        />
      )}
      {variant.ddl && (
        <CodeBlock
          code={variant.ddl.code}
          language={variant.ddl.language}
          label={variant.ddl.label}
        />
      )}
    </div>
  );
}

function Facet({
  facet,
  model,
}: {
  facet: BeforeAfterFacet;
  model: ReviewModel;
}) {
  const { phase: globalPhase } = usePhase();
  // The facet starts on the document-wide phase, and re-syncs whenever the global
  // control moves — flipping the hero/rail toggle flips every facet at once. Once
  // synced, the reviewer can still flip this one facet locally without moving the
  // rest of the document, until the global phase moves again.
  const [phase, setPhase] = useState<Phase>(globalPhase);
  useEffect(() => setPhase(globalPhase), [globalPhase]);

  const variant = phase === "before" ? facet.before : facet.after;

  return (
    <Annotatable
      target={{
        surfaceId: `beforeAfter:${facet.id}`,
        label: facet.title,
        kind: "concept",
        facts: [
          facet.summary,
          `Before: ${facet.before.explain}`,
          `After: ${facet.after.explain}`,
        ],
      }}
    >
      <article
        className="ba-facet"
        data-state={facet.state}
        data-phase={phase}
      >
        <header className="ba-facet__head">
          <div className="ba-facet__heading">
            <span className="ba-facet__title">{facet.title}</span>
            <StateBadge state={facet.state} />
            {facet.needsInput && (
              <span className="ba-facet__flag">needs your input</span>
            )}
          </div>
          <p className="ba-facet__summary">{facet.summary}</p>
        </header>

        <div
          className="ba-facet__switch"
          role="group"
          aria-label={`Show ${facet.title} before or after this run`}
        >
          {(["before", "after"] as const).map((p) => (
            <button
              key={p}
              type="button"
              className="ba-facet__switch-btn"
              data-phase={p}
              data-active={phase === p}
              aria-pressed={phase === p}
              onClick={() => setPhase(p)}
            >
              {p === "before" ? "Before" : "After"}
            </button>
          ))}
        </div>

        <div className="ba-facet__phase-label" data-phase={phase}>
          {PHASE_LABEL[phase]}
        </div>

        <VariantBody variant={variant} model={model} />

        {facet.needsInput && phase === "after" && (
          <NeedsInputFlag gap={facet.needsInput} />
        )}
      </article>
    </Annotatable>
  );
}

export function BeforeAfter({
  facets,
  model,
}: {
  facets: readonly BeforeAfterFacet[];
  model: ReviewModel;
}) {
  const bands = groupBy(facets, (f) => f.group);
  if (bands.length === 1) {
    return (
      <div className="ba">
        {facets.map((f) => (
          <Facet key={f.id} facet={f} model={model} />
        ))}
      </div>
    );
  }
  return (
    <div className="ba ba--banded">
      {bands.map((band, i) => (
        <div key={band.caption ?? i} className="ba__band">
          {band.caption && (
            <span className="ba__band-caption">{band.caption}</span>
          )}
          <div className="ba">
            {band.items.map((f) => (
              <Facet key={f.id} facet={f} model={model} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
