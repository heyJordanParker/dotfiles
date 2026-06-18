/**
 * MatrixGrid — an interactive two-axis grid. The reader clicks a row to reveal
 * that row's full story in plain language below the grid; the grid itself stays
 * scannable. Color comes from quiet tinted columns (a faint red column for the
 * thing that broke, a faint green column for the thing that closes it) and
 * alternating row bands — NOT red-and-green text in every cell. Cells read in
 * plain language with no codes.
 *
 * A real <table>: semantic, scrollable when wide, sticky row headers, keyboard-
 * reachable rows. Clicking a row toggles its detail; one row open at a time, so
 * the reader digs into exactly the row they want.
 */
import { useState } from "react";
import type { Matrix, MatrixCell } from "../model";
import { Annotatable } from "./primitives";

export function MatrixGrid({ matrix }: { matrix: Matrix }) {
  const [openRow, setOpenRow] = useState<string | null>(null);

  const cellAt = (rowId: string, columnId: string): MatrixCell | undefined =>
    matrix.cells.find((c) => c.rowId === rowId && c.columnId === columnId);

  const activeRow = matrix.rows.find((r) => r.id === openRow) ?? null;

  return (
    <Annotatable
      target={{
        surfaceId: `matrix:${matrix.corner ?? "grid"}`,
        label: matrix.corner ?? "matrix",
        kind: "matrix",
        facts: matrix.rows.flatMap((row) =>
          matrix.columns.map((col) => {
            const cell = cellAt(row.id, col.id);
            return `${row.label} / ${col.label}: ${cell?.value ?? "—"}`;
          })
        ),
      }}
    >
      <div className="matrix">
        <div className="matrix__scroll">
          <table className="matrix__table">
            <thead>
              <tr>
                <th className="matrix__corner" scope="col">
                  {matrix.corner}
                </th>
                {matrix.columns.map((col) => (
                  <th
                    key={col.id}
                    className="matrix__col"
                    scope="col"
                    data-tint={col.tint}
                  >
                    <span className="matrix__col-label">{col.label}</span>
                    {col.note && (
                      <span className="matrix__col-note">{col.note}</span>
                    )}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {matrix.rows.map((row) => {
                const expandable = Boolean(row.detail);
                const isOpen = openRow === row.id;
                return (
                  <tr
                    key={row.id}
                    className="matrix__line"
                    data-open={isOpen}
                    data-expandable={expandable}
                    onClick={() =>
                      expandable && setOpenRow(isOpen ? null : row.id)
                    }
                  >
                    <th
                      className="matrix__row"
                      scope="row"
                      tabIndex={expandable ? 0 : undefined}
                      role={expandable ? "button" : undefined}
                      aria-expanded={expandable ? isOpen : undefined}
                      onKeyDown={(e) => {
                        if (!expandable) return;
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          setOpenRow(isOpen ? null : row.id);
                        }
                      }}
                    >
                      {expandable && (
                        <span
                          className="matrix__disclosure"
                          aria-hidden="true"
                        />
                      )}
                      <span className="matrix__row-text">
                        <span className="matrix__row-label">{row.label}</span>
                        {row.note && (
                          <span className="matrix__row-note">{row.note}</span>
                        )}
                      </span>
                    </th>
                    {matrix.columns.map((col) => {
                      const cell = cellAt(row.id, col.id);
                      return (
                        <td
                          key={col.id}
                          className="matrix__cell"
                          data-tint={col.tint}
                          data-empty={!cell?.value}
                        >
                          {cell?.value}
                        </td>
                      );
                    })}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        {activeRow?.detail && (
          <div className="matrix__detail">
            <span className="matrix__detail-label">{activeRow.label}</span>
            <p className="matrix__detail-text">{activeRow.detail}</p>
          </div>
        )}
      </div>
    </Annotatable>
  );
}
