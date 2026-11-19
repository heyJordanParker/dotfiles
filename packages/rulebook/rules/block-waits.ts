import { defineRule, type SgNode } from '@cmodjs/rulebook'

const WAITS = /^(u?sleep|sleepSync|nanosleep|time_nanosleep|time_sleep_until|setTimeout|delay|until|poll|eventually|toPass|wait(?:[A-Z_]\w*)?)$/

const SPAWNS = /^(\$|spawn|spawnSync|exec|execSync|execFile|execFileSync|execa|shell_exec|system|passthru|proc_open|popen|run|call|check_call|check_output|Popen|create_subprocess_exec|create_subprocess_shell)$/

const STRINGS: ReadonlySet<string> = new Set(['string_fragment', 'string_content'])

export default defineRule({
  description: "Don't make a test or a command wait.",
  files: [
    '**/*.{test,spec,cy}.{ts,tsx,mts,cts,js,jsx,mjs,cjs}',
    '**/*Test.php',
    '**/*.phpt',
    '**/test_*.py',
    '**/*_test.py',
    '**/conftest.py',
    '**/{test,tests,spec,__tests__,e2e,cypress}/**',
    '@command',
  ],
  create(context) {
    const reported = new Set<number>()
    const report = (node: SgNode): void => {
      const line = node.range().start.line
      if (reported.has(line)) return
      reported.add(line)
      context.report({ node })
    }
    const check = (call: SgNode): void => {
      if (isWait(call)) report(call)
    }
    return {
      call_expression: check,
      call: check,
      function_call_expression: check,
      member_call_expression: check,
      nullsafe_member_call_expression: check,
      scoped_call_expression: check,
      command(command: SgNode) {
        if (command.field('name')?.text() === 'sleep') report(command)
      },
      Command(command) {
        if (command.program === 'sleep') context.report({ node: command })
      },
    }
  },
})

function isWait(call: SgNode): boolean {
  const args = call.field('arguments')
  const callee = call.text().slice(0, (args?.range().start.index ?? call.range().end.index) - call.range().start.index).trim()
  const name = call.field('name')?.text() ?? /[A-Za-z_$][\w$]*$/.exec(callee)?.[0] ?? callee
  if (WAITS.test(name)) return true
  if (name === 'pause') return /^\(\s*\d/.test(args?.text() ?? '')
  if (callee.includes('::') && /(^|\\)Sleep$/.test(callee.split('::')[0] ?? '')) return true
  return SPAWNS.test(name) && args !== null && /^sleep(\s|$)/.test(firstString(args) ?? '')
}

function firstString(node: SgNode): string | undefined {
  if (STRINGS.has(String(node.kind()))) return node.text()
  for (const child of node.children()) {
    const found = firstString(child)
    if (found !== undefined) return found
  }
  return undefined
}
