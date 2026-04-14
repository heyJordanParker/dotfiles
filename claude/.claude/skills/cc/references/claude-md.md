
# Claude.md Files

Guide for creating and maintaining Claude.md files — the hierarchical documentation system that gives agents context about a project, module, or directory.

Document how agents should think about the system — business goals, user intent, strategic direction, and architectural reasoning that code alone can't express.

## Template

Every Claude.md file follows this structure. `#` is reserved for the scope name. All sections use `##`.

```markdown
# [Scope Name]
v1.0 | Updated: 2026-01-01

## Why

A brief explanation of why this project or module exists — the business
problem it solves, the motivation behind it, or the pain it addresses.
Not what it does or how — just why it matters.

## What

A sentence framing what this scope is responsible for at a high level.

### Requirements

- Must [do something specific]
- Always [enforce something]

### Boundaries

- Never [do something specific]
- Never [cross some line]

## Architecture

Brief sentence describing the system shape, then annotated trees.

app/
├── Models/          # unified domain models
├── Services/        # cross-cutting operations
└── Http/
    ├── Controllers/ # REST API endpoints
    └── Middleware/   # access control

Schema overviews where relevant:

public:
├── tenants              # tenant registry
├── accounts             # admin auth source of truth
└── migrations           # schema versioning

tenant_{id}:
├── wp_*                 # all WordPress tables
└── (business tables)    # products, orders, enrollments

## Workflow

How to work within this scope day-to-day. Subsections vary — use
whatever groupings make the content scannable.

Examples at root level: Commands, Setup, URLs.
Examples at module level: Testing, Migration.

## How

Recurring process patterns specific to this scope (e.g. "how to add
a migration", "how to register a new entity"). Not one-time code
changes — those are visible in git.
Subsections vary by module (e.g. Conventions, Patterns, Naming).

## Ledger

Every ledger entry uses the file's current version number as its key.
Adding a ledger entry requires bumping the version; bumping the version
requires a ledger entry. They enforce each other.

Amend while iterating so the ledger matches what git shows.

- v1.0: Chose Postmark over SendGrid — pure env config, 12-factor fit

## References

- [app/Claude.md](app/Claude.md) — backend architecture
- [admin/Claude.md](admin/Claude.md) — frontend architecture
```

### Section Rules

**Required sections:**

**Why** — min 1 sentence. Business context, user intent, domain knowledge.
- "Funnel SaaS for creators — solo developer, minimize maintenance, maximize 3rd-party reuse"
- "Plan to fully remove FunnelKit — own our funnel data"

**What** — min 1 sentence, with Requirements and/or Boundaries.

Every requirement and boundary must correct default Claude Code behavior — not restate it. If Claude Code would already do it without being told, it doesn't belong. This is the same litmus test from the /cc principles applied specifically to documentation.

Requirements prevent plausible future mistakes. If the code makes it obvious, skip it.
- Good: "Account is source of truth for admin auth — WP users created on demand" — agent can't derive this from code
- Bad: "Move billing from UserController to BillingService" — that's the commit
- Bad: "No coupon CRUD in admin" — no agent builds unplanned UI unprompted. This is default behavior, not a correction
- Bad: "Always read files before editing" — Claude Code already does this
- Bad: "Use descriptive variable names" — default behavior, not a correction

Boundaries define encapsulation. Use domain language, not library names.
- Good: "Domain models never import plugin code — service providers own integrations"
- Bad: "Models never import FunnelKit" — names the library; generic rule is durable

