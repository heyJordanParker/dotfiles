/**
 * FileDetail — the one rich detail for a file, rendered identically whether it
 * is reached from a tree row or a graph node. It carries, as distinct fields:
 *
 *   · the file path (mono, verbatim) — or a needs-input flag when the proposal
 *     names the file as a concept and no real path exists to show
 *   · WHY it exists — one line, from the node
 *   · HOW — a few bullets of the major points, from the node's usage
 *   · the file's own relationship diagram — a compact graph of this node plus
 *     its immediate dependsOn neighbours, drawn from the registry
 *   · its public API AS CODE through Pierre — the real class, rendered open
 *   · a full proposed diff — folded by default, expands on click
 *
 * Context is defined once on the node; this view reads it. The same component
 * mounts in the tree detail and the graph popover, so the two can never drift.
 */
import { useState } from "react";
import type { ContextNode, FileDetail as FileDetailModel, ReviewModel } from "../model";
import { StateBadge } from "./primitives";
import { CodeBlock } from "./CodeBlock";
import { CodeDiff } from "./CodeDiff";
import { RelationshipGraph } from "./RelationshipGraph";
import { NeedsInputFlag } from "./NeedsInputFlag";

/**
 * The file's full neighbourhood, BOTH directions: this node, the nodes it
 * depends on (outgoing), and the nodes that depend on it (incoming). Including
 * the dependents means their own `dependsOn` edge back to this node renders as
 * an explicit incoming arrow in the diagram — so the file's complete
 * relationship surface is visible, not only what it reaches.
 */
function neighbourhood(node: ContextNode, model: ReviewModel): ContextNode[] {
  const outgoing = (node.dependsOn ?? []).map((d) => d.target);
  const incoming = Object.values(model.nodes)
    .filter((n) => (n.dependsOn ?? []).some((d) => d.target === node.id))
    .map((n) => n.id);
  const ids = [node.id, ...outgoing, ...incoming];
  const seen = new Set<string>();
  const nodes: ContextNode[] = [];
  for (const id of ids) {
    if (seen.has(id)) continue;
    seen.add(id);
    const n = model.nodes[id];
    if (n) nodes.push(n);
  }
  return nodes;
}

export function FileDetail({
  node,
  detail,
  model,
}: {
  node: ContextNode;
  detail: FileDetailModel;
  model: ReviewModel;
}) {
  const [diffOpen, setDiffOpen] = useState(false);
  const diagram = neighbourhood(node, model);
  // A lone node with a signature still draws — as a rich signature card, not an
  // empty box (the adaptive-signature rule replaces suppressing single nodes).
  const hasDiagram =
    diagram.length > 1 || (node.signature?.length ?? 0) > 0;

  return (
    <div className="file-detail">
      <header className="file-detail__head">
        <span className="file-detail__name" data-state={node.state}>
          {node.name}
        </span>
        <StateBadge state={node.state} />
        {node.path && !node.needsInput && (
          <code className="file-detail__path">{node.path}</code>
        )}
      </header>

      {node.needsInput && <NeedsInputFlag gap={node.needsInput} />}

      <dl className="file-detail__facts">
        <div className="file-detail__fact">
          <dt className="file-detail__term">why</dt>
          <dd className="file-detail__def">{node.why}</dd>
        </div>
        {node.usage && node.usage.length > 0 && (
          <div className="file-detail__fact">
            <dt className="file-detail__term">how</dt>
            <dd className="file-detail__def">
              <ul className="file-detail__how">
                {node.usage.map((u, i) => (
                  <li key={i}>{u}</li>
                ))}
              </ul>
            </dd>
          </div>
        )}
      </dl>

      {hasDiagram && (
        <div className="file-detail__diagram">
          <span className="file-detail__label">how it sits</span>
          <RelationshipGraph nodes={diagram} compact />
        </div>
      )}

      {detail.surface && (
        <div className="file-detail__surface">
          <span className="file-detail__label">public surface</span>
          <CodeBlock
            code={detail.surface.code}
            language={detail.surface.language}
            label={detail.surface.label}
          />
        </div>
      )}

      {detail.code && (
        <div className="file-detail__diff" data-open={diffOpen}>
          <button
            type="button"
            className="file-detail__diff-toggle"
            aria-expanded={diffOpen}
            onClick={() => setDiffOpen((v) => !v)}
          >
            <span className="file-detail__disclosure" aria-hidden="true" />
            <span className="file-detail__diff-label">proposed diff</span>
            <span className="file-detail__diff-hint">
              {diffOpen ? "hide" : "show"}
            </span>
          </button>
          {diffOpen && (
            <div className="file-detail__diff-body">
              <CodeDiff change={detail.code} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
