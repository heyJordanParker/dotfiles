/**
 * Annotations — the built-in annotate-then-copy flow. No server, no MCP.
 *
 * Clicking any annotatable surface opens a composer. The architect writes a
 * note; copying yields self-contained text an agent that never saw this
 * document can act on. "Self-contained" is the whole point: the copied block
 * carries the concrete target (which file, method, table, column, or decision),
 * that target's context defined once on its node (relationship, why, usage),
 * and the architect's note — never a reference internal to the document like
 * "see the diagram above" or "section 3".
 */
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type { ReviewModel, ContextNode } from "./model";

/** What an annotation points at — enough to rebuild context with no document. */
export interface AnnotationTarget {
  /** A stable id for this surface, so notes survive re-render. */
  readonly surfaceId: string;
  /** Human label of the target, e.g. "Grant.php · level()". */
  readonly label: string;
  /** The kind of thing, for the copied header. */
  readonly kind: string;
  /** The node whose context this surface belongs to, when there is one. */
  readonly nodeId?: string;
  /** Extra concrete facts to carry — a column, a signature, a decision body. */
  readonly facts?: readonly string[];
}

export interface Annotation extends AnnotationTarget {
  readonly note: string;
}

interface AnnotationContextValue {
  readonly model: ReviewModel;
  readonly annotations: Readonly<Record<string, Annotation>>;
  /** The surface currently being composed, if any. */
  readonly composing: AnnotationTarget | null;
  open(target: AnnotationTarget): void;
  close(): void;
  save(note: string): void;
  remove(surfaceId: string): void;
  /** Build the self-contained copy text for one annotation. */
  buildCopyText(surfaceId: string): string;
  /** Build the copy text for every annotation, as a handoff packet. */
  buildAllCopyText(): string;
}

const Ctx = createContext<AnnotationContextValue | null>(null);

function nodeContextBlock(node: ContextNode): string {
  const lines: string[] = [];
  lines.push(`Target: ${node.name}${node.path ? ` (${node.path})` : ""}`);
  lines.push(`Status: ${node.state}`);
  lines.push(`Role: ${node.summary}`);
  lines.push(`Why: ${node.why}`);
  if (node.usage && node.usage.length > 0) {
    lines.push("Used for:");
    for (const u of node.usage) lines.push(`  - ${u}`);
  }
  if (node.dependsOn && node.dependsOn.length > 0) {
    const deps = node.dependsOn
      .map((d) => `${d.label ?? "depends on"} ${d.target}`)
      .join("; ");
    lines.push(`Depends on: ${deps}`);
  }
  return lines.join("\n");
}

function buildOne(model: ReviewModel, ann: Annotation): string {
  const blocks: string[] = [];
  blocks.push(`# Architecture review comment`);
  blocks.push(`From the proposal: "${model.title}"`);
  blocks.push("");
  blocks.push(`## On: ${ann.label} [${ann.kind}]`);
  const node = ann.nodeId ? model.nodes[ann.nodeId] : undefined;
  if (node) {
    blocks.push("");
    blocks.push(nodeContextBlock(node));
  }
  if (ann.facts && ann.facts.length > 0) {
    blocks.push("");
    blocks.push("Concrete details:");
    for (const f of ann.facts) blocks.push(`  - ${f}`);
  }
  blocks.push("");
  blocks.push(`## Comment`);
  blocks.push(ann.note);
  blocks.push("");
  blocks.push(
    `(This comment is self-contained: it names the exact target and carries its ` +
      `surrounding architecture, so it can be acted on without the original document.)`
  );
  return blocks.join("\n");
}

export function AnnotationProvider({
  model,
  children,
}: {
  model: ReviewModel;
  children: ReactNode;
}) {
  const [annotations, setAnnotations] = useState<
    Record<string, Annotation>
  >({});
  const [composing, setComposing] = useState<AnnotationTarget | null>(null);

  const open = useCallback((target: AnnotationTarget) => {
    setComposing(target);
  }, []);

  const close = useCallback(() => setComposing(null), []);

  const save = useCallback(
    (note: string) => {
      setComposing((target) => {
        if (target && note.trim()) {
          setAnnotations((prev) => ({
            ...prev,
            [target.surfaceId]: { ...target, note: note.trim() },
          }));
        }
        return null;
      });
    },
    []
  );

  const remove = useCallback((surfaceId: string) => {
    setAnnotations((prev) => {
      const next = { ...prev };
      delete next[surfaceId];
      return next;
    });
  }, []);

  const buildCopyText = useCallback(
    (surfaceId: string) => {
      const ann = annotations[surfaceId];
      return ann ? buildOne(model, ann) : "";
    },
    [annotations, model]
  );

  const buildAllCopyText = useCallback(() => {
    const all = Object.values(annotations);
    if (all.length === 0) return "";
    const header = [
      `# Architecture review — ${all.length} comment${all.length === 1 ? "" : "s"}`,
      `Proposal: "${model.title}"`,
      "",
      `Each comment below names its own target and carries that target's context,`,
      `so any one can be handed to an agent on its own.`,
      "",
      "---",
      "",
    ].join("\n");
    return header + all.map((a) => buildOne(model, a)).join("\n\n---\n\n");
  }, [annotations, model]);

  const value = useMemo<AnnotationContextValue>(
    () => ({
      model,
      annotations,
      composing,
      open,
      close,
      save,
      remove,
      buildCopyText,
      buildAllCopyText,
    }),
    [
      model,
      annotations,
      composing,
      open,
      close,
      save,
      remove,
      buildCopyText,
      buildAllCopyText,
    ]
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useAnnotations(): AnnotationContextValue {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useAnnotations must be used within AnnotationProvider");
  return ctx;
}
