/**
 * DatabasePanel — the database section, one table at a time. Each table renders
 * as real migration DDL (the Schema::create / Blueprint, or raw SQL) through the
 * Pierre viewer, with every column's note as an inline `//` comment. A changed
 * table renders as a before/after DDL diff. No boxed column rows — the schema
 * IS code, so it reads as code.
 */
import { useState } from "react";
import type { DbTable, ReviewModel } from "../model";
import { groupBy } from "../group";
import { StateBadge, Annotatable } from "./primitives";
import { CodeBlock } from "./CodeBlock";
import { CodeDiff } from "./CodeDiff";

/** A tab — its data-active is keyed off identity, not index, so banding works. */
function Tab({
  table,
  active,
  name,
  onSelect,
}: {
  table: DbTable;
  active: boolean;
  name: string;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      className="db__tab"
      data-state={table.state}
      data-active={active}
      onClick={onSelect}
    >
      <span className="db__tab-name">{name}</span>
      <StateBadge state={table.state} />
    </button>
  );
}

export function DatabasePanel({
  tables,
  model,
}: {
  tables: readonly DbTable[];
  model: ReviewModel;
}) {
  const [activeId, setActiveId] = useState(tables[0]?.nodeId);
  const table = tables.find((t) => t.nodeId === activeId) ?? tables[0];
  const node = model.nodes[table.nodeId];
  const language = table.language ?? "php";
  const bands = groupBy(tables, (t) => t.group);
  const nameOf = (t: DbTable) => model.nodes[t.nodeId]?.name ?? t.nodeId;

  return (
    <div className="db" data-banded={bands.length > 1}>
      {bands.length === 1 ? (
        <div className="db__tabs" role="tablist" aria-label="Database tables">
          {tables.map((t) => (
            <Tab
              key={t.nodeId}
              table={t}
              name={nameOf(t)}
              active={t.nodeId === table.nodeId}
              onSelect={() => setActiveId(t.nodeId)}
            />
          ))}
        </div>
      ) : (
        <div className="db__bands">
          {bands.map((band, i) => (
            <div key={band.caption ?? i} className="db__band">
              {band.caption && (
                <span className="db__band-caption">{band.caption}</span>
              )}
              <div
                className="db__tabs"
                role="tablist"
                aria-label={`${band.caption ?? "Tables"} tables`}
              >
                {band.items.map((t) => (
                  <Tab
                    key={t.nodeId}
                    table={t}
                    name={nameOf(t)}
                    active={t.nodeId === table.nodeId}
                    onSelect={() => setActiveId(t.nodeId)}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {node && (
        <Annotatable
          target={{
            surfaceId: `db:${table.nodeId}`,
            label: node.name,
            kind: "table",
            nodeId: table.nodeId,
            facts: [node.why, ...(node.usage ?? [])],
          }}
        >
          <div className="db__panel">
            {table.ddlBefore ? (
              <CodeDiff
                change={{
                  language,
                  before: table.ddlBefore,
                  after: table.ddl,
                  layout: "split",
                  beforeLabel: `${node.name} — before`,
                  afterLabel: `${node.name} — after`,
                }}
              />
            ) : (
              <CodeBlock code={table.ddl} language={language} label={node.name} />
            )}
          </div>
        </Annotatable>
      )}
    </div>
  );
}
