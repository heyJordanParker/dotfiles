/**
 * The review model — the contract an agent fills with content.
 *
 * One principle governs the whole shape: context is defined once, on the node,
 * and reused everywhere. A file, a table, a module, a decision — each is a node
 * with a stable id, a one-line relationship summary, a one-line why, and a few
 * usage bullets. Views reference nodes by id and render a lean version inline;
 * the full context surfaces on demand (a graph node's popover, a tree row's
 * detail). The agent writes the context in one place and never repeats it.
 */

/**
 * The five change states. One color, one meaning, document-wide. This is the
 * single most load-bearing decision in the kit: every badge, every diff rail,
 * every column dot, every graph edge draws its color from this enum and nothing
 * else. A state never changes meaning between sections or between before/after.
 */
export type ChangeState =
  | "added" // net-new — did not exist before
  | "changed" // existed, its shape or behavior moves
  | "removed" // existed, deleted by this change
  | "kept" // existed, deliberately unchanged
  | "separate"; // deliberately left out of scope, no dependency drawn

export interface ChangeStateMeta {
  readonly state: ChangeState;
  readonly label: string;
  /** The CSS custom-property name carrying this state's hue, e.g. "--state-added". */
  readonly token: string;
}

/* ─────────────────────────── Nodes — context defined once ─────────────────── */

/** What kind of thing a node is. Drives the glyph and the graph shape. */
export type NodeKind = "file" | "directory" | "table" | "module" | "service" | "concept";

export interface ContextNode {
  readonly id: string;
  readonly name: string;
  readonly kind: NodeKind;
  readonly state: ChangeState;
  /** The real path or namespace, verbatim. Shown in mono. */
  readonly path?: string;
  /** One line: how this node relates to its neighbours. The graph node's subtitle. */
  readonly summary: string;
  /** One line: why this node exists / why it changes. */
  readonly why: string;
  /**
   * The node's full method / API signature surface — the real public lines, one
   * per entry (e.g. "can(string $action, Model $target): bool"). When a graph
   * draws only one to three nodes, each node renders its signature in place, so a
   * sparse diagram becomes a rich signature card rather than an empty box. Absent
   * → the node stays a name + subtitle. Dense diagrams suppress it for room.
   */
  readonly signature?: readonly string[];
  /**
   * The second organizing axis. A node may belong to a group — a tier, a wave, a
   * domain — and a graph that carries groups renders its nodes under group
   * captions instead of one flat grid. The primary axis stays change-state
   * (color); the group is the orthogonal dimension a roadmap or a domain-sliced
   * schema would otherwise lose to prose.
   */
  readonly group?: string;
  /** A few bullets: how it is used. Lean inline, full in the popover. */
  readonly usage?: readonly string[];
  /** Ids of nodes this one depends on (import direction: this → target). */
  readonly dependsOn?: readonly NodeDependency[];
  /**
   * Set when the proposal names this node as a concept, not a concrete value —
   * the `path` is a description ("PHP attributes", "one templated policy per
   * table"), not a real file path. The flag names the specific the orchestrator
   * must supply. The detail surfaces it honestly instead of inventing a path.
   */
  readonly needsInput?: NeedsInput;
}

export interface NodeDependency {
  readonly target: string;
  /** How the edge reads. "imports", "reads", "calls", "kept separate". */
  readonly label?: string;
  /** A separate edge is drawn dashed and carries no real dependency. */
  readonly state?: ChangeState;
}

/* ─────────────────────────── Code — block and diff ─────────────────────────── */

/**
 * A single block of real code rendered through the Pierre file viewer — the
 * public surface IS the class, a table IS its DDL. The why/notes live inside
 * the code as inline `//` comments, authored here. Never a boxed badge-row.
 */
export interface CodeBlockSpec {
  /** Shiki language id. "php", "ts", "sql", "json". */
  readonly language: string;
  /** The verbatim code — class with signatures, or DDL — with `//` comments. */
  readonly code: string;
  /** Optional file/label shown above the block. */
  readonly label?: string;
}

/** A before/after change rendered through the Pierre diff viewer. */
export interface CodeChange {
  /** Shiki language id. "php", "ts", "sql", "json". */
  readonly language: string;
  readonly before?: string;
  readonly after?: string;
  /** "split" shows before and after side by side; "unified" interleaves. */
  readonly layout?: "split" | "unified";
  readonly beforeLabel?: string;
  readonly afterLabel?: string;
}

