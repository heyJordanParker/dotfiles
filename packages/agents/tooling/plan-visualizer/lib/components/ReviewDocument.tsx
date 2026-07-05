/**
 * ReviewDocument — the top-level assembly, read problem by problem.
 *
 * The document is not a flat stream of sections. It opens with one overview,
 * then lists the problems the plan solves as collapsed cards; the reader expands
 * a problem to dig into its own overview and sections (progressive disclosure —
 * never everything at once). Validation is the closing region, at the very end,
 * never folded inside a problem and never mid-document.
 *
 * A sticky rail carries the problem index and the one-color legend. Every section
 * type still maps to one component; the problem layer only governs WHERE and WHEN
 * a section renders, never how.
 */
import { useState } from "react";
import type { ReviewModel, Problem, Section } from "../model";
import { STATE_ORDER, CHANGE_STATES } from "../model";
import { AnnotationProvider } from "../annotations";
import { PhaseProvider, PhaseToggle } from "../phase";
import { RelationshipGraph } from "./RelationshipGraph";
import { BeforeAfter } from "./BeforeAfter";
import { FileTree } from "./FileTree";
import { DatabasePanel } from "./DatabasePanel";
import { Collection } from "./Collection";
import { DecisionList } from "./DecisionList";
import { ConstraintList } from "./ConstraintList";
import { MatrixGrid } from "./MatrixGrid";
import { ChoiceList } from "./ChoiceList";
import { StateDot } from "./primitives";
import { AnnotationLayer } from "./AnnotationLayer";

/** Count of needs-input gaps a section carries — surfaced as the problem's flag. */
function sectionFlag(section: Section, model: ReviewModel): number {
  if (section.kind === "decisions") {
    return section.decisions.filter((d) => d.needsInput).length;
  }
  if (section.kind === "choice") {
    return section.choices.filter((c) => c.needsInput).length;
  }
  if (section.kind === "graph") {
    return section.nodeIds.filter((id) => model.nodes[id]?.needsInput).length;
  }
  if (section.kind === "fileTree") {
    const ids = new Set<string>();
    for (const group of section.tree)
      for (const e of group.entries)
        if (model.nodes[e.nodeId]?.needsInput) ids.add(e.nodeId);
    return ids.size;
  }
  if (section.kind === "beforeAfter") {
    return section.facets.filter((f) => f.needsInput).length;
  }
  return 0;
}

/** A problem's total needs-input count, summed across its sections. */
function problemFlag(problem: Problem, model: ReviewModel): number | null {
  const n = problem.sections.reduce(
    (sum, s) => sum + sectionFlag(s, model),
    0
  );
  return n > 0 ? n : null;
}

function SectionBody({
  section,
  model,
}: {
  section: Section;
  model: ReviewModel;
}) {
  switch (section.kind) {
    case "prose":
      return (
        <div
          className="prose"
          // The model's prose is trusted authored content, not user input.
          dangerouslySetInnerHTML={{ __html: section.body }}
        />
      );
    case "graph":
      return (
        <RelationshipGraph
          model={model}
          nodes={section.nodeIds
            .map((id) => model.nodes[id])
            .filter((n): n is NonNullable<typeof n> => Boolean(n))}
        />
      );
    case "fileTree":
      return <FileTree tree={section.tree} model={model} />;
    case "database":
      return <DatabasePanel tables={section.tables} model={model} />;
    case "collection":
      return <Collection collection={section.collection} />;
    case "decisions":
      return <DecisionList decisions={section.decisions} model={model} />;
    case "constraints":
      return <ConstraintList constraints={section.constraints} model={model} />;
    case "matrix":
      return <MatrixGrid matrix={section.matrix} />;
    case "choice":
      return <ChoiceList choices={section.choices} model={model} />;
    case "beforeAfter":
      return <BeforeAfter facets={section.facets} model={model} />;
  }
}

/** One section with its head — used inside an expanded problem and in validation. */
function SectionBlock({
  section,
  model,
}: {
  section: Section;
  model: ReviewModel;
}) {
  return (
    <section id={section.id} className="sec">
      <div className="sec__head">
        <h3 className="sec__title">{section.title}</h3>
        {"caption" in section && section.caption && (
          <p className="sec__caption">{section.caption}</p>
        )}
      </div>
      <SectionBody section={section} model={model} />
    </section>
  );
}

