/**
 * ChoiceList — how the kit presents an open CHOICE, the most load-bearing thing
 * a proposal carries. A choice is a question with two or three traced options,
 * each with pros, cons, and a confidence, plus a recommendation. The generating
 * agent did the tracing; the architect picks.
 *
 * The recommended option reads through the `added` green (the path forward); its
 * confidence chip and the option's shape (as Pierre code) make the case. The
 * others stay neutral. No per-option rainbow — the one semantic color system
 * carries recommended-vs-not, and confidence is a labelled chip, not a hue.
 */
import type { Choice, ChoiceOption, ReviewModel } from "../model";
import { Annotatable } from "./primitives";
import { CodeBlock } from "./CodeBlock";
import { CodeDiff } from "./CodeDiff";
import { NeedsInputFlag } from "./NeedsInputFlag";

function OptionChange({ option }: { option: ChoiceOption }) {
  if (!option.change) return null;
  if (option.change.mode === "diff") {
    return <CodeDiff change={option.change.diff} />;
  }
  return (
    <CodeBlock
      code={option.change.block.code}
      language={option.change.block.language}
      label={option.change.block.label}
    />
  );
}

function OptionCard({ option }: { option: ChoiceOption }) {
  return (
    <article
      className="option"
      data-recommended={Boolean(option.recommended)}
    >
      <header className="option__head">
        <span className="option__label">{option.label}</span>
        {option.recommended && (
          <span className="option__pick">recommended</span>
        )}
        <span
          className="option__confidence"
          data-recommended={Boolean(option.recommended)}
        >
          {option.confidence}%
        </span>
      </header>

      <p className="option__summary">{option.summary}</p>

      <div className="option__weigh">
        <ul className="option__pros">
          {option.pros.map((p, i) => (
            <li key={i}>{p}</li>
          ))}
        </ul>
        <ul className="option__cons">
          {option.cons.map((c, i) => (
            <li key={i}>{c}</li>
          ))}
        </ul>
      </div>

      <OptionChange option={option} />
    </article>
  );
}

function ChoiceCard({ choice }: { choice: Choice }) {
  return (
    <Annotatable
      target={{
        surfaceId: `choice:${choice.id}`,
        label: choice.question,
        kind: "choice",
        facts: [
          `Stakes: ${choice.stakes}`,
          ...choice.options.map(
            (o) =>
              `Option ${o.label} [${o.confidence}% confidence${
                o.recommended ? ", recommended" : ""
              }]: ${o.summary}`
          ),
          `Recommendation: ${choice.recommendation}`,
        ],
      }}
    >
      <section className="choice" data-needs-input={Boolean(choice.needsInput)}>
        <header className="choice__head">
          <h3 className="choice__question">{choice.question}</h3>
          <p className="choice__stakes">{choice.stakes}</p>
        </header>

        <div className="choice__options">
          {choice.options.map((o) => (
            <OptionCard key={o.id} option={o} />
          ))}
        </div>

        <footer className="choice__rec">
          <span className="choice__rec-lead">recommendation</span>
          <p className="choice__rec-body">{choice.recommendation}</p>
        </footer>

        {choice.needsInput && <NeedsInputFlag gap={choice.needsInput} />}
      </section>
    </Annotatable>
  );
}

export function ChoiceList({
  choices,
}: {
  choices: readonly Choice[];
  model: ReviewModel;
}) {
  return (
    <div className="choices">
      {choices.map((c) => (
        <ChoiceCard key={c.id} choice={c} />
      ))}
    </div>
  );
}