/** Either a static code block or a before/after diff. The renderer picks. */
export type CodeView =
  | { readonly mode: "block"; readonly block: CodeBlockSpec }
  | { readonly mode: "diff"; readonly diff: CodeChange };

/* ─────────────────────────── Needs-input gap ───────────────────────────────── */

/**
 * An honest gap flag. A generating agent needs a specific — a real file path, a
 * column name, a chosen value — but the source proposal only names a concept.
 * Rather than fabricate the value, the agent flags exactly what is missing for
 * the orchestrator to fill. This is the kit's one mechanism for "I don't know
 * this yet, and I will not invent it." Rendered as an amber chip that names the
 * missing specific; never a placeholder dressed as a fact.
 */
export interface NeedsInput {
  /** The specific that is missing, named exactly. e.g. "the real file path". */
  readonly missing: string;
  /** What the proposal does pin down, so the gap reads in context. Optional. */
  readonly knownInstead?: string;
}

/* ─────────────────────────── File detail ───────────────────────────────────── */

/**
 * What opening a file reveals — identical whether opened from the tree row or a
 * graph node. It carries the node's context (path, why, how) plus, AS CODE
 * through Pierre: the public surface (the real class with its signatures and
 * inline `//` comments) and a full proposed diff that is folded by default and
 * expands on demand. The file's own relationship diagram is drawn from the
 * node's dependsOn, so it is defined once and never repeated here.
 */
export interface FileDetail {
  /** The node id whose context (summary, why, deps) this file carries. */
  readonly nodeId: string;
  /** The public surface as code — rendered open. */
  readonly surface?: CodeBlockSpec;
  /** A full proposed change — rendered folded, expands on click. */
  readonly code?: CodeChange;
}

/* ─────────────────────────── File tree ─────────────────────────────────────── */

export interface TreeEntry {
  readonly nodeId: string;
  /** Nesting depth for the tree glyphs. 0 = group root. */
  readonly depth: number;
  /** A directory/group header is a label only, not openable. */
  readonly isGroup?: boolean;
  /** The detail panel shown when this entry is opened. Absent for groups. */
  readonly detail?: FileDetail;
}

export interface FileTreeSection {
  readonly caption?: string;
  readonly entries: readonly TreeEntry[];
}

/* ─────────────────────────── Database ──────────────────────────────────────── */

/**
 * A table rendered as real migration DDL through Pierre — the Schema::create /
 * Blueprint (or raw SQL), with each column's note as an inline `//` comment. A
 * changed table is a before/after DDL diff. Never a list of boxed column rows.
 */
export interface DbTable {
  readonly nodeId: string;
  /** The whole table's change state — the tab badge. */
  readonly state: ChangeState;
  /**
   * The second axis — the domain a table belongs to (Billing, Identity, Content).
   * A schema whose primary organizing dimension is domain renders its tabs under
   * domain captions instead of one flat strip, so the domain stays a visible axis
   * rather than guesswork from table names.
   */
  readonly group?: string;
  /** The DDL as code. For an added/kept table, the whole definition. */
  readonly ddl: string;
  /** When present, render a before/after DDL diff instead of a block. */
  readonly ddlBefore?: string;
  /** DDL language — "php" (Blueprint) or "sql". Defaults to "php". */
  readonly language?: string;
}

/* ─────────────────────────── Decisions ─────────────────────────────────────── */

export interface Decision {
  readonly id: string;
  readonly title: string;
  /** Where the prior ruling came from, named — kept terse, one line. */
  readonly priorRuling: string;
  /** The impact of overruling it — one line. */
  readonly impactIfOverruled: string;
  /**
   * The agent's conviction in this settled decision, as a percentage (0–100) —
   * the same number a Choice option carries. A decision is settled, not open, but
   * it can still show how sure the run is in the call. Absent → no chip.
   */
  readonly confidence?: number;
  /**
   * How the decision was proven — the validation that settled it, one line.
   * e.g. "all 1,204 existing tests pass against the flattened policy". The
   * counterpart to a Choice's recommendation: not which to pick, but why this one
   * is now trusted. Absent → no validation line.
   */
  readonly validation?: string;
  /**
   * The second axis, as on a node — a wave, a tier, a slice. A decision-heavy log
   * organized by wave keeps the wave visible as a caption instead of folding it
   * into the title.
   */
  readonly group?: string;
  /** The tangible change, AS CODE, through Pierre — the meaning, not prose. */
  readonly change?: CodeView;
  /** Optional small relationship diagram — node ids to draw for this decision. */
  readonly diagramNodeIds?: readonly string[];
  /**
   * Set when this decision visibly needs the architect's input. `true` is the
   * bare flag; a NeedsInput names exactly what specific is missing for the
   * orchestrator to fill (preferred — it tells the architect what to decide).
   */
  readonly needsInput?: boolean | NeedsInput;
}

