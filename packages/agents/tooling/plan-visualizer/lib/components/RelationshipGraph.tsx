/**
 * RelationshipGraph — the big-picture map. Nodes are the proposal's real parts;
 * edges are the dependency direction (import: source → target). Color is the
 * change state, the one semantic vocabulary.
 *
 * Two node densities, chosen by how many nodes are on screen:
 *   · DENSE (4+ nodes) — each node is a compact name + subtitle box, sized to its
 *     longest line so nothing truncates and nothing overflows the canvas.
 *   · SPARSE (1–3 nodes) — each node that carries a `signature` expands into a
 *     rich signature card: name, path, and its full method / API lines rendered
 *     as real HTML through an SVG <foreignObject>, so a single-node diagram is a
 *     readable card rather than an empty box. The card's interior is laid out
 *     with a CSS @container query, so it reflows to the room the node is given —
 *     wide cards go two-column, narrow cards stack.
 *
 * Nodes lay out on a grid whose columns are as wide as they need to be; edges
 * route from box edge to box edge. Clicking a node opens a popover carrying the
 * SAME rich file detail a tree row opens.
 *
 * Grouping (the second axis): when the drawn nodes carry `group`, the graph
 * renders one captioned band per group, each band its own grid. The primary axis
 * stays change-state (color); the group is the orthogonal dimension a roadmap or
 * a domain-sliced schema would otherwise lose to prose.
 */
import { useState, useMemo, useRef, useLayoutEffect, type CSSProperties } from "react";
import type { ContextNode, ChangeState, ReviewModel, FileDetail as FileDetailModel } from "../model";
import { fileDetailsByNode } from "../model";
import { groupBy } from "../group";
import { Annotatable, StateBadge } from "./primitives";
import { FileDetail } from "./FileDetail";
import { NeedsInputFlag } from "./NeedsInputFlag";

interface Placed {
  readonly node: ContextNode;
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
  /** A sparse node with a signature renders as a foreignObject card. */
  readonly card: boolean;
}

/* Monospace-ish character widths at the node's font sizes (px per char). */
const TITLE_CH = 8.4; // 14px semibold
const SUB_CH = 6.0; // 10px mono
const SIG_CH = 6.7; // 11px mono signature line
const PAD_X = 16;
const NODE_H = 64;
const GAP_X = 56;
const GAP_Y = 48;
const PAD = 24;
const MIN_W = 130;
/* Card geometry — a sparse signature node. The card is sized to show its
   longest signature IN FULL on one line, so a single-column layout never has to
   wrap or truncate. The two-column container reflow (graph.css) only triggers
   when the card is given enough room for two full signatures side by side —
   which a self-sized card never is — so the default render is one full column
   with no ellipsis. CARD_LINE_H carries a small wrap allowance regardless, so a
   line that does wrap (e.g. inside a wider popover) still has vertical room. */
const CARD_MIN_W = 260;
const CARD_HEAD_H = 56; // name + path
const CARD_LINE_H = 26; // each signature line, with wrap headroom
const CARD_PAD_Y = 18;

/** A diagram is sparse when 1–3 nodes are drawn — then signatures show in-node. */
const SPARSE_MAX = 3;

function subtitle(node: ContextNode): string {
  return node.path ? node.path : node.kind;
}

function isCard(node: ContextNode, sparse: boolean): boolean {
  return sparse && Boolean(node.signature && node.signature.length > 0);
}

/** Width a node needs to show its longest line in full — no truncation. */
function nodeWidth(node: ContextNode, sparse: boolean): number {
  if (isCard(node, sparse)) {
    const lines = [node.name, subtitle(node)];
    const headW = Math.max(...lines.map((l) => l.length)) * TITLE_CH;
    const sigW = Math.max(...node.signature!.map((s) => s.length)) * SIG_CH;
    return Math.max(CARD_MIN_W, Math.ceil(Math.max(headW, sigW)) + PAD_X * 2);
  }
  const titleW = node.name.length * TITLE_CH;
  const subW = subtitle(node).length * SUB_CH;
  return Math.max(MIN_W, Math.ceil(Math.max(titleW, subW)) + PAD_X * 2);
}

function nodeHeight(node: ContextNode, sparse: boolean): number {
  if (isCard(node, sparse)) {
    return CARD_HEAD_H + node.signature!.length * CARD_LINE_H + CARD_PAD_Y;
  }
  return NODE_H;
}

