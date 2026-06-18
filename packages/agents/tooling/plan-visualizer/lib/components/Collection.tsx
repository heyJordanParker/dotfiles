/**
 * Collection — a vague count made reviewable. "~40 bespoke verb routes" expands
 * to the actual list, each row a route → destination mapping. A row that carries
 * a `change` is itself openable: clicking it reveals, AS CODE through Pierre, how
 * that route changed — the old route beside the new, or the action code it
 * becomes. The list stops being dead; every item is inspectable.
 *
 * Flat and dense — no box per item, no chevron-character bullet. The disclosure
 * triangle and the one semantic color carry the structure.
 */
import { useState } from "react";
import type {
  Collection as CollectionModel,
  CollectionItem,
} from "../model";
import { groupBy } from "../group";
import { StateBadge, Annotatable } from "./primitives";
import { CodeBlock } from "./CodeBlock";
import { CodeDiff } from "./CodeDiff";

function ItemChange({ item }: { item: CollectionItem }) {
  if (!item.change) return null;
  if (item.change.mode === "diff") {
    return <CodeDiff change={item.change.diff} />;
  }
  return (
    <CodeBlock
      code={item.change.block.code}
      language={item.change.block.language}
      label={item.change.block.label}
    />
  );
}

function Row({ item }: { item: CollectionItem }) {
  const [open, setOpen] = useState(false);
  const openable = Boolean(item.change);

  return (
    <li className="collection__item" data-open={open}>
      <button
        type="button"
        className="collection__row"
        data-state={item.state ?? "changed"}
        data-openable={openable}
        aria-expanded={openable ? open : undefined}
        onClick={() => openable && setOpen((v) => !v)}
      >
        <span className="collection__disclosure" aria-hidden="true" />
        <code className="collection__source">{item.label}</code>
        {item.destination && (
          <>
            <span className="collection__maps" aria-hidden="true">
              →
            </span>
            <code className="collection__dest">{item.destination}</code>
          </>
        )}
        {item.note && <span className="collection__note">{item.note}</span>}
        {item.state && <StateBadge state={item.state} />}
      </button>

      {open && item.change && (
        <div className="collection__detail">
          <ItemChange item={item} />
        </div>
      )}
    </li>
  );
}

export function Collection({
  collection,
}: {
  collection: CollectionModel;
}) {
  const [open, setOpen] = useState(false);
  const total = collection.total ?? collection.items.length;
  return (
    <div className="collection" data-open={open}>
      <button
        type="button"
        className="collection__head"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="collection__disclosure" aria-hidden="true" />
        <span className="collection__summary">{collection.summary}</span>
        <span className="collection__count">{total}</span>
      </button>

      {open && (
        <Annotatable
          target={{
            surfaceId: `collection:${collection.id}`,
            label: collection.summary,
            kind: "collection",
            facts: collection.items.map(
              (it) =>
                `${it.label}${it.destination ? ` → ${it.destination}` : ""}${
                  it.note ? ` (${it.note})` : ""
                }`
            ),
          }}
        >
          <CollectionList items={collection.items} />
        </Annotatable>
      )}
    </div>
  );
}

/** The rows — one flat list, or captioned bands when items carry a `group`. */
function CollectionList({ items }: { items: readonly CollectionItem[] }) {
  const bands = groupBy(items, (it) => it.group);
  if (bands.length === 1) {
    return (
      <ul className="collection__list">
        {items.map((it, i) => (
          <Row key={i} item={it} />
        ))}
      </ul>
    );
  }
  return (
    <div className="collection__bands">
      {bands.map((band, i) => (
        <div key={band.caption ?? i} className="collection__band">
          {band.caption && (
            <span className="collection__band-caption">{band.caption}</span>
          )}
          <ul className="collection__list">
            {band.items.map((it, j) => (
              <Row key={j} item={it} />
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
}