/* ─────────────────────────── Standing constraint ───────────────────────────── */

/**
 * A boundary held on purpose — "what stays, and why crossing it would hurt". The
 * exact inverse of a Decision: a decision overturns a prior ruling; a constraint
 * is a ruling deliberately NOT overturned. It carries what it protects and the
 * impact if it were crossed, so a slicing plan can state its non-negotiable edges
 * as first-class content instead of burying them in prose. Read through the
 * `kept` neutral hue — these are the unchanged-by-design boundaries.
 */
export interface Constraint {
  readonly id: string;
  readonly title: string;
  /** What this boundary protects — one line. */
  readonly protects: string;
  /** What breaks if it were crossed — one line. */
  readonly impactIfCrossed: string;
  /** The second axis, optional — the slice or domain this boundary guards. */
  readonly group?: string;
  /** Optional small relationship diagram — node ids the boundary spans. */
  readonly diagramNodeIds?: readonly string[];
}

/* ─────────────────────────── Two-axis matrix ───────────────────────────────── */

/**
 * A rows × columns grid for content the one-axis collection cannot express: a
 * failure-triage grid (failure × root-cause × fix-owner), a rule doctrine
 * (rule × where-it-applies). Each cell sits at one (row, column) and carries a
 * terse value plus an optional change state, so the grid reads through the one
 * semantic color system. The collection stays the right tool for a single
 * mapping; the matrix is the right tool when two axes cross.
 */
export interface MatrixAxis {
  readonly id: string;
  readonly label: string;
  /** Optional one-line gloss shown under the label. */
  readonly note?: string;
  /**
   * A column's subtle tint, naming the column's MEANING — not a change state.
   * "caution" tints the column faintly red (the thing that broke), "ok" faintly
   * green (the thing that closes it), "watch" faintly amber. Rows ignore it. The
   * grid reads through quiet tinted columns and alternating rows instead of
   * red/green text in every cell. Absent → no column tint.
   */
  readonly tint?: MatrixTint;
  /**
   * A row's plain-language detail, revealed when the reader clicks the row. The
   * grid stays scannable; the full story for one row surfaces on demand, in
   * sentences, with no codes. Rows only. Absent → the row is not expandable.
   */
  readonly detail?: string;
}

/** A column's quiet tint, by meaning. Never the change-state vocabulary. */
export type MatrixTint = "caution" | "watch" | "ok" | "neutral";

export interface MatrixCell {
  readonly rowId: string;
  readonly columnId: string;
  /** The cell's value, terse. Empty/absent → a blank cell. */
  readonly value?: string;
  /** Reads through the one color system when the cell carries a change state. */
  readonly state?: ChangeState;
}

export interface Matrix {
  readonly rows: readonly MatrixAxis[];
  readonly columns: readonly MatrixAxis[];
  /** The cells, keyed by (rowId, columnId). A missing pair renders blank. */
  readonly cells: readonly MatrixCell[];
  /** Optional label for the top-left corner — names the row axis. */
  readonly corner?: string;
}

/* ─────────────────────────── Open choice ───────────────────────────────────── */

/**
 * An open choice in a proposal — the most load-bearing thing a review carries.
 * It is a question with two or three traced options, each with pros, cons, and a
 * confidence, plus a recommendation. The generating agent did the tracing; the
 * architect picks. Confidence is the agent's own conviction in the option, NOT
 * a vote — the recommendation names which option to take.
 */
export interface ChoiceOption {
  readonly id: string;
  /** The option's name, terse. e.g. "Specificity-resolved". */
  readonly label: string;
  /** One line: what this option is. */
  readonly summary: string;
  readonly pros: readonly string[];
  readonly cons: readonly string[];
  /**
   * The agent's conviction in this option as the answer, as a percentage
   * (0–100) — the same number the architect reads in /pcc. Grounded in the
   * option's own pros/cons; the recommended option reads higher, and options
   * differ by a meaningful margin.
   */
  readonly confidence: number;
  /** The option's shape AS CODE through Pierre — block or before/after diff. */
  readonly change?: CodeView;
  /** Marks the option the agent recommends. Exactly one across the set. */
  readonly recommended?: boolean;
}