/** Grid layout with per-column widths sized to the widest node in the column. */
function layout(
  nodes: readonly ContextNode[],
  perRow: number,
  sparse: boolean
): { placed: Placed[]; width: number; height: number } {
  const cols = Math.max(1, perRow);
  const widths = nodes.map((n) => nodeWidth(n, sparse));
  const heights = nodes.map((n) => nodeHeight(n, sparse));
  // Column width = widest node assigned to that column.
  const colW: number[] = new Array(cols).fill(MIN_W);
  nodes.forEach((_, i) => {
    const c = i % cols;
    colW[c] = Math.max(colW[c], widths[i]);
  });
  const colX: number[] = [];
  let x = PAD;
  for (let c = 0; c < cols; c++) {
    colX[c] = x;
    x += colW[c] + GAP_X;
  }
  // Row height = tallest node in that row, so card rows get the room they need.
  const rows = Math.ceil(nodes.length / cols);
  const rowH: number[] = new Array(rows).fill(NODE_H);
  nodes.forEach((_, i) => {
    const r = Math.floor(i / cols);
    rowH[r] = Math.max(rowH[r], heights[i]);
  });
  const rowY: number[] = [];
  let y = PAD;
  for (let r = 0; r < rows; r++) {
    rowY[r] = y;
    y += rowH[r] + GAP_Y;
  }
  const placed: Placed[] = nodes.map((node, i) => {
    const c = i % cols;
    const r = Math.floor(i / cols);
    return {
      node,
      x: colX[c],
      y: rowY[r],
      w: widths[i],
      h: heights[i],
      card: isCard(node, sparse),
    };
  });
  const width = x - GAP_X + PAD;
  const height = y - GAP_Y + PAD;
  return { placed, width, height };
}

/** Edge from the nearest edge of a to the nearest edge of b (cleaner routing). */
function edgePath(a: Placed, b: Placed): string {
  const ac = { x: a.x + a.w / 2, y: a.y + a.h / 2 };
  const bc = { x: b.x + b.w / 2, y: b.y + b.h / 2 };
  // Same row → connect horizontal edges; otherwise vertical edges.
  const sameRow = Math.abs(ac.y - bc.y) < Math.min(a.h, b.h);
  let ax: number, ay: number, bx: number, by: number;
  if (sameRow) {
    ay = ac.y;
    by = bc.y;
    if (bc.x >= ac.x) {
      ax = a.x + a.w;
      bx = b.x;
    } else {
      ax = a.x;
      bx = b.x + b.w;
    }
    const mx = (ax + bx) / 2;
    return `M ${ax} ${ay} C ${mx} ${ay}, ${mx} ${by}, ${bx} ${by}`;
  }
  ax = ac.x;
  bx = bc.x;
  if (bc.y >= ac.y) {
    ay = a.y + a.h;
    by = b.y;
  } else {
    ay = a.y;
    by = b.y + b.h;
  }
  const my = (ay + by) / 2;
  return `M ${ax} ${ay} C ${ax} ${my}, ${bx} ${my}, ${bx} ${by}`;
}

const STATES: ChangeState[] = ["added", "changed", "removed", "kept", "separate"];

export function RelationshipGraph({
  nodes,
  compact = false,
  model,
}: {
  nodes: readonly ContextNode[];
  compact?: boolean;
  /** When present, a node click opens the SAME rich file detail the tree shows. */
  model?: ReviewModel;
}) {
  const [openId, setOpenId] = useState<string | null>(null);
  const groups = useMemo(() => groupBy(nodes, (n) => n.group), [nodes]);
  const banded = groups.length > 1;
  const details = useMemo(
    () => (model ? fileDetailsByNode(model) : {}),
    [model]
  );

  return (
    <div className="graph" data-compact={compact} data-banded={banded}>
      {groups.map((band, i) => (
        <GraphBand
          key={band.caption ?? i}
          caption={band.caption}
          nodes={band.items}
          compact={compact}
          openId={openId}
          setOpenId={setOpenId}
          details={details}
          model={model}
        />
      ))}
    </div>
  );
}

