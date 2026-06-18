/**
 * Shared primitives: the state badge (the document's one-color-one-meaning chip)
 * and the Annotatable wrapper (turns any surface into an annotation target).
 */
import type { ReactNode } from "react";
import { CHANGE_STATES, type ChangeState } from "../model";
import { useAnnotations, type AnnotationTarget } from "../annotations";

/** The lifecycle chip. Reads its color from the state token, nothing else. */
export function StateBadge({
  state,
  children,
}: {
  state: ChangeState;
  children?: ReactNode;
}) {
  return (
    <span className="state-badge" data-state={state}>
      {children ?? CHANGE_STATES[state].label}
    </span>
  );
}

/** A small state dot, for legends and tree rows. */
export function StateDot({ state }: { state: ChangeState }) {
  return <span className="state-dot" data-state={state} aria-hidden="true" />;
}

/**
 * Annotatable — wraps a surface so clicking its margin pin opens the annotation
 * composer for that target. An existing note shows a filled pin. The wrapped
 * content is untouched; the pin lives in a gutter so it never disturbs layout.
 */
export function Annotatable({
  target,
  children,
  className,
}: {
  target: AnnotationTarget;
  children: ReactNode;
  className?: string;
}) {
  const { annotations, open } = useAnnotations();
  const annotated = Boolean(annotations[target.surfaceId]);
  return (
    <div
      className={`annotatable${className ? ` ${className}` : ""}`}
      data-annotated={annotated}
    >
      <button
        type="button"
        className="annotatable__pin"
        aria-label={
          annotated
            ? `Edit comment on ${target.label}`
            : `Add comment on ${target.label}`
        }
        title={annotated ? "Edit comment" : "Comment on this"}
        onClick={() => open(target)}
      >
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path
            d="M3 3h10v7H7l-3 3v-3H3z"
            fill={annotated ? "currentColor" : "none"}
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinejoin="round"
          />
        </svg>
      </button>
      <div className="annotatable__body">{children}</div>
    </div>
  );
}