export interface Choice {
  readonly id: string;
  /** The question, as a question. e.g. "How does a ban override a grant?" */
  readonly question: string;
  /** One line of stakes: why this is open, what hangs on it. */
  readonly stakes: string;
  readonly options: readonly ChoiceOption[];
  /** The recommendation in one line — which option, and the deciding reason. */
  readonly recommendation: string;
  /** Set when even the recommendation needs the architect to confirm a value. */
  readonly needsInput?: NeedsInput;
}

/* ─────────────────────────── Expandable collection ─────────────────────────── */

/** A vague count that expands on click to the actual list. */
export interface Collection {
  readonly id: string;
  /** The vague summary line, e.g. "~40 bespoke verb routes". */
  readonly summary: string;
  /** The real total, so the header count matches the list. */
  readonly total?: number;
  readonly items: readonly CollectionItem[];
}

export interface CollectionItem {
  readonly label: string;
  /** Where this item maps to — the action / handler / capability. */
  readonly destination?: string;
  readonly note?: string;
  readonly state?: ChangeState;
  /**
   * The second axis — the slice, wave, or bucket this item falls under. A log
   * whose primary dimension is slice renders its rows under slice captions
   * instead of one flat list, keeping the slice a visible axis.
   */
  readonly group?: string;
  /**
   * How this item changed, AS CODE through Pierre — a before/after diff (the old
   * route beside the new) or the action code it becomes. When present, the row
   * is openable and expands to this on click. The collection stops being a dead
   * list and becomes reviewable per item.
   */
  readonly change?: CodeView;
}

/* ─────────────────────────── Before / after facet ──────────────────────────
 *
 * The one primitive that maps the OLD architecture onto the NEW one in place.
 * A facet is one component or one layer of the system — the store, the check,
 * the audiences, a gate, the field read, a link tier. It carries a `before`
 * variant and an `after` variant of the SAME thing, and an in-place toggle flips
 * between them on the same spot: the same heading, the same explanation slot, the
 * same code/DDL/diagram slot. This is what replaces a linear before-section then
 * after-section — the reviewer sits on a component and flips its two states.
 *
 * Each variant carries its own one-line explanation (plain sentences) and one
 * body: code AS CODE through the viewer (a block), a DDL block, or a small
 * relationship diagram drawn from node ids. The two variants of a facet share a
 * body kind so the toggle swaps like for like — before-DDL flips to after-DDL,
 * before-graph to after-graph. A facet whose after-state is a change from its
 * before-state reads through the one color system via its `state`.
 */
export interface PhaseVariant {
  /** One line, plain sentences: what this component IS in this phase. */
  readonly explain: string;
  /** The body AS CODE through the viewer — the class, method, or shape. */
  readonly code?: CodeBlockSpec;
  /** The body as migration DDL / SQL through the viewer. */
  readonly ddl?: CodeBlockSpec;
  /** The body as a small relationship diagram — node ids to draw. */
  readonly diagramNodeIds?: readonly string[];
  /** Extra one-line notes for this phase, shown under the explanation. */
  readonly notes?: readonly string[];
}

export interface BeforeAfterFacet {
  readonly id: string;
  /** The component / layer, named in plain language — no codes. */
  readonly title: string;
  /** One line shown always (phase-independent): what this facet covers. */
  readonly summary: string;
  /**
   * The facet's overall change state — the badge and accent. `changed` for a
   * component that existed and moved, `added` for one net-new after the run,
   * `removed` for one that only existed before, `separate` for a deliberate
   * sibling. Drives the one color system.
   */
  readonly state: ChangeState;
  /** The old state of this component/layer — what the toggle shows on "before". */
  readonly before: PhaseVariant;
  /** The new state of this component/layer — what the toggle shows on "after". */
  readonly after: PhaseVariant;
  /** Set when even the after-state still needs the architect's input. */
  readonly needsInput?: NeedsInput;
  /** The second axis — the layer band this facet sits in (optional). */
  readonly group?: string;
}

/* ─────────────────────────── Sections ──────────────────────────────────────── */

export type Section =
  | { readonly kind: "prose"; readonly id: string; readonly title: string; readonly body: string }
  | {
      readonly kind: "graph";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly nodeIds: readonly string[];
    }
  | {
      readonly kind: "fileTree";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly tree: readonly FileTreeSection[];
    }
  | {
      readonly kind: "database";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly tables: readonly DbTable[];
    }
  | {
      readonly kind: "collection";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly collection: Collection;
    }
  | {
      readonly kind: "decisions";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly decisions: readonly Decision[];
    }
  | {
      readonly kind: "constraints";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly constraints: readonly Constraint[];
    }
  | {
      readonly kind: "matrix";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly matrix: Matrix;
    }
  | {
      readonly kind: "choice";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      readonly choices: readonly Choice[];
    }
  | {
      readonly kind: "beforeAfter";
      readonly id: string;
      readonly title: string;
      readonly caption?: string;
      /**
       * The components / layers this section covers, each with its own in-place
       * before/after toggle. Reads the document-wide phase as its default, and
       * each facet can flip locally. This is the section that maps the old
       * architecture onto the new one component by component.
       */
      readonly facets: readonly BeforeAfterFacet[];
    };

