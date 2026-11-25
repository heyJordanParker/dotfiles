import { defineRule, type SgNode } from '@cmodjs/rulebook'

const CLOCKS: ReadonlySet<string> = new Set(['sleep', 'usleep', 'nanosleep', 'sleepSync', 'time_nanosleep', 'time_sleep_until', 'setTimeout', 'waitForTimeout'])

const POLLS: ReadonlySet<string> = new Set([
  'poll',
  'toPass',
  'eventually',
  'until',
  'whenAvailable',
  'waitFor',
  'waitForFunction',
  'waitForSelector',
  'waitForElementToBeRemoved',
  'waitUntil',
  'waitUsing',
  'waitForText',
  'waitForTextIn',
  'waitForReload',
  'clickAndWaitForReload',
  'waitForDialog',
  'waitForLink',
  'waitForInput',
  'waitForLocation',
  'waitForRoute',
  'waitUntilMissing',
  'waitUntilMissingText',
  'waitUntilEnabled',
  'waitUntilDisabled',
  'waitUntilVue',
  'waitUntilVueIsNot',
  'waitForDisplayed',
  'waitForExist',
  'waitForClickable',
  'waitForEnabled',
  'waitForStable',
])

const DURATIONS: ReadonlySet<string> = new Set(['wait', 'pause', 'delay'])

const SPAWNS: ReadonlySet<string> = new Set(['$', 'spawn', 'spawnSync', 'exec', 'execSync', 'execFile', 'execFileSync', 'execa', 'shell_exec', 'system', 'passthru', 'proc_open', 'popen', 'run', 'call', 'check_call', 'check_output', 'Popen', 'create_subprocess_exec', 'create_subprocess_shell'])

const NAME_FIELDS: readonly string[] = ['property', 'attribute', 'field', 'name']

const NUMBERS: ReadonlySet<string> = new Set(['number', 'integer', 'float'])

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
  const callee = call.field('name') ?? call.field('function')
  if (callee === null) return false
  const name = nameOf(callee)
  const args = call.field('arguments')
  if (CLOCKS.has(name) || POLLS.has(name)) return true
  if (DURATIONS.has(name)) return NUMBERS.has(String(firstArgument(args)?.kind()))
  const scope = call.field('scope')
  if (scope !== null && nameOf(scope) === 'Sleep') return true
  return SPAWNS.has(name) && args !== null && firstString(args)?.trim().split(' ')[0] === 'sleep'
}

function nameOf(node: SgNode): string {
  for (const field of NAME_FIELDS) {
    const part = node.field(field)
    if (part !== null) return nameOf(part)
  }
  const last = node.kind() === 'qualified_name' ? node.children().findLast((child) => child.isNamed()) : undefined
  return last === undefined ? node.text() : nameOf(last)
}

function firstArgument(args: SgNode | null): SgNode | undefined {
  const first = args?.children().find((child) => child.isNamed())
  return first?.kind() === 'argument' ? first.children().find((child) => child.isNamed()) : first
}

function firstString(node: SgNode): string | undefined {
  if (STRINGS.has(String(node.kind()))) return node.text()
  for (const child of node.children()) {
    const found = firstString(child)
    if (found !== undefined) return found
  }
  return undefined
}