**Ledger** — min 1 versioned entry. Every entry is keyed by the file's version number. Adding a ledger entry requires bumping the version; bumping the version requires a ledger entry. They enforce each other. Amend while iterating so the ledger matches what git shows.
- Read `git diff --stat` before writing — scope the entry to the actual commit, not the task
- WHY comes from the architect (plan, shaping doc, conversation) — not from patterns inferred while coding
- Good: "v1.1: Chose Postmark over SendGrid — pure env config, 12-factor fit"
- Bad: "Added getCustomer() to Account" — that's the diff
- Bad: A→B then B→C entries when the commit only shows A→C
- Bad: Re-explaining architecture the file already documents — the ledger records decisions, not descriptions
- Bad: Fabricating WHY — "to scale independently of X" when you don't know the motivation. Ask if unknown
- Bad: Splitting one decision into multiple entries — "Added multi-tenancy" + "Added dual-boot" when they're one decision
- Bad: Writing what changed instead of why it was decided — that's a changelog, not a ledger
- Bad: Long entries with technical details — ledger entries are one line. Details belong in the file body or a deeper Claude.md
- Bad: Including execution context — "(importance 8)", "(impact 6)" are meaningless outside the session that produced them
- Bad: Implementation HOW — "extract helper so callers share validation" describes code mechanics. Ledger names decisions and business motivation, not how the code achieves them
- Bad: Iteration debris — entries left from intermediate steps that were revised or reverted. Amend during iteration; the committed ledger reflects only final state
- Bad: Unverified claims — stating outcomes without reading the code to confirm what actually changed
- Bad: Overloaded terms — using words that conflict with codebase concepts (e.g. "schema" where `#[Schema]` is an attribute)

**Optional sections** (include when they add value):
- **Architecture** — annotated file trees and schema overviews
- **Workflow** — commands, procedures, collaboration norms
- **How** — recurring process patterns. Not one-time code changes
- **References** — links to related Claude.md files

### Language

- Requirements use **must** and **always**
- Boundaries use **never**
- No hedging — "Use X" not "consider using X"

### Scope Name

The `#` heading names the current scope — an expanded, human-readable version of the directory name:
- Root: `# Creator Income Blueprint`
- Backend: `# Backend`
- Models: `# Domain Models`
- Admin frontend: `# Admin Frontend`

## Triggers

1. **Explicit request** - "update docs", "save this to Claude.md", etc.
2. **Pre-commit prompt** - Before any commit, use AskUserQuestion: "Should we update docs for these changes?"
3. **Opportunistic flags** - When reading a Claude.md, use AskUserQuestion to flag:
   - Contradictions with actual code (staleness)
   - Files covering too many concerns (needs splitting into deeper hierarchy)
   - Missing Why or What sections (template compliance)
4. **Pattern detection** - Offer to document when noticing:
   - Repeated explanations across sessions
   - Emerging conventions that should be codified
   - User corrections to assumptions or behavior
5. **Missing hierarchy** - When working in a directory without a Claude.md but with established patterns, proactively offer to create one

## Proactive Offers (impact 9-10 decisions)

- Architectural changes
- New patterns introduced
- Breaking changes to existing conventions

## Update Process

**Prerequisites:**
- Target Claude.md file path
- Topic to document (one concept only)
- User's instruction or context

**Step 1: Research (Subagent)**

Dispatch subagent to read documentation hierarchy:
- Target Claude.md file
- All Claude.md files in parent directories
- All Claude.md files in child directories

Subagent returns summary:
- File structure (sections and what they cover)
- Existing documentation on this topic (quotes or "none found")
- Conflicts with proposed content (quote both sides)
- Recommended placement with reasoning

Main agent reads the full target Claude.md before proposing any edits. Claude.md files are holistic documents — piecemeal edits without full context cause contradictions and repetition.

**Step 2: Handle Conflicts**

If existing content found, present options:
- A) Replace existing with new version
- B) Merge both (draft combined version)
- C) Add to different section
- D) Cancel - existing is sufficient

Wait for user choice.

**Step 3: Propose Diff**

```
File: [path]
Section: [section name]

Remove:
> [exact text being removed, if any]

Add:
> [exact text being added]

Reasoning: [why this placement, how it fits]
```

Rules:
- One concept per change
- Match existing voice/style
- Minimal additions - no drive-by improvements
- Self-review before proposing - minimal, elegant, direct

**Step 4: Execute**

After explicit approval:
- Apply exact change from approved diff
- Bump version and update date on the version line — every ledger entry requires a version bump, every version bump requires a ledger entry

## Hierarchy & Placement

**How Claude.md Files Work:**

