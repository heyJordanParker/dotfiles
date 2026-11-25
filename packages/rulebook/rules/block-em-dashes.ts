import { defineRule, type SgNode } from '@cmodjs/rulebook'
import { codeLines } from '../lib/prose.ts'

const EM_DASH = String.fromCharCode(0x2014)

export default defineRule({
  description: "Don't write em dashes.",
  message: 'An em dash joins two ideas in one sentence. Split it into two sentences.',
  files: ['**', '@reply', '@inbox/**'],
  create(context) {
    const { filename, sourceCode } = context
    const dashed = sourceCode.lines.flatMap((line, index) => (line.includes(EM_DASH) ? [index + 1] : []))
    if (dashed.length === 0) return {}
    const isMarkdown = filename.startsWith('@') || /\.(md|mdx|markdown)$/i.test(filename)
    const code = isMarkdown ? codeLines(sourceCode.ast as SgNode) : new Set<number>()
    for (const line of dashed) if (!code.has(line)) context.report({ loc: { start: { line, column: sourceCode.lines[line - 1]?.indexOf(EM_DASH) ?? 0 } } })
    return {}
  },
})