/**
 * A problem card. Collapsed, it shows only its title and tagline — the reader
 * scans the problems before opening one. Expanded, it reveals the problem's own
 * overview, then its sections in order. Progressive disclosure: nothing inside a
 * problem renders until the reader asks for it.
 */
function ProblemCard({
  problem,
  model,
}: {
  problem: Problem;
  model: ReviewModel;
}) {
  const [open, setOpen] = useState(false);
  const flag = problemFlag(problem, model);

  return (
    <article id={problem.id} className="problem" data-open={open}>
      <button
        type="button"
        className="problem__head"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="problem__disclosure" aria-hidden="true" />
        <span className="problem__heading">
          <span className="problem__title">{problem.title}</span>
          <span className="problem__tagline">{problem.tagline}</span>
        </span>
        {flag !== null && (
          <span className="problem__flag" aria-label="open decisions needing your input">
            {flag} to decide
          </span>
        )}
      </button>

      {open && (
        <div className="problem__body">
          {problem.overview && (
            <div
              className="problem__overview prose"
              dangerouslySetInnerHTML={{ __html: problem.overview }}
            />
          )}
          {problem.sections.map((s) => (
            <SectionBlock key={s.id} section={s} model={model} />
          ))}
        </div>
      )}
    </article>
  );
}

export function ReviewDocument({ model }: { model: ReviewModel }) {
  return (
    <AnnotationProvider model={model}>
    <PhaseProvider>
      <div className="doc">
        <nav className="rail">
          <div className="rail__brand">
            <span className="rail__kicker">Architecture review</span>
            <span className="rail__title">{model.title}</span>
            {model.meta && <span className="rail__meta">{model.meta}</span>}
            <PhaseToggle variant="rail" />
          </div>

          <ol className="rail__nav">
            <li>
              <a className="rail__link" href="#overview">
                <span className="rail__link-text">Overview</span>
              </a>
            </li>
            {model.problems.map((p) => {
              const flag = problemFlag(p, model);
              return (
                <li key={p.id}>
                  <a className="rail__link" href={`#${p.id}`}>
                    <span className="rail__link-text">{p.title}</span>
                    {flag !== null && (
                      <span className="rail__link-flag">{flag}</span>
                    )}
                  </a>
                </li>
              );
            })}
            {model.validation && model.validation.length > 0 && (
              <li>
                <a className="rail__link" href="#validation">
                  <span className="rail__link-text">Validation</span>
                </a>
              </li>
            )}
          </ol>

          <div className="rail__legend">
            <span className="rail__legend-title">One color, one meaning</span>
            <ul className="rail__legend-list">
              {STATE_ORDER.map((s) => (
                <li key={s} className="rail__legend-row">
                  <StateDot state={s} />
                  <span>{legendLabel(s)}</span>
                </li>
              ))}
            </ul>
          </div>
        </nav>

        <main className="main">
          <header id="overview" className="hero">
            <h1 className="hero__title">{model.title}</h1>
            {model.subtitle && <p className="hero__lede">{model.subtitle}</p>}
            {model.overview && (
              <div
                className="hero__overview prose"
                dangerouslySetInnerHTML={{ __html: model.overview }}
              />
            )}
            <div className="hero__phase">
              <span className="hero__phase-label">
                Read the whole review before this run, or after it — every
                component and layer flips in place:
              </span>
              <PhaseToggle variant="hero" />
            </div>
          </header>

          <div className="problems">
            {model.problems.map((p) => (
              <ProblemCard key={p.id} problem={p} model={model} />
            ))}
          </div>

          {model.validation && model.validation.length > 0 && (
            <section id="validation" className="validation">
              <div className="validation__head">
                <span className="validation__kicker">Validation</span>
                <h2 className="validation__title">How the plan proves itself</h2>
                <p className="validation__lede">
                  The checks that close the document — not folded into any one
                  problem, run last, against every problem above.
                </p>
              </div>
              <div className="validation__body">
                {model.validation.map((s) => (
                  <SectionBlock key={s.id} section={s} model={model} />
                ))}
              </div>
            </section>
          )}
        </main>

        <AnnotationLayer />
      </div>
    </PhaseProvider>
    </AnnotationProvider>
  );
}

function legendLabel(s: (typeof STATE_ORDER)[number]): string {
  const map: Record<string, string> = {
    added: "added — net-new",
    changed: "changed — shape moves",
    removed: "removed — deleted",
    kept: "kept — unchanged by design",
    separate: "separate — out of scope",
  };
  return map[s] ?? CHANGE_STATES[s].label;
}
