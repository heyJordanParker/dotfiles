/**
 * ConstraintList — standing constraints, the inverse of decisions. A decision
 * overturns a prior ruling; a constraint is a boundary deliberately NOT crossed —
 * "what stays, on purpose". Each carries what it protects and the impact if it
 * were crossed, plus an optional small relationship diagram of the nodes it
 * spans. Read through the `kept` neutral hue — the unchanged-by-design edges.
 *
 * Two terse lines, one color system, no boxes or rails — the same grammar as a
 * decision, so the two read as a matched pair. Bands by the second axis when the
 * constraints carry a `group`.
 */
import type { Constraint, ReviewModel } from "../model";
import { groupBy } from "../group";
import { Annotatable } from "./primitives";
import { RelationshipGraph } from "./RelationshipGraph";

function ConstraintCard({
  constraint,
  model,
}: {
  constraint: Constraint;
  model: ReviewModel;
}) {
  const diagramNodes = (constraint.diagramNodeIds ?? [])
    .map((id) => model.nodes[id])
    .filter((n): n is NonNullable<typeof n> => Boolean(n));

  return (
    <Annotatable
      target={{
        surfaceId: `constraint:${constraint.id}`,
        label: constraint.title,
        kind: "constraint",
        facts: [
          `Protects: ${constraint.protects}`,
          `Impact if crossed: ${constraint.impactIfCrossed}`,
        ],
      }}
    >
      <article className="constraint" data-state="kept">
        <header className="constraint__head">
          <span className="constraint__hold" aria-hidden="true" />
          <h3 className="constraint__title">{constraint.title}</h3>
        </header>

        <p className="constraint__line">
          <span className="constraint__lead">protects</span>{" "}
          {constraint.protects}
        </p>
        <p className="constraint__line">
          <span className="constraint__lead">if crossed</span>{" "}
          {constraint.impactIfCrossed}
        </p>

        {diagramNodes.length > 0 && (
          <RelationshipGraph nodes={diagramNodes} compact model={model} />
        )}
      </article>
    </Annotatable>
  );
}

export function ConstraintList({
  constraints,
  model,
}: {
  constraints: readonly Constraint[];
  model: ReviewModel;
}) {
  const bands = groupBy(constraints, (c) => c.group);
  if (bands.length === 1) {
    return (
      <div className="constraints">
        {constraints.map((c) => (
          <ConstraintCard key={c.id} constraint={c} model={model} />
        ))}
      </div>
    );
  }
  return (
    <div className="constraints constraints--banded">
      {bands.map((band, i) => (
        <div key={band.caption ?? i} className="constraints__band">
          {band.caption && (
            <span className="constraints__caption">{band.caption}</span>
          )}
          <div className="constraints">
            {band.items.map((c) => (
              <ConstraintCard key={c.id} constraint={c} model={model} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
