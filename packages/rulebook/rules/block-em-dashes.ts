import { defineRule, type SgNode } from '@cmodjs/rulebook'

const EM_DASH = String.fromCharCode(0x2014)

const CODE_BLOCKS = { rule: { any: [{ kind: 'fenced_code_block' }, { kind: 'indented_code_block' }] } }

export default defineRule({
  description: "Don't write em dashes.",
  message: 'An em dash joins two ideas in one sentence. Split it into two sentences.',
  files: ['**', '@reply', '@inbox/**'],
  create(context) {
    const { filename, sourceCode } = context
    const dashed = sourceCode.lines.flatMap((line, index) => (line.includes(EM_DASH) ? [index + 1] : []))
    if (dashed.length === 0) return {}
    const isMarkdown = filename.startsWith('@') || /\.(md|mdx|markdown)$/i.test(filename)
    const code = new Set(isMarkdown ? (sourceCode.ast as SgNode).findAll(CODE_BLOCKS).flatMap(linesOf) : [])
    for (const line of dashed) if (!code.has(line)) context.report({ loc: { start: { line, column: sourceCode.lines[line - 1]?.indexOf(EM_DASH) ?? 0 } } })
    return {}
  },
})

function linesOf(block: SgNode): readonly number[] {
  const { start, end } = block.range()
  const last = end.column === 0 ? end.line : end.line + 1
  return Array.from({ length: last - start.line }, (_, index) => start.line + 1 + index)
}
