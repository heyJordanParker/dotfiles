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

- **agent-browser** — Automate browser interactions, web testing, screenshots, data extraction (requires [agent-browser](https://www.npmjs.com/package/agent-browser))
- **architecture** — Present architectural options with tradeoffs
- **cc** — Work with Claude Code skills, hooks, settings
- **code-first** — Autonomous async execution mode for AFK / overnight handoffs
- **codex** — Drive codex CLI runs as your agents
- **commit** — Stage, write the message, commit, verify; structured commits with type prefix and file tree
- **debate** — N independent architects debate architectural options through structured rounds with cross-pollination
- **debug** — Systematic debugging with root cause tracing
- **design** — UI components, styling, and interaction decisions
- **diagram** — Generate Excalidraw diagrams on a live canvas (requires [drawbridge](https://github.com/alexknowshtml/drawbridge))
- **execute** — Contract for executing-state turns: implement approved work, escalate on architectural change
- **execute-plan** — Orchestrate implementation plans with persistent team, validation gates per slice
- **gh-issue** — Create self-contained GitHub issues an agent can implement without conversation history
- **impeccable** — Design, audit, and polish frontend interfaces
- **independent-review** — N identical parallel agents for consensus through redundancy
- **laravel** — Laravel PHP patterns for controllers, models, migrations, queries
- **modeling** — Transform shaped parts into concrete models (DB schema, UX flows, architecture)
- **naming** — Consistent naming for variables, files, classes, everything
- **pcc** — Add pros/cons/confidence to any prompt
- **personas** — 5 parallel persona agents for diverse perspectives
- **pragmatic-engineering** — KISS-driven planning and review
- **propose** — Contract for proposing-state turns: the seven named proposal failures and the choice-block shape
- **react** — React patterns for components, hooks, data fetching, state, performance
- **regressions** — Map a diff to affected flows and report capability regressions
- **review** — Parallel code reviewers on uncommitted changes
- **review-plan** — Review planning artifacts with 5 parallel specialized agents
- **shaping** — Collaborate on problem definition and solution options
- **show-architecture** — Annotated file trees inline
- **slicing** — Break modeled features into vertical implementation slices with acceptance criteria
- **solo** — Single-agent deep-research mode, no subagents
- **subagents** — Framework for dispatching one-shot subagents
- **team** — Framework for creating and coordinating persistent teams
- **trace** — Code intelligence CLI: search, callers, definitions, complexity, file/method reads with rich architectural context (requires the bundled `trace` binary; see Tracer Setup below)
- **user-testing** — Trace real user flows through code changes, find gaps

### Commands

Also invoked as `/talents:<name>`:

- **ask** — Break complex scenarios into decision questions with 4+ options
- **commit** — Validated commit with tests and comprehensive review
- **copy-plan-path** — Copy the current plan path to the clipboard
- **copy-shaping-dir** — Copy the current shaping directory path to the clipboard
- **copy-transcript-path** — Copy the current conversation transcript path to the clipboard
- **plan** — Plan a feature using structured format
- **retro** — Analyze conversation history for patterns and improvements
- **wtf** — Hard reset, redo the last task correctly

### Hooks

The plugin includes hooks that run automatically to keep Claude disciplined:

- **block-git-revert** — Blocks `git reset`, `git restore`, `git checkout -- <file>`. Forces manual execution.
- **block-unsafe-delete** — Whitelists `rm` to specific directories only. Everything else blocked.
- **validate-planning-docs** — Checks planning documents on write/edit for deferral, optionality, and other quality issues.
- **validate-plan-quality** — Gates `ExitPlanMode` on plan quality before a plan is accepted.
- **sync-shaping** — Ripple-check reminders when editing shaping documents


### Tracer Setup

The `/trace` skill needs the `trace` binary. The plugin ships a POSIX launcher that lands on `PATH` automatically when the plugin is enabled. tracer is a native Rust binary, so the launcher resolves a host-appropriate one: a committed prebuilt (macOS arm64 / Linux x86_64), a previously locally-built cached binary, or — on any other platform — `cargo build` from the crate source shipped in the plugin. If no Rust toolchain is present for that last path, `trace` prints an actionable "install Rust" error.

All 23 commands work from whichever binary the launcher resolves — there is no reduced-feature tier. If you want `trace` outside the plugin too, build it from `tools/tracer` and put it on `PATH` (the dotfiles `setup.sh` does this into `~/.local/bin`):

```bash
cd tools/tracer
cargo build --release
install -m 755 target/release/trace ~/.local/bin/trace
```

Tracer wraps five external binaries: `ast-grep`, `scc`, `universal-ctags`, `ripgrep`, `git`. Install whichever your platform needs:

**macOS (Homebrew):**

```bash
brew install ast-grep scc universal-ctags ripgrep
xcode-select --install     # git
```

**Linux (Debian/Ubuntu):**

```bash
apt install universal-ctags ripgrep git
# ast-grep:  https://ast-grep.github.io/guide/quick-start.html
# scc:       https://github.com/boyter/scc#installation
```

**Windows (Scoop):**

```bash
scoop install ast-grep scc ripgrep
# universal-ctags: https://github.com/universal-ctags/ctags
# git:             https://git-scm.com/download/win
```

Verify with `trace doctor` — it lists missing binaries with the same install command for your platform.

### Agents

Per Claude Code plugin schema, agents (`.md` files in `packages/agents/agents/`) are NOT distributed by the marketplace install — only skills, commands, and hooks. To use the bundled agents (explorer, researcher, architect, debugger, etc.), copy the files you want into your own `~/.claude/agents/` or `<repo>/.claude/agents/`:

```bash
git clone https://github.com/heyJordanParker/dotfiles.git /tmp/dotfiles
cp /tmp/dotfiles/packages/agents/agents/explorer.md ~/.claude/agents/
cp /tmp/dotfiles/packages/agents/agents/researcher.md ~/.claude/agents/
# ... and any others you want
```

The split worth knowing:

- **explorer** — in-codebase architectural mapping ("where is X used", "how does Y work end-to-end"). Uses the trace skill heavily.
- **researcher** — external research (library docs, APIs, framework references, web lookups). Uses agent-browser, cc, claude-api, plus trace for incidental in-repo grounding.
- **architect / backend-engineer / code-reviewer / debugger / frontend-engineer / regression-reviewer / tester** — also have the trace skill for navigating the codebase during their work.

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

Each subdirectory of `packages/` is a stow package. Its contents are exactly what should land at the package's stow target — no inner mirror wrappers. The package-to-target mapping lives in `scripts/stow.py`.

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

Stow packages live under `dotfiles/packages/<pkg>/`. Each package contains the actual files that get symlinked to its target — see `Claude.md` for the full architecture.
