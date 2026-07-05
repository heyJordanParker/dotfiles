/**
 * arch-review-kit — a component library for assembling architecture-review
 * documents from data. The agent supplies a ReviewModel; the library renders it
 * to a high design bar, with a built-in annotate-then-copy flow for handing
 * comments to another agent.
 *
 * Import the styles once (lib/styles/index.css), then render <ReviewDocument>.
 */
export { ReviewDocument } from "./components/ReviewDocument";
export { RelationshipGraph } from "./components/RelationshipGraph";
export { FileTree } from "./components/FileTree";
export { CodeBlock } from "./components/CodeBlock";
export { CodeDiff } from "./components/CodeDiff";
export { DatabasePanel } from "./components/DatabasePanel";
export { Collection } from "./components/Collection";
export { DecisionList } from "./components/DecisionList";
export { ConstraintList } from "./components/ConstraintList";
export { MatrixGrid } from "./components/MatrixGrid";
export { ChoiceList } from "./components/ChoiceList";
export { BeforeAfter } from "./components/BeforeAfter";
export {
  PhaseProvider,
  PhaseToggle,
  usePhase,
  PHASE_LABEL,
  type Phase,
} from "./phase";
export { FileDetail } from "./components/FileDetail";
export { NeedsInputFlag } from "./components/NeedsInputFlag";
export { AnnotationLayer } from "./components/AnnotationLayer";
export {
  AnnotationProvider,
  useAnnotations,
  type Annotation,
  type AnnotationTarget,
} from "./annotations";
export {
  StateBadge,
  StateDot,
  Annotatable,
} from "./components/primitives";

export * from "./model";
