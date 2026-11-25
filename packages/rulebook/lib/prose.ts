import type { SgNode } from '@cmodjs/rulebook'

const CODE_BLOCKS = { rule: { any: [{ kind: 'fenced_code_block' }, { kind: 'indented_code_block' }] } }

export function codeLines(ast: SgNode): Set<number> {
  return new Set(ast.findAll(CODE_BLOCKS).flatMap(linesOf))
}

export function findInProse(source: { ast: unknown; lines: readonly string[] }, pattern: RegExp): { line: number; column: number }[] {
  const code = codeLines(source.ast as SgNode)
  return source.lines.flatMap((text, index) => {
    if (code.has(index + 1)) return []
    const match = pattern.exec(text.replace(/`[^`]*`|"[^"]*"/g, (span) => ' '.repeat(span.length)))
    return match ? [{ line: index + 1, column: match.index }] : []
  })
}

function linesOf(block: SgNode): readonly number[] {
  const { start, end } = block.range()
  const last = end.column === 0 ? end.line : end.line + 1
  return Array.from({ length: last - start.line }, (_, index) => start.line + 1 + index)
}