/** One grid of nodes — the whole graph when ungrouped, one band when grouped. */
function GraphBand({
  caption,
  nodes,
  compact,
  openId,
  setOpenId,
  details,
  model,
}: {
  caption?: string;
  nodes: readonly ContextNode[];
  compact: boolean;
  openId: string | null;
  setOpenId: (id: string | null) => void;
  details: Readonly<Record<string, FileDetailModel>>;
  model?: ReviewModel;
}) {
  const sparse = nodes.length <= SPARSE_MAX;
  const perRow = compact
    ? Math.min(nodes.length, 2)
    : Math.min(3, Math.max(1, Math.ceil(Math.sqrt(nodes.length))));
  const { placed, width, height } = useMemo(
    () => layout(nodes, perRow, sparse),
    [nodes, perRow, sparse]
  );
  const byId = useMemo(
    () => new Map(placed.map((p) => [p.node.id, p])),
    [placed]
  );

  return (
    <div
      className="graph__band"
      style={{ "--graph-w": `${width}px` } as CSSProperties}
    >
      {caption && <span className="graph__band-caption">{caption}</span>}
      <svg
        className="graph__svg"
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={caption ? `${caption} relationship diagram` : "Relationship diagram"}
        preserveAspectRatio="xMidYMin meet"
      >
        <defs>
          {STATES.map((s) => (
            <marker
              key={s}
              id={`arrow-${s}`}
              viewBox="0 0 10 10"
              refX="8"
              refY="5"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path
                d="M0,0 L10,5 L0,10 z"
                data-state={s}
                className="graph__arrowhead"
              />
            </marker>
          ))}
        </defs>

        {placed.map((p) =>
          (p.node.dependsOn ?? []).map((dep) => {
            const target = byId.get(dep.target);
            if (!target) return null;
            const state = dep.state ?? "separate";
            return (
              <path
                key={`${p.node.id}->${dep.target}`}
                className="graph__edge"
                data-state={state}
                data-separate={state === "separate"}
                d={edgePath(p, target)}
                markerEnd={`url(#arrow-${state})`}
              />
            );
          })
        )}

        {placed.map((p) => {
          const active = p.node.id === openId;
          const openable = !compact;
          return (
            <g
              key={p.node.id}
              className="graph__node"
              data-state={p.node.state}
              data-active={active}
              data-card={p.card}
              transform={`translate(${p.x} ${p.y})`}
              onClick={() => openable && setOpenId(active ? null : p.node.id)}
              role={openable ? "button" : undefined}
              tabIndex={openable ? 0 : undefined}
              aria-label={openable ? `${p.node.name} — open context` : undefined}
              onKeyDown={(e) => {
                if (openable && (e.key === "Enter" || e.key === " ")) {
                  e.preventDefault();
                  setOpenId(active ? null : p.node.id);
                }
              }}
            >
              <rect className="graph__node-box" width={p.w} height={p.h} rx="10" />
              {p.card ? (
                <foreignObject x="0" y="0" width={p.w} height={p.h}>
                  <SignatureCard node={p.node} />
                </foreignObject>
              ) : (
                <>
                  <text className="graph__node-title" x={PAD_X} y="26">
                    {p.node.name}
                  </text>
                  <text className="graph__node-sub" x={PAD_X} y="44">
                    {subtitle(p.node)}
                  </text>
                </>
              )}
            </g>
          );
        })}
      </svg>

      {openId && byId.get(openId) && (
        <NodePopover
          node={byId.get(openId)!.node}
          detail={details[openId]}
          model={model}
          onClose={() => setOpenId(null)}
        />
      )}
    </div>
  );
}

/**
 * The rich in-node card for a sparse diagram. Real HTML through foreignObject so
 * the interior reflows with a CSS @container query — wide cards go two-column,
 * narrow cards stack. Color is the node's change state, the one system.
 */
function SignatureCard({ node }: { node: ContextNode }) {
  return (
    <div className="graph__card" data-state={node.state}>
      <div className="graph__card-head">
        <span className="graph__card-name">{node.name}</span>
        <span className="graph__card-sub">{subtitle(node)}</span>
      </div>
      <ul className="graph__card-sigs">
        {node.signature!.map((sig, i) => (
          <li key={i} className="graph__card-sig">
            {sig}
          </li>
        ))}
      </ul>
    </div>
  );
}

function NodePopover({
  node,
  detail,
  model,
  onClose,
}: {
  node: ContextNode;
  detail?: FileDetailModel;
  model?: ReviewModel;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    ref.current?.focus();
  }, []);
  const rich = Boolean(detail && model);

  return (
    <div className="graph__popover-scrim" onClick={onClose}>
      <div
        ref={ref}
        className="graph__popover"
        data-state={node.state}
        data-rich={rich}
        role="dialog"
        aria-label={`${node.name} context`}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      >
        <Annotatable
          target={{
            surfaceId: `graph:${node.id}`,
            label: node.name,
            kind: node.kind,
            nodeId: node.id,
          }}
        >
          {rich ? (
            <FileDetail node={node} detail={detail!} model={model!} />
          ) : (
            <ContextSummary node={node} />
          )}
        </Annotatable>

        <button
          type="button"
          className="graph__popover-close"
          onClick={onClose}
        >
          Close
        </button>
      </div>
    </div>
  );
}

/** The terse fallback when a node carries no file detail (no public surface). */
function ContextSummary({ node }: { node: ContextNode }) {
  const deps = node.dependsOn ?? [];
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
      {deps.length > 0 && (
        <ul className="graph__popover-deps">
          {deps.map((d, i) => (
            <li
              key={i}
              className="graph__popover-dep"
              data-state={d.state ?? "separate"}
            >
              <span className="graph__popover-dep-label">
                {d.label ?? "depends on"}
              </span>
              <code>{d.target}</code>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