Claude.md files are hierarchical. When opening any file, Claude automatically reads every Claude.md in the file tree from root to that file's directory. This means:
- Parent context flows down automatically
- Don't repeat parent content in child files
- Each file only needs to add what's specific to its level

**Core Principles:**

- **Push context as deep as possible while keeping it discoverable.** Root stays navigable, details live where they're needed.
- **Write minimum documentation that provides full context.** Irrelevant context doesn't just waste space — it dilutes the signal and biases agent output away from what matters. Too little → agents can't complete tasks. Too much → agents drift toward wrong priorities. Every line must earn its place.
- **Never duplicate parent content in child files.** Claude reads every Claude.md from root to the working directory automatically. Duplication biases the model — repeated content gets treated as higher priority — and creates staleness risk.
- **Never put scope-specific content in parent files.** A frontend agent doesn't need backend details. If content only applies to one subdirectory, it belongs there — not in the parent.

**What Goes Where:**

- **Project root:** Why, universal requirements/boundaries that apply to every subdirectory, architecture overview, workflow commands. If a requirement only applies to backend or frontend, it belongs there — not root
- **Feature/module/namespace directories:** Module-specific why, requirements, boundaries, patterns
- **Code-heavy directories:** Tactical docs with code examples, function signatures, usage patterns
- **Organizational directories:** Structural docs explaining what's inside and how subdirectories relate

**The deciding factor is what's in the directory:**
- Mostly code files → include concrete examples and API patterns
- Mostly subdirectories → explain the structure and relationships

**Each Claude.md answers:** "What do I need to know to work effectively in this directory?"

**Signals to Split:**

- A section grows beyond ~10 bullet points
- Content primarily serves a subdirectory's developers
- Adding implementation details to a strategic document

**Signals to Create New Claude.md:**

- Working in a directory with established patterns but no Claude.md
- A parent file is covering concerns that belong deeper
- Developers in that directory would benefit from local context

**Placement Decision Process:**

1. Start at the deepest directory the content applies to
2. Only escalate to parent if the content genuinely applies to ALL children of that parent
3. If no Claude.md exists at the right level, create one
4. Litmus test: would a developer working in a sibling directory need this? No → it stays deep. Yes → consider the parent
5. Propose placement with reasoning, wait for approval

## Style Guide

**File Metadata:**
- File name must be `Claude.md` (PascalCase, not ALL_CAPS)
- Version line under `#` heading: `v1.2 | Updated: 2026-01-01` — bump major for structural changes, minor for content additions. Every version bump requires a ledger entry; every ledger entry requires a version bump

**Structure:**
- 2-space indentation
- Bullet points under descriptive headings
- `#` for scope name, `##` for main sections, `###` for subsections
- No orphan content (everything under a heading)
- Keep sections under ~10 bullet points (split if larger)

**Voice:**
- Imperative ("Use X" not "You should use X")
- No filler words or preamble
- No hedging ("consider", "might", "should", "could") - use direct commands ("Use", "Run", "Do")
- Examples illustrate, not prescribe - describe the capability, then give examples with "e.g." or "such as": "prettify html code (e.g. buttons, links)" not "prettify buttons and links"
- No fluff - every line earns its place

**Formatting:**
- **Bold** for key terms being defined
- `Backticks` for code, file paths, commands
- Numbered lists only for procedural steps
- Bullet points for everything else

**What to Exclude:**
- Session-specific context
- Temporary decisions
- Anything that needs frequent updates
- Content that belongs in a deeper file

**Writing Docs AI Will Read:**
- **Explicit over implicit** - State rules directly; AI won't infer from examples
- **Firm, specific, declarative** - "Use X for Y" beats "potentially consider X"
- **Front-load critical info** - First lines of sections get highest weight
- **Structured data** - Markdown with bullets/annotated file trees — scannable and dense. Avoid tables & gaudy ASCII displays that add noise without information.

## References

- `.claude/rules/` - Alternative to Claude.md; files auto-loaded
- [Claude Code Changelog](https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md) - Check for new features affecting documentation
