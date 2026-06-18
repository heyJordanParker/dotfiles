# plan-visualizer tooling

The rendering kit the `plan-visualizer` agent runs. To *use* it (turn a proposal
into a review), follow the agent's instructions — author a `ReviewModel`, run the
build. This README is for working on the kit itself.

## What it is

A data-driven React component library that turns one typed contract — `ReviewModel`
(`lib/model.ts`) — into a self-contained architecture-review HTML file with no
server. An agent authors a `ReviewModel`; a one-way render tree consumes it.

Every per-run artifact — the authored model, the build scratch, the output HTML —
lives under `/tmp`, never in this directory. This directory ships only managed
code (the library, the builder) plus the installed `node_modules`; nothing a run
produces is written here. That also keeps the build runnable while the calling
session is mid-proposal, where writes under `/tmp` are permitted but writes inside
the working directory are not.

- **`lib/`** — the library.
  - `model.ts` — the contract an agent fills. Context for a node (summary, why,
    usage) is defined once on the node, keyed by id; views reference it by id. The
    five `ChangeState` values are the document's entire color vocabulary — one
    color, one meaning, everywhere: `added` green, `changed` amber, `removed` red,
    `kept` neutral, `separate` blue.
  - `components/` — `ReviewDocument` (the shell, dispatches by `Section.kind`),
    `RelationshipGraph`, `FileTree`/`FileDetail`, `CodeBlock`/`CodeDiff` (the two
    ways code appears, both through Pierre), `DatabasePanel`, `Collection`,
    `DecisionList`, `ConstraintList`, `MatrixGrid`, `ChoiceList`, `NeedsInputFlag`,
    and the annotation layer.
  - `styles/` — OKLCH tokens (`tokens.css` owns the five state hues) and one
    single-purpose BEM stylesheet per component area, wired through `index.css`.
- **`example/mount.tsx`** — the single renderer: `mount(model)` → `<ReviewDocument>`.
- **`build.mjs`** — the one builder (see below).

The agent-authored `ReviewModel` data file is a per-run artifact, written under
`/tmp` (e.g. `/tmp/plan-visualizer/<name>/<name>.ts`) — never in this directory.

## The build

`node build.mjs <model> [outDir]` runs one single-input Vite build with
`vite-plugin-singlefile`, inlining all JS and CSS into one `.html`. `<model>` is a
path to a `.ts` file (authored under `/tmp`) that exports a `ReviewModel`. The
entry and HTML shell are generated per-run into a `/tmp` scratch dir
(`/tmp/plan-visualizer-build/<name>/`) and torn down after — the model is the only
thing an author writes. `outDir` defaults to the model's own directory, so the
output `.html` lands beside the model under `/tmp`.

The single-file plugin needs `inlineDynamicImports`, which rollup forbids with
multiple inputs, so each model is its own build. `root` is set to the `/tmp`
scratch dir so the input HTML sits at the build root and the output emits flat;
the entry imports the library and the model by absolute path, and bare
dependencies resolve from the library files' own location in this directory.

## Pierre offline — @pierre/diffs

Both code renderers use `@pierre/diffs` (Shiki-based, so code tokenizes by real
grammar). Two settings make it render under `file://`:

1. **`disableWorkerPool`** on each diff — the worker pool is the only part that
   breaks offline; with it off, Pierre highlights on the main thread.
2. **Preload the shared highlighter first** (`lib/highlight.ts`,
   `preferredHighlighter: "shiki-js"` to avoid the wasm fetch). Without it the diff
   mounts a zero-height empty shell; with it, the diff mounts populated.

Pierre renders into a Shadow DOM; its rails are themed to the document palette via
the `--diffs-*` custom properties set in `document.css`.

## Adding a renderer or model field

Extend `model.ts` (the contract), add the section variant to the `Section` union,
render it in `ReviewDocument`'s `Section.kind` switch, and add a BEM stylesheet
wired through `index.css`. Code always routes through `CodeBlock`/`CodeDiff` — never
a boxed badge row. State is always a `data-state` attribute reading from
`tokens.css` — never a literal color in a component.
