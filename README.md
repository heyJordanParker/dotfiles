# Talent Tree

Overpowered Claude Code talents to power level your software architecture.

## Install Talents

Requires [Claude Code](https://docs.anthropic.com/en/docs/claude-code).

```bash
/plugin marketplace add heyJordanParker/dotfiles
/plugin install talents@talent-tree
```

### Talents

Invoked as `/talents:<name>`:

- **architecture**: present architectural options with their tradeoffs
- **build**: build mode, where the agent does every edit and run itself, with no subagents
- **cc**: write and fix Claude Code skills, agents, commands, hooks, rules, and Claude.md
- **commit**: commit your own changes with a type prefix, a Problem and Solution body, and a file tree
- **critical-path**: measure the blast radius of a risky change and verify what depends on it
- **debate**: independent architect agents debate architectural options through rounds
- **debug**: systematic debugging that traces the root cause before any fix
- **delegate**: dispatch one-shot subagents with a complete brief and an Evidence folder
- **design**: UI components, styling, CSS architecture, and interaction decisions
- **discover**: find every option that solves a problem before proposing one
- **domain-design**: build a project's Domain.md and record its Decisions
- **gh-issue**: create GitHub issues that carry the problem, the WHY, and the systems it crosses
- **handoff**: compact the conversation into a handoff document for another agent
- **independent-review**: identical parallel agents on one task, for consensus through redundancy
- **merge**: merge two branches and prove every behaviour from both sides still runs
- **modeling**: turn a shaped change into a database schema, UX flows, and architecture
- **naming**: name a code identifier, or review the names code already has
- **orchestrate**: orchestrate mode, where subagents make every change and the agent validates them
- **plan-tests**: list the tests a change needs before any code is written
- **pragmatic-engineering**: keep engineering boring and complete
- **present**: present proposals and findings so the reader can act on them
- **progressive-research**: broad-to-narrow research that specializes subagents once the situation is proven
- **propose**: the contract for proposing turns, with researched Solutions and their code, ordered by dependency
- **prove**: produce the Evidence that work is done before claiming it is
- **rank**: score a set of options and order them
- **regressions**: map a diff to the Capabilities it affects and report any it breaks
- **research**: source-grounded research of current library, API, framework, and vendor behavior
- **review**: review the changeset with parallel reviewers and iterate until nothing breaking remains
- **shaping**: shape a rough idea into requirements, boundaries, and solution options
- **show-me**: draw every code change as a diff, an annotated file tree, or a diagram
- **show-me-html**: publish one page that shows an architecture proposal's start and final states
- **slicing**: break a modeled change into vertical slices with acceptance criteria
- **tdd**: implement an approved change one Capability at a time, test first
- **triage**: turn reviewer findings into one graded report and act on the gate
- **understand**: deep research of the systems behind a change, written up for another agent
- **user-testing**: trace real user flows through code changes and find the gaps
- **verify-changes**: verify the whole changeset the way the user meets it
- **write-test**: write one test that goes red when the app loses a Capability

### Commands

Also invoked as `/talents:<name>`:

- **copy-plan-path**: copy the current plan path to the clipboard
- **copy-shaping-dir**: copy the current shaping directory path to the clipboard
- **copy-transcript-path**: copy the current conversation transcript path to the clipboard
- **retro**: analyze conversation history for patterns and improvements

### Tracer

Code intelligence lives in its own repository, [tracer](https://github.com/heyJordanParker/tracer): the `trace` program and the mod that wires it into Claude Code. Its README covers installing both.

### Agents

The marketplace install ships only skills and commands, never agents. To use the agents in `packages/agents/agents/`, copy the files you want into your own `~/.claude/agents/` or `<repo>/.claude/agents/`:

```bash
git clone https://github.com/heyJordanParker/dotfiles.git /tmp/dotfiles
cp /tmp/dotfiles/packages/agents/agents/explorer.md ~/.claude/agents/
cp /tmp/dotfiles/packages/agents/agents/researcher.md ~/.claude/agents/
# ... and any others you want
```

The split worth knowing:

- **explorer**: in-codebase architectural mapping ("where is X used", "how does Y work end-to-end"), with the trace and understand skills
- **researcher**: external research (library docs, APIs, framework references, web lookups), with the research, agent-browser, cc, and trace skills

Every agent but ux-tester preloads [tracer](https://github.com/heyJordanParker/tracer)'s trace skill. The designer, researcher, tester, and ux-tester agents preload agent-browser, and designer also preloads impeccable. Install the [agent-browser](https://github.com/vercel-labs/agent-browser) and [impeccable](https://github.com/pbakaus/impeccable) plugins before using them.

### Safe Delete

Claude loves running `rm -rf`. Protect yourself by replacing `rm` with [trash](https://github.com/ali-rantakari/trash) so deleted files go to macOS Trash instead of being permanently destroyed:

```bash
brew install trash
```

Then alias `rm` to `trash` in your shell profile:

```bash
cat >> ~/.zshenv << 'EOF'
# Safe delete - moves to Trash instead of permanent deletion
rm() {
  local args=()
  for arg in "$@"; do
    [[ "$arg" =~ ^-[rRfidv]+$ ]] && continue
    args+=("$arg")
  done
  trash "${args[@]}"
}
EOF
```

This silently strips `rm` flags (`-rf`, `-i`, etc.) and sends files to Trash. Claude thinks it's deleting; you can undo from Trash.

---

## Dotfiles

macOS dotfiles managed with [GNU Stow](https://www.gnu.org/software/stow/).

### New Machine Setup

One-line bootstrap (clones to `~/dotfiles`, then runs setup):

```bash
curl -fsSL https://raw.githubusercontent.com/heyJordanParker/dotfiles/master/setup.sh | bash
```

To clone elsewhere, do it yourself first; setup will run from whichever path you pick:

```bash
git clone https://github.com/heyJordanParker/dotfiles.git <path>
cd <path> && ./setup.sh
```

Either way installs Xcode CLI tools, Homebrew, all packages from `Brewfile`, bun, and symlinks configs.

### Manual Usage

Each subdirectory of `packages/` is a stow package. Its contents are exactly what should land at the package's stow target, with no inner mirror wrappers. The package-to-target mapping lives in `scripts/stow.py`.

```bash
# Add a new config — example: 'newapp' targets ~/.config/newapp/
cd <repo>/packages
mkdir -p newapp
mv ~/.config/newapp/* newapp/
stow -t ~/.config/newapp newapp

# Remove symlinks
stow -D -t ~/.config/newapp newapp

# Re-link after changes
stow -R -t ~/.config/newapp newapp
```

### Structure

Stow packages live under `dotfiles/packages/<pkg>/`. Each package contains the actual files that get symlinked to its target. `Claude.md` describes the full architecture.
