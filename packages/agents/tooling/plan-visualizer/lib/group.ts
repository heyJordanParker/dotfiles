/**
 * groupBy — the second axis, shared. Several sections (decisions, the collection
 * list, database tabs, standing constraints, graph nodes) can carry an optional
 * `group` on each item — a tier, a wave, a domain. When any item carries one,
 * the section renders captioned bands in first-seen group order, preserving the
 * authored item order inside each band; when none do, it stays one flat list.
 *
 * One implementation, so the grouping rule (and the "no group at all → flat"
 * fallback) can never drift between the sections that use it.
 */
export interface Band<T> {
  readonly caption?: string;
  readonly items: readonly T[];
}

export function groupBy<T>(
  items: readonly T[],
  groupOf: (item: T) => string | undefined
): Band<T>[] {
  const anyGrouped = items.some((it) => groupOf(it));
  if (!anyGrouped) return [{ items }];

  const order: string[] = [];
  const byGroup = new Map<string, T[]>();
  for (const it of items) {
    const key = groupOf(it) ?? "—";
    if (!byGroup.has(key)) {
      byGroup.set(key, []);
      order.push(key);
    }
    byGroup.get(key)!.push(it);
  }
  return order.map((caption) => ({ caption, items: byGroup.get(caption)! }));
}
