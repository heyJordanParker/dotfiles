# Updating the /cc Skill

The Process for syncing /cc with a Claude Code release that changes how Agents write, load, test, or distribute Prompts.

## 1. Decide whether the release belongs in /cc

- Claude Code release notes live at `https://github.com/anthropics/claude-code/releases/tag/v{VERSION}`.
- The full Claude Code changelog lives at `https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md`.
- /cc covers Claude Code behavior that Agents writing Prompts can act on: Hook events, Commands, command-line flags, Skill frontmatter, Agent frontmatter, plugin features, settings.json options, deprecations, removals, and bug fixes that reveal undocumented behavior.
- /cc skips internal performance work, enterprise administrator only settings, and interface polish that Agents writing Prompts cannot act on.

IF a Claude Code release has no changes that Agents writing Prompts can act on:
### Leave /cc unchanged
A release with no Prompt-writing consequence does not create a gap in the /cc Process.
Example: skip a release note that only says startup got faster.
Never: bump the "Last synced" line for a release /cc did not absorb.

## 2. Read current coverage before editing

### Read every /cc Reference before changing SKILL.md
The existing Reference usually already owns the new behavior. Reading all of `references/` first prevents duplicates and keeps the SKILL.md topics list honest.
Example: read `references/plugins-marketplace.md` before adding a plugin marketplace change.
Never: patch the SKILL.md topics list from release-note words alone.

### Compare release notes against References line by line
A change is missing only after the current owner was checked.
Example: compare a new Hook event against `references/automating-with-hooks.md`, then against SKILL.md only if the step itself changed.
Never: infer a gap from a keyword match without reading the owning Reference.

## 3. Put each change in the owning Prompt

- SKILL.md owns the /cc trigger, the one Process, ordered steps, and the References list.
- A Reference owns one Process split out for Progressive Disclosure.
- Existing References own their named problems: Skills, Examples, Skill testing, Claude.md, Hooks, Agents, plugin distribution, Commands, and /cc syncing.

### Update the existing Reference before creating a new Reference
A new Reference is only for a genuinely new recurring Process that no existing Reference owns.
Template:
  Claude Code change: <observed release-note change>
  Owning Prompt: <SKILL.md or references/name.md>
  WHY it belongs there: <one sentence tied to the Prompt type allowance>
  Edit: <the exact Fact, Rule, Example, Template, or Process step to change>
Example: place a marketplace `renames` map in plugin distribution.
  Claude Code change: marketplace `renames` map
  Owning Prompt: `references/plugins-marketplace.md`
  WHY it belongs there: plugin distribution owns marketplace behavior
  Edit: add a version-management Fact under plugin state
Never: duplicate the same plugin change in SKILL.md and `references/plugins-marketplace.md`.

IF no existing Reference owns the Claude Code change:
### Propose the new Reference before creating it
A new Reference adds a new Prompt file, so the Architect needs the problem it solves before it exists.
Example: propose `references/<problem>.md` with the one Process it would own.
Never: create a Reference because the current file is getting long.

## 4. Apply the Prompt block law

### Write each changed line in the block shape from SKILL.md
Facts are plain sentences, Rules are `###` headings with explanations, Conditions are one IF line owning one Rule, Examples and Nevers are labeled lines, and Templates are indented blocks under `Template:`.
Template:
  ### <Rule title>
  <explanation>
  Example: <correct behavior>
  Never: <wrong behavior>
Example: `### Bump version for every distributed change` owns the plugin version Rule, with an Example and a Never directly under it.
Never: `**Rule:** bump version` or a Condition that owns multiple Rules.

### Preserve measured findings exactly where they already earn their line
A measured finding is Evidence behind a Prompt correction; moving it or paraphrasing it can weaken the correction.
Example: keep the codex communication score finding in SKILL.md step 3 while changing surrounding prose.
Never: cut a measured score because it is old.

## 5. Run the gap pass

### Re-read the release against the updated Prompts
The second pass catches changes missed while editing and proves every release-note item has either a home or a deliberate skip.
Template:
  Release item: <release-note item>
  Home: <Prompt path or skipped>
  Reason: <why this home owns it, or why /cc skips it>
  Verification: <line read, command run, or file checked>
Example: track `defaultEnabled: false` in plugin distribution.
  Release item: `defaultEnabled: false` for plugins
  Home: `references/plugins-marketplace.md`
  Reason: plugin distribution owns plugin enablement
  Verification: version-management section contains the Fact
Never: finish after the first edit pass without re-reading the release notes.

## 6. Update the Last synced line and verify

### Update the Last synced line last
Only bump the line after every covered change has a home, every skipped change is deliberate, and the audit passes.
Template:
  `- Last synced with Claude Code **v{VERSION}** ({DATE}).`
Example: `- Last synced with Claude Code **v2.1.195** (2026-06-28).`
Never: bump the line before the gap pass.

### Verify every edited Prompt against the /cc audit
Every edited Prompt must pass the file-type allowance, the block notation, and the line audit before the /cc sync is done.
Example: one H1 maximum, flat headings, `###` Rules with explanations, one-line Conditions, `Example:`/`Never:`/`Template:` labels, plain-sentence Facts, Domain.md capitalization, no coined terms, no duplicate content, and Fluff cut.
Never: call the release synced because the version line changed.
