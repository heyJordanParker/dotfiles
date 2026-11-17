import { realpathSync } from 'node:fs'
import { homedir } from 'node:os'
import { basename, dirname, isAbsolute, join, normalize, relative } from 'node:path'

export type Token = { readonly text: string; readonly isOperator: boolean }

export type Segment = { readonly words: readonly string[] }

const OPERATOR_CHARACTERS = ';&|()<>\n'

const SEPARATOR_CHARACTERS = ';&|()\n'

export const REMOTE: ReadonlySet<string> = new Set(['ssh', 'scp', 'sftp', 'rsync', 'mosh', 'telnet', 'ssh-copy-id'])

const RUNNERS: ReadonlySet<string> = new Set(['uv', 'uvx', 'npx', 'bunx', 'poetry', 'pipenv', 'pdm', 'rye'])

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

export function segments(line: string): readonly Segment[] | undefined {
  const found = tokens(line)
  if (found === undefined) return undefined
  const result: Segment[] = []
  let words: string[] = []
  for (const token of found) {
    if (token.isOperator && isSeparator(token.text)) {
      if (words.length > 0) result.push({ words })
      words = []
    } else words.push(token.text)
  }
  if (words.length > 0) result.push({ words })
  return result
}

function nameOf(word: string): string {
  return basename(word.replace(/^["']+|["']+$/g, ''))
}

function isSeparator(text: string): boolean {
  return text !== '' && [...text].every((char) => SEPARATOR_CHARACTERS.includes(char))
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

export function isInTmp(path: string): boolean {
  return `${realPath(path)}/`.startsWith(`${realPath('/tmp')}/`)
}

export function absolutePath(path: string, folder: string): string {
  const expanded = path.replace(/^["']+|["']+$/g, '').replace(/^~(?=\/|$)/, homedir())
  return normalize(isAbsolute(expanded) ? expanded : join(folder, expanded))
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
