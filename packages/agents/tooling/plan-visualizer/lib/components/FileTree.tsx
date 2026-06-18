/**
 * FileTree — a clean nested tree. Each row is a file; opening one reveals its
 * public surface AS CODE through Pierre (the real class with its signatures and
 * inline `//` comments), or a before/after Pierre diff for a changed surface.
 * No stray dots, no chevron-character bullets, no duplicated all-caps path
 * headers — indentation and a disclosure triangle carry the structure, and the
 * code carries the meaning.
 */
import { useState } from "react";
import type {
  FileTreeSection,
  TreeEntry,
  ReviewModel,
  ContextNode,
} from "../model";
import { StateBadge, Annotatable } from "./primitives";
import { FileDetail } from "./FileDetail";

function FileRow({
  entry,
  node,
  model,
}: {
  entry: TreeEntry;
  node: ContextNode;
  model: ReviewModel;
}) {
  const [open, setOpen] = useState(false);
  const openable = Boolean(entry.detail);

  return (
    <li className="tree-row" data-open={open} data-depth={entry.depth}>
      <button
        type="button"
        className="tree-row__head"
        data-openable={openable}
        aria-expanded={openable ? open : undefined}
        onClick={() => openable && setOpen((v) => !v)}
      >
        <span className="tree-row__disclosure" aria-hidden="true" />
        <span className="tree-row__name" data-state={node.state}>
          {node.name}
        </span>
        <span className="tree-row__marks">
          <StateBadge state={node.state} />
          {node.needsInput && (
            <span
              className="tree-row__flag"
              aria-label="needs orchestrator input"
            >
              needs input
            </span>
          )}
        </span>
        <span className="tree-row__summary">{node.why}</span>
      </button>

      {open && entry.detail && (
        <div className="tree-row__detail">
          <Annotatable
            target={{
              surfaceId: `file:${node.id}`,
              label: node.name,
              kind: node.kind,
              nodeId: node.id,
            }}
          >
            <FileDetail node={node} detail={entry.detail} model={model} />
          </Annotatable>
        </div>
      )}
    </li>
  );
}

export function FileTree({
  tree,
  model,
}: {
  tree: readonly FileTreeSection[];
  model: ReviewModel;
}) {
  return (
    <div className="tree">
      {tree.map((section, si) => (
        <div key={si} className="tree__group">
          {section.caption && (
            <h3 className="tree__group-label">{section.caption}</h3>
          )}
          <ul className="tree__list">
            {section.entries.map((entry, ei) => {
              const node = model.nodes[entry.nodeId];
              if (!node) return null;
              // A group-header entry is just a directory label inside the tree.
              if (entry.isGroup) {
                return (
                  <li
                    key={`${si}:${ei}`}
                    className="tree__dir"
                    data-depth={entry.depth}
                  >
                    {node.path ?? node.name}
                  </li>
                );
              }
              return (
                <FileRow
                  key={`${si}:${ei}`}
                  entry={entry}
                  node={node}
                  model={model}
                />
              );
            })}
          </ul>
        </div>
      ))}
    </div>
  );
}
