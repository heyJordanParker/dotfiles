import { basename } from 'node:path'
import { defineRule } from '@cmodjs/rulebook'
import { segments, tokens } from '../../lib/shell.ts'

type Assignment = readonly [string, string]

type Effective = { readonly assignments: readonly Assignment[]; readonly executed: readonly string[]; readonly nested: string }

const ASSIGNMENT = /^([A-Za-z_][A-Za-z0-9_]*)=([\s\S]*)$/
const SHELLS: ReadonlySet<string> = new Set(['zsh', 'bash', 'sh', 'dash', 'ksh', 'fish'])
const SHELL_COMMAND_FLAG = /^-[a-z]*c$/
const RESERVED: ReadonlySet<string> = new Set(['if', 'then', 'elif', 'else', 'fi', 'while', 'until', 'do', 'done', 'for', 'case', 'esac', 'select', 'function', '{', '}', '!', '[[', ']]'])
const PREFIXES: ReadonlySet<string> = new Set(['command', 'builtin', 'exec', 'nohup', 'sudo', 'doas', 'time', 'setsid', 'caffeinate', 'stdbuf', 'timeout', 'xargs', 'nice', 'ionice', 'unbuffer', 'script', 'strace', 'dtruss'])
const PREFIX_ARGUMENT = /^\d+(\.\d+)?[smhd]?$/
const PREFIX_FLAGS_WITH_VALUE: ReadonlySet<string> = new Set(['-u', '--user', '-n', '-k', '--kill-after', '-s', '--signal', '-I', '-i'])
const RUNNERS: ReadonlySet<string> = new Set(['npx', 'bunx', 'pnpx', 'uvx'])
const RUNNER_PAIRS: ReadonlySet<string> = new Set(['npm exec', 'npm run', 'pnpm exec', 'pnpm run', 'pnpm dlx', 'yarn run', 'yarn dlx', 'bun run', 'bun x', 'deno run', 'uv run', 'poetry run', 'pdm run', 'rye run'])
const FLAGS_WITH_VALUE: ReadonlySet<string> = new Set(['-p', '--package', '--with', '--with-editable', '--from', '--allow-run', '--project', '--python', '--directory', '--cwd', '--prefix', '--workspace', '-w', '--filter', '--index-url', '--node-modules-dir', '--config-file'])
const COMMAND_FLAGS: ReadonlySet<string> = new Set(['-c', '--call'])
const PYTHONS: ReadonlySet<string> = new Set(['python', 'python2', 'python3', 'py'])
const INTERPRETERS: ReadonlySet<string> = new Set(['node', 'deno', 'bun', 'ruby', 'php', 'perl', 'osascript', ...PYTHONS])
const PAYLOAD_FLAGS: ReadonlySet<string> = new Set(['-e', '--eval', '-p', '--print', '-c', '-r', '--command'])
const DRIVERS: ReadonlySet<string> = new Set(['chromedriver', 'geckodriver', 'msedgedriver', 'safaridriver', 'operadriver'])
const BROWSER_EXECUTABLE = /^(google[ _-]?chrome(-stable)?|chrome|chromium(-browser)?|firefox(-bin)?|safari|microsoft[ _-]?edge|msedge|brave(-browser)?|thorium|waterfox|librewolf)([ _-](canary|beta|dev|developer[ _-]edition|nightly|stable))*$/i
const BROWSER_APPLICATION = /^(google[ _-]?chrome(-stable)?|chrome|chromium(-browser)?|firefox(-bin)?|safari(-technology[ _-]preview)?|microsoft[ _-]?edge|msedge|edge|brave(-browser)?|thorium|waterfox|librewolf|arc|opera(-gx)?|vivaldi|orion|zen(-browser)?|dia|tor[ _-]browser|duckduckgo|min|sigmaos)([ _-](canary|beta|dev|developer[ _-]edition|nightly|stable|browser))*$/i
const BROWSER_BUNDLE_ID = /^(com\.google\.chrome|com\.apple\.safari|org\.mozilla\.firefox|com\.microsoft\.edgemac|com\.brave\.browser|org\.chromium)/i
const URL = /^(https?:\/\/|www\.)/i
const INFORMATIONAL: ReadonlySet<string> = new Set(['--version', '-v', '-V', '--help', '-h', '--product-version', 'help'])
const PLAYWRIGHT_LAUNCHES: ReadonlySet<string> = new Set(['open', 'codegen', 'screenshot', 'pdf', 'launch-server', 'cr', 'ff', 'wk'])
const PAYLOAD_PATTERNS: readonly RegExp[] = [
  /chromium\.launch/i,
  /firefox\.launch/i,
  /webkit\.launch/i,
  /launchPersistentContext/i,
  /connectOverCDP/i,
  /puppeteer\.launch/i,
  /(require\s*\(|import\s+|from\s+)\s*['"]?(node:)?(@playwright\/test|playwright[\w.]*|puppeteer(-core)?|selenium[\w.]*|pyppeteer|cypress|webdriverio|splinter|helium)\b/i,
  /\bwebdriver\.\w+\(/i,
  /\/(Google Chrome|Chromium|Firefox|Safari)\.app\//i,
  /--remote-debugging-port/i,
  /--dump-dom/i,
  /--screenshot=/i,
  /\b(sync|async)_playwright\b/i,
  /\b(chromedriver|geckodriver)\b.{0,40}\b(Service|start|launch)\b/i,
]
const CHANNEL_CHROME = /channel\s*[:=]\s*['"]?chrome/i
const HEREDOC = /<<-?\s*(['"]?)(\w+)\1([\s\S]*?)^\s*\2\s*$/gm
const SINGLE_QUOTED = /'[^']*'/g
const BACKTICK = /(?<!\\)`([^`]*)`/g
const LOOKUPS: ReadonlySet<string> = new Set(['which', 'command', 'type', 'whence', 'whereis'])
const PLACEHOLDER = /^__substitution(\d+)__$/
const VARIABLE = /^\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?$/
const BROWSER_WORDS = /chrom|firefox|safari|webkit|puppeteer|playwright|selenium|webdriver|cypress|edge|browser/i

export default defineRule({
  description: 'Drive browsers only through agent-browser, headless.',
  message: 'Read the usage docs with agent-browser skills get core, then use its open, snapshot, click, fill, screenshot, and close. Never launch Chrome, Playwright, Puppeteer, or any other browser yourself, and never pass --headed.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (launchesBrowser(context.text, 0)) context.report({ node: document })
      },
    }
  },
})

function launchesBrowser(command: string, depth: number): boolean {
  if (depth > 4) return false
  const { bodies, kept } = heredocs(command)
  for (const [words, body] of bodies) {
    const [executed] = effective(words).executed
    if (executed !== undefined && INTERPRETERS.has(baseOf(executed)) && payloadLaunches(body)) return true
  }
  let text = kept
  const literal = text.replace(SINGLE_QUOTED, (span) => ' '.repeat(span.length))
  const found = substitutions(text)
  const resolved = new Map<number, boolean>()
  for (let index = found.length - 1; index >= 0; index -= 1) {
    const [start, end, body] = found[index] ?? [0, 0, '']
    const isExecuted = literal.charAt(start) !== ' '
    if (isExecuted && launchesBrowser(body, depth + 1)) return true
    resolved.set(index, isExecuted && resolvesToBrowser(body))
    text = `${text.slice(0, start)} __substitution${index}__ ${text.slice(end)}`
  }
  const parsed = segments(text)
  if (parsed === undefined) return BROWSER_WORDS.test(command)
  const browserVariables = new Set(parsed.flatMap(({ words }) => effective(words).assignments.filter(([, value]) => unquoted(value).includes('/') && isBrowser(value)).map(([name]) => name)))
  return parsed.some(({ words }) => launches(words, depth, resolved, browserVariables))
}

function launches(words: readonly string[], depth: number, resolved: ReadonlyMap<number, boolean>, browserVariables: ReadonlySet<string>): boolean {
  const { assignments, executed, nested } = effective(words)
  const environment = new Map(assignments)
  if (nested !== '') return launchesBrowser(nested, depth + 1)
  const [first, sub = ''] = executed
  if (first === undefined) return false
  const head = baseOf(first)
  if (SHELLS.has(head)) {
    const flag = executed.findIndex((word, index) => index > 0 && SHELL_COMMAND_FLAG.test(word))
    return flag > 0 && launchesBrowser(executed.slice(flag + 1).join(' '), depth + 1)
  }
  const variable = VARIABLE.exec(unquoted(first))
  if (variable?.[1] !== undefined && browserVariables.has(variable[1])) return true
  const placeholder = PLACEHOLDER.exec(head)
  if (placeholder?.[1] !== undefined) return resolved.get(Number(placeholder[1])) === true
  const tool = toolOf(first)
  const rest = executed.slice(1)
  if (isBrowser(first)) return rest.some((word) => !INFORMATIONAL.has(word)) || rest.length === 0
  if (tool === 'open') return opensBrowser(executed)
  if (tool === 'agent-browser') {
    const isOpening = rest.includes('open')
    if (isOpening && unquoted(environment.get('AGENT_BROWSER_HEADED') ?? 'false') !== 'false') return true
    return isHeaded(executed) || (isOpening && (executed.includes('--config') || environment.has('AGENT_BROWSER_CONFIG')))
  }
  if (tool === 'playwright' || tool === '@playwright/test') return PLAYWRIGHT_LAUNCHES.has(sub)
  if (tool === 'cypress') return sub === 'open'
  if (tool === 'puppeteer' || tool === 'puppeteer-core') return true
  if (INTERPRETERS.has(head)) {
    const payload = scriptPayload(executed)
    if (payload !== '') return payloadLaunches(payload)
  }
  return false
}

function effective(words: readonly string[]): Effective {
  const assignments: Assignment[] = []
  let index = 0
  for (let match = ASSIGNMENT.exec(words[index] ?? ''); match !== null; match = ASSIGNMENT.exec(words[index] ?? '')) {
    assignments.push([match[1] ?? '', match[2] ?? ''])
    index += 1
  }
  while (index < words.length) {
    const head = baseOf(words[index] ?? '')
    if (RESERVED.has(head)) {
      index += 1
      continue
    }
    if (head === 'env' || PREFIXES.has(head)) {
      index = pastPrefixArguments(words, index + 1)
      for (let match = ASSIGNMENT.exec(words[index] ?? ''); match !== null && index < words.length; match = ASSIGNMENT.exec(words[index] ?? '')) {
        assignments.push([match[1] ?? '', match[2] ?? ''])
        index += 1
      }
      continue
    }
    if (RUNNERS.has(head)) index += 1
    else if (index + 1 < words.length && RUNNER_PAIRS.has(`${head} ${words[index + 1]}`)) index += 2
    else if (PYTHONS.has(head) && words[index + 1] === '-m') index += 2
    else break
    while (index < words.length && (words[index] ?? '').startsWith('-')) {
      const flag = words[index] ?? ''
      if (COMMAND_FLAGS.has(flag)) return { assignments, executed: [], nested: words.slice(index + 1, index + 2).join(' ') }
      index += FLAGS_WITH_VALUE.has(flag) ? 2 : 1
    }
  }
  return { assignments, executed: words.slice(index), nested: '' }
}

function pastPrefixArguments(words: readonly string[], start: number): number {
  let index = start
  while (index < words.length) {
    const word = words[index] ?? ''
    if (PREFIX_FLAGS_WITH_VALUE.has(word)) index += 2
    else if (word.startsWith('-') || PREFIX_ARGUMENT.test(word)) index += 1
    else break
  }
  return index
}

function scriptPayload(words: readonly string[]): string {
  for (let index = 1; index < words.length; index += 1) {
    const word = words[index] ?? ''
    if (PAYLOAD_FLAGS.has(word)) return words.slice(index + 1).join(' ')
    const equals = word.indexOf('=')
    if (equals >= 0 && PAYLOAD_FLAGS.has(word.slice(0, equals))) return [word.slice(equals + 1), ...words.slice(index + 1)].join(' ')
  }
  return ''
}

function payloadLaunches(payload: string): boolean {
  return (CHANNEL_CHROME.test(payload) && /launch/i.test(payload)) || PAYLOAD_PATTERNS.some((pattern) => pattern.test(payload))
}

function isHeaded(words: readonly string[]): boolean {
  for (let index = 0; index < words.length; index += 1) {
    const word = words[index] ?? ''
    if (word === '--headed') return index + 1 >= words.length || words[index + 1] !== 'false'
    if (word.startsWith('--headed=')) return word.slice('--headed='.length) !== 'false'
  }
  return false
}

function opensBrowser(words: readonly string[]): boolean {
  for (let index = 1; index < words.length; index += 1) {
    const word = unquoted(words[index] ?? '')
    const previous = words[index - 1] ?? ''
    if (previous === '-a' || previous === '-b') {
      if (isBrowserApplication(word) || BROWSER_BUNDLE_ID.test(word)) return true
    } else if (baseOf(word).endsWith('.app') && isBrowserApplication(word)) return true
  }
  const isNamed = words.some((word) => word === '-a' || word === '-b')
  return !isNamed && words.slice(1).some((word) => URL.test(unquoted(word)))
}

function substitutions(text: string): readonly (readonly [number, number, string])[] {
  const found: [number, number, string][] = []
  for (const opener of ['$(', '<(', '>(']) {
    for (let index = text.indexOf(opener); index >= 0; index = text.indexOf(opener, index)) {
      if (index > 0 && text.charAt(index - 1) === '\\') {
        index += 2
        continue
      }
      let depth = 0
      let end = index + 1
      for (; end < text.length; end += 1) {
        if (text.charAt(end) === '(') depth += 1
        else if (text.charAt(end) === ')') {
          depth -= 1
          if (depth === 0) break
        }
      }
      if (end >= text.length) break
      found.push([index, end + 1, text.slice(index + 2, end)])
      index = end + 1
    }
  }
  for (const match of text.matchAll(BACKTICK)) found.push([match.index, match.index + match[0].length, match[1] ?? ''])
  return found.sort(([a], [b]) => a - b)
}

function resolvesToBrowser(body: string): boolean {
  const words = (tokens(body) ?? []).map(({ text }) => text)
  return words.length > 1 && LOOKUPS.has(baseOf(words[0] ?? '')) && words.slice(1).some(isBrowser)
}

function heredocs(command: string): { readonly bodies: readonly (readonly [readonly string[], string])[]; readonly kept: string } {
  const bodies: [readonly string[], string][] = []
  const kept: string[] = []
  let last = 0
  for (const match of command.matchAll(HEREDOC)) {
    const owner = command.slice(last, match.index).split(/[;&|\n]/).at(-1) ?? ''
    bodies.push([(tokens(owner) ?? []).map(({ text }) => text), match[3] ?? ''])
    kept.push(command.slice(last, match[1] === '' ? match.index + match[0].length : match.index))
    last = match.index + match[0].length
  }
  kept.push(command.slice(last))
  return { bodies, kept: kept.join(' ') }
}

function isBrowser(token: string): boolean {
  const name = applicationName(token)
  return DRIVERS.has(name) || BROWSER_EXECUTABLE.test(name)
}

function isBrowserApplication(token: string): boolean {
  const name = applicationName(token)
  return DRIVERS.has(name) || BROWSER_APPLICATION.test(name)
}

function applicationName(token: string): string {
  const name = baseOf(token)
  return name.endsWith('.app') ? name.slice(0, -4) : name
}

function toolOf(token: string): string {
  let tool = unquoted(token)
  for (const prefix of ['npm:', 'jsr:']) if (tool.toLowerCase().startsWith(prefix)) tool = tool.slice(prefix.length)
  tool = basename(tool).toLowerCase().replace(/(?<=.)@[^/]*$/, '')
  return tool.endsWith('.exe') ? tool.slice(0, -4) : tool
}

function baseOf(token: string): string {
  return basename(unquoted(token)).toLowerCase()
}

function unquoted(token: string): string {
  return token.replace(/^["']+|["']+$/g, '')
}