/* ─────────────────────────── Problems — the organizing layer ────────────────
 *
 * A plan is read by the PROBLEMS it solves, problem by problem — never as a flat
 * dump of every section at once. The document opens with one overview, then each
 * problem is its own collapsed card the reader expands to dig in. A problem
 * carries its own one-line tagline (shown collapsed), its own short overview
 * (shown first when expanded), and its own ordered sections — the same section
 * vocabulary, scoped to this one problem. The reader understands the document
 * level by level: overview → which problem → that problem's own breakdown.
 *
 * Validation is never folded inside a problem and never sits mid-document. It is
 * the document's closing region, authored once at the model root.
 */
export interface Problem {
  readonly id: string;
  /** The problem, named in plain language — no codes. */
  readonly title: string;
  /** One line shown on the collapsed card: what this problem is. */
  readonly tagline: string;
  /**
   * The problem's own short overview, shown first when the card expands — the
   * architectural issue stated plainly, before any section. Trusted authored
   * HTML, same contract as a prose section's body.
   */
  readonly overview?: string;
  /** This problem's own ordered sections — the same section vocabulary, scoped. */
  readonly sections: readonly Section[];
}

/* ─────────────────────────── The document ──────────────────────────────────── */

export interface ReviewModel {
  readonly title: string;
  readonly subtitle?: string;
  readonly meta?: string;
  /** Every node, keyed by id. Context defined once, here. */
  readonly nodes: Readonly<Record<string, ContextNode>>;
  /**
   * The document's opening overview — the whole plan in one read, before any
   * problem expands. Trusted authored HTML (same contract as a prose section).
   */
  readonly overview?: string;
  /** The plan, sliced by the problems it solves. Read problem by problem. */
  readonly problems: readonly Problem[];
  /**
   * Validation — the closing region, at the document's end. How the plan proves
   * itself: the gates, the checks, the acceptance rules. Authored once here, so
   * it can never drift into the middle of a problem. Same section vocabulary.
   */
  readonly validation?: readonly Section[];
}

/* ─────────────────────────── State metadata ────────────────────────────────── */

export const CHANGE_STATES: Readonly<Record<ChangeState, ChangeStateMeta>> = {
  added: { state: "added", label: "added", token: "--state-added" },
  changed: { state: "changed", label: "changed", token: "--state-changed" },
  removed: { state: "removed", label: "removed", token: "--state-removed" },
  kept: { state: "kept", label: "kept", token: "--state-kept" },
  separate: { state: "separate", label: "separate", token: "--state-separate" },
};

export const STATE_ORDER: readonly ChangeState[] = [
  "added",
  "changed",
  "removed",
  "kept",
  "separate",
];

/* ─────────────────────────── Detail lookup ─────────────────────────────────── */

/**
 * The FileDetail for every node that has one, keyed by node id, gathered from
 * every fileTree section. This is what lets a graph node open the SAME rich
 * detail a tree row opens — the detail is authored once in the tree, and the
 * graph reads it by id. A node without a detail (no file surface) is absent;
 * the graph falls back to its terse context view for those.
 */
export function fileDetailsByNode(
  model: ReviewModel
): Readonly<Record<string, FileDetail>> {
  const out: Record<string, FileDetail> = {};
  for (const section of allSections(model)) {
    if (section.kind !== "fileTree") continue;
    for (const group of section.tree) {
      for (const entry of group.entries) {
        if (entry.detail) out[entry.detail.nodeId] = entry.detail;
      }
    }
  }
  return out;
}

/**
 * Every section in the document, in reading order: each problem's sections,
 * then the closing validation region. The single place that flattens the
 * problem hierarchy back to a section stream, so node-gathering and any other
 * whole-document pass reads one source.
 */
export function allSections(model: ReviewModel): readonly Section[] {
  const out: Section[] = [];
  for (const problem of model.problems) out.push(...problem.sections);
  if (model.validation) out.push(...model.validation);
  return out;
}
