import { realpathSync, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { basename, dirname, isAbsolute, join, normalize, relative, resolve } from 'node:path'
import type { Command, Context } from '@cmodjs/rulebook'

export type Token = { readonly text: string; readonly isOperator: boolean }

export type Segment = { readonly words: readonly string[]; readonly piped: boolean }

const OPERATOR_CHARACTERS = ';&|()<>\n'

const SEPARATOR_CHARACTERS = ';&|()\n'

const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/

const NUMERIC = /^\d+(?:\.\d+)?[smhd]?$/

export const SHELLS: ReadonlySet<string> = new Set(['bash', 'sh', 'zsh', 'dash', 'ksh'])

export const REMOTE: ReadonlySet<string> = new Set(['ssh', 'scp', 'sftp', 'rsync', 'mosh', 'telnet', 'ssh-copy-id'])

const RUNTIMES: ReadonlySet<string> = new Set(['python', 'python3', 'python2', 'node', 'deno', 'perl', 'ruby'])

const CODE_RUNNERS: ReadonlySet<string> = new Set(['source', '.', 'eval'])

const AWK: ReadonlySet<string> = new Set(['awk', 'gawk', 'mawk'])

const RUNNERS: ReadonlySet<string> = new Set(['uv', 'uvx', 'npx', 'bunx', 'poetry', 'pipenv', 'pdm', 'rye'])

const OUR_TREE = ['/dotfiles/', '/.agents/', '/.claude/']

const READERS: ReadonlySet<string> = new Set([
  'trace', 'agent-browser',
  'rg', 'grep', 'egrep', 'fgrep', 'ag', 'ack', 'find', 'fd', 'ls', 'tree',
  'cat', 'bat', 'head', 'tail', 'wc', 'nl', 'file', 'stat', 'du', 'df', 'diff', 'comm', 'strings', 'od', 'xxd',
  'sort', 'uniq', 'cut', 'tr', 'column', 'jq', 'yq', 'sed', 'awk', 'basename', 'dirname', 'realpath', 'readlink', 'pwd', 'echo', 'printf',
  'date', 'which', 'type', 'uname', 'whoami', 'hostname', 'id', 'env', 'git', 'curl',
  'ps', 'lsof', 'uptime', 'sw_vers', 'sysctl',
])

const VALIDATORS: ReadonlySet<string> = new Set(['pytest', 'phpunit', 'pest', 'playwright', 'vitest', 'jest', 'mocha', 'ruff', 'mypy', 'eslint', 'tsc', 'shellcheck', 'stylelint', 'phpstan'])

const SCRIPT_RUNNERS: ReadonlySet<string> = new Set(['npm', 'pnpm', 'yarn', 'bun'])

const CHECK_SCRIPTS: ReadonlySet<string> = new Set(['test', 'tests', 'lint', 'typecheck', 'check', 'format:check'])

const WRITE_FLAGS: Readonly<Record<string, readonly string[]>> = { sed: ['-i', '--in-place'], curl: ['-o', '-O', '--output', '--remote-name', '--create-dirs'] }

const GIT_READERS: ReadonlySet<string> = new Set(['status', 'log', 'diff', 'show', 'blame', 'annotate', 'shortlog', 'reflog', 'rev-parse', 'rev-list', 'ls-files', 'ls-tree', 'cat-file', 'describe', 'merge-base', 'name-rev', 'whatchanged', 'grep', 'count-objects', 'version'])

type Options = { readonly shortValued: ReadonlySet<string>; readonly shortBare: ReadonlySet<string>; readonly longValued: ReadonlySet<string>; readonly longBare: ReadonlySet<string> }

const PREFIXES: ReadonlyMap<string, Options> = new Map([
  ['env', options('C:P:u:i0v --chdir= --unset= --ignore-environment --null --debug')],
  ['sudo', options('C:D:g:h:p:R:r:T:t:U:u:ABbEeHiKklNnPSsVv --close-from= --chdir= --group= --host= --prompt= --chroot= --role= --command-timeout= --type= --other-user= --user= --askpass --bell --background --preserve-env --edit --set-home --login --remove-timestamp --reset-timestamp --list --no-update --non-interactive --preserve-groups --stdin --shell --version --validate')],
  ['doas', options('a:C:u:Lns')],
  ['command', options('pvV')],
  ['exec', options('a:cl')],
  ['nohup', options('')],
  ['time', options('f:o:ahlpqv --format= --output= --append --portability --verbose --quiet')],
  ['timeout', options('k:s:fpv --kill-after= --signal= --foreground --preserve-status --verbose')],
  ['nice', options('n: --adjustment=')],
  ['ionice', options('c:n:p:P:u:t --class= --classdata= --pid= --pgid= --uid= --ignore')],
  ['stdbuf', options('i:o:e: --input= --output= --error=')],
  ['xargs', options('a:d:E:I:J:L:n:P:R:S:s:0oprtx --arg-file= --delimiter= --max-args= --max-procs= --max-chars= --max-lines= --process-slot-var= --null --interactive --no-run-if-empty --verbose --exit --open-tty')],
  ...['do', 'then', 'else', 'elif', 'if', 'while', 'until'].map((keyword): [string, Options] => [keyword, options('')]),
])

export function tokens(line: string): readonly Token[] | undefined {
  const found: Token[] = []
  let word: string | undefined
  let operator = ''
  const endWord = (): void => {
    if (word !== undefined) found.push({ text: word, isOperator: false })
    word = undefined
  }
  const endOperator = (): void => {
    if (operator !== '') found.push({ text: operator, isOperator: true })
    operator = ''
  }
  for (let index = 0; index < line.length; index += 1) {
    const char = line.charAt(index)
    if (OPERATOR_CHARACTERS.includes(char)) {
      endWord()
      operator += char
      continue
    }
    endOperator()
    if (char === ' ' || char === '\t' || char === '\r') endWord()
    else if (char === '#' && word === undefined) {
      while (index + 1 < line.length && line.charAt(index + 1) !== '\n') index += 1
    } else if (char === '\\') {
      if (line.charAt(index + 1) !== '\n') word = (word ?? '') + line.charAt(index + 1)
      index += 1
    } else if (char === "'") {
      const close = line.indexOf("'", index + 1)
      if (close < 0) return undefined
      word = (word ?? '') + line.slice(index + 1, close)
      index = close
    } else if (char === '"') {
      let text = ''
      let at = index + 1
      for (; at < line.length && line.charAt(at) !== '"'; at += 1) {
        if (line.charAt(at) === '\\' && '"\\'.includes(line.charAt(at + 1))) at += 1
        text += line.charAt(at)
      }
      if (at >= line.length) return undefined
      word = (word ?? '') + text
      index = at
    } else word = (word ?? '') + char
  }
  endWord()
  endOperator()
  return found
}

export function isSeparator(text: string): boolean {
  return text !== '' && [...text].every((char) => SEPARATOR_CHARACTERS.includes(char))
}

export function segments(line: string): readonly Segment[] | undefined {
  const found = tokens(line)
  if (found === undefined) return undefined
  const result: Segment[] = []
  let words: string[] = []
  let piped = false
  for (const token of found) {
    if (token.isOperator && isSeparator(token.text)) {
      if (words.length > 0) result.push({ words, piped })
      words = []
      piped = token.text.includes('|')
    } else words.push(token.text)
  }
  if (words.length > 0) result.push({ words, piped })
  return result
}

export function localLines(context: Context): readonly string[] {
  const remote = context.commands.filter(({ program }) => REMOTE.has(program)).map(({ args }) => args.join(' '))
  const scripts = context.commands.flatMap((command) => {
    const script = shellScript(command)
    return script === undefined || remote.some((line) => line.includes(script)) ? [] : [script]
  })
  return [context.text, ...scripts].map((line) => line.trim())
}

export function shellScript({ program, args }: Command): string | undefined {
  if (!SHELLS.has(program)) return undefined
  const flag = args.findIndex((arg) => /^-[a-z]*c[a-z]*$/.test(arg))
  return flag < 0 ? undefined : args.slice(flag + 1).find((arg) => !arg.startsWith('-'))
}

export function headAndArgs(words: readonly string[]): readonly [string, readonly string[]] {
  let index = pastAssignments(words, 0)
  for (let prefix = PREFIXES.get(nameOf(words[index] ?? '')); prefix !== undefined && index < words.length; prefix = PREFIXES.get(nameOf(words[index] ?? ''))) index = pastPrefixArgs(prefix, words, index + 1)
  return index < words.length ? [nameOf(words[index] ?? ''), words.slice(index + 1)] : ['', []]
}

export function nameOf(word: string): string {
  return basename(word.replace(/^["']+|["']+$/g, ''))
}

export function readonlyRefusal(head: string, args: readonly string[]): string {
  if (REMOTE.has(head) || VALIDATORS.has(head)) return ''
  if (RUNNERS.has(head)) {
    let inner = args.find((arg) => !arg.startsWith('-')) ?? ''
    if (inner === 'run') inner = args.slice(args.indexOf('run') + 1).find((arg) => !arg.startsWith('-')) ?? ''
    if (VALIDATORS.has(nameOf(inner))) return ''
    return inner === '' ? `\`${head}\` needs a checking command here.` : `\`${head} ${inner}\` runs whatever it is handed.`
  }
  if (SCRIPT_RUNNERS.has(head)) {
    const positional = args.filter((arg) => !arg.startsWith('-'))
    const script = positional[0] === 'run' && positional.length > 1 ? positional[1] ?? '' : positional[0] ?? ''
    if (CHECK_SCRIPTS.has(script)) return ''
    return script === '' ? `\`${head}\` needs a checking script here.` : `\`${head} ${script}\` is not a checking script.`
  }
  if (!READERS.has(head)) return `\`${head}\` is not one of the commands a read-only agent runs.`
  const flag = (WRITE_FLAGS[head] ?? []).find((each) => args.some((arg) => arg === each || arg.startsWith(`${each}=`)))
  if (flag !== undefined) return `\`${head} ${flag}\` writes.`
  if (head === 'git') {
    const subcommand = args.find((arg) => !arg.startsWith('-')) ?? ''
    if (!GIT_READERS.has(subcommand)) return subcommand === '' ? '`git` needs a reporting subcommand here.' : `\`git ${subcommand}\` changes the repository.`
  }
  return ''
}

export function unreadable(context: Context): string {
  if (context.commands.length === 0 && context.shell?.isFullyParsed === false) return 'The command cannot be parsed.'
  const cwd = context.call?.cwd ?? context.root
  for (const command of context.commands) {
    if (/^\$[A-Za-z_{]/.test(command.program) || command.program.includes('`')) return `\`${command.program}\` names a command that is not in the line.`
    if (command.program === 'alias') return '`alias` gives a command a name that is not in the line.'
    if (AWK.has(command.program)) {
      const program = command.args.find((arg) => !arg.startsWith('-')) ?? ''
      if (program.includes('system(') || program.includes('| "')) return `\`${command.program}\` reaches the shell from inside its own program.`
      continue
    }
    for (const [head, args] of [[command.program, command.args] as const, ...runnerTarget(command)]) {
      const hidden = hiddenProgram(head, args, command.input !== undefined, resolve(cwd, command.folder.replace(/^~(?=\/|$)/, homedir())))
      if (hidden !== '') return hidden
    }
  }
  return ''
}

export function isInTmp(path: string): boolean {
  return `${realPath(path)}/`.startsWith(`${realPath('/tmp')}/`)
}

export function absolutePath(path: string, folder: string): string {
  const expanded = path.replace(/^["']+|["']+$/g, '').replace(/^~(?=\/|$)/, homedir())
  return normalize(isAbsolute(expanded) ? expanded : join(folder, expanded))
}

function hiddenProgram(head: string, args: readonly string[], isPiped: boolean, folder: string): string {
  if (SHELLS.has(head) && args.some((arg) => /^-[a-z]*c[a-z]*$/.test(arg))) return ''
  if (!SHELLS.has(head) && !RUNTIMES.has(head) && !CODE_RUNNERS.has(head)) return ''
  const flag = args.find((arg) => arg === '--command' || arg === '--eval' || (arg.startsWith('-') && !arg.startsWith('--') && /[cem]/.test(arg.slice(1))))
  if (flag !== undefined) return `\`${head} ${flag}\` runs code that is not in this command line.`
  const script = args.find((arg) => !arg.startsWith('-'))
  if (script === undefined) return isPiped ? `\`${head}\` runs whatever is piped into it.` : ''
  const path = realPath(absolutePath(script, folder))
  return isFile(path) && OUR_TREE.some((fragment) => `${path}/`.includes(fragment)) ? '' : `\`${head} ${script}\` runs a program this check cannot read.`
}

function runnerTarget({ program, args }: Command): readonly (readonly [string, readonly string[]])[] {
  if (!RUNNERS.has(program)) return []
  let rest = args.filter((arg) => !ASSIGNMENT.test(arg))
  if (rest[0] === 'run') rest = rest.slice(1)
  while (rest[0]?.startsWith('-') === true) rest = rest.slice(1)
  return rest[0] === undefined ? [] : [[nameOf(rest[0]), rest.slice(1)]]
}

function options(spec: string): Options {
  const [short = '', ...long] = spec.split(/\s+/).filter((part) => part !== '')
  const valued = new Set([...short].filter((char, index) => short.charAt(index + 1) === ':'))
  return {
    shortValued: valued,
    shortBare: new Set([...short.replaceAll(':', '')].filter((char) => !valued.has(char))),
    longValued: new Set(long.filter((name) => name.endsWith('=')).map((name) => name.slice(0, -1))),
    longBare: new Set(long.filter((name) => !name.endsWith('='))),
  }
}

function pastAssignments(words: readonly string[], start: number): number {
  let index = start
  while (index < words.length && ASSIGNMENT.test(words[index] ?? '')) index += 1
  return index
}

function pastPrefixArgs(prefix: Options, words: readonly string[], start: number): number {
  let index = start
  while (index < words.length) {
    const word = words[index] ?? ''
    if (ASSIGNMENT.test(word) || NUMERIC.test(word) || isSeparator(word)) index += 1
    else if (word.startsWith('-')) index += flagSpan(prefix, word) || 1
    else break
  }
  return index
}

function flagSpan(prefix: Options, word: string): number {
  if (word === '--') return 1
  if (word.startsWith('--')) {
    const equals = word.indexOf('=')
    const name = equals < 0 ? word : word.slice(0, equals)
    if (prefix.longValued.has(name)) return equals < 0 ? 2 : 1
    return prefix.longBare.has(name) && equals < 0 ? 1 : 0
  }
  for (let at = 1; at < word.length; at += 1) {
    const letter = word.charAt(at)
    if (prefix.shortValued.has(letter)) return at + 1 < word.length ? 1 : 2
    if (!prefix.shortBare.has(letter)) return 0
  }
  return 1
}

function realPath(path: string): string {
  for (let existing = path; ; existing = dirname(existing)) {
    try {
      return join(realpathSync(existing), relative(existing, path))
    } catch {
      if (dirname(existing) === existing) return path
    }
  }
}

function isFile(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isFile() === true
}
