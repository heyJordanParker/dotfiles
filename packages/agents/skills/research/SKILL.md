---
name: research
description: Source-grounded external research for current library, API, framework, service, and vendor behavior. TRIGGER when the Task asks for current facts, docs, releases, compatibility, adoption, or external technical Decisions. DO NOT TRIGGER for in-codebase Architecture tracing; use /trace or the explorer agent.
---

# Research

- Web search is a lead source, not a truth source; search summaries and blogs can lag reality by months.
- Current means grounded to a visible date, version, tag, release, or commit.

## 1. Anchor the question

State the external system, the claim being tested, the version or date the answer depends on, and the local codebase detail that matters.

### Keep in-repo trace scoped to the local anchor
Use `trace` only to learn how the current codebase uses the external system: installed version, enabled option, import path, call site, or runtime constraint.
Never: use `trace` as the source for the external library's current behavior.

## 2. Use the source-of-truth ladder

Rank sources by distance from ground truth:

1. A cloned or updated upstream repository in `~/Developer/references`: actual source code, git history, CHANGELOG, tags, and release artifacts.
2. GitHub directly: latest release, recent commits, open issues, and pull requests.
3. Official docs with a visible version, release line, or publication/update date.
4. Web search results, blogs, AI summaries, and community posts: leads and pointers only.

### Never state current behavior from the bottom rung
A web-search summary, blog post, or community answer can suggest where to look, but it does not support a current claim.
Never: "Search says the latest version does X" without a repo commit, tag, release, or dated official doc behind it.

## 3. Ground current claims in the upstream repo

Clone the source repository into `~/Developer/references`, or update the existing clone, before trusting search results about current behavior.

Template:
  ```bash
  mkdir -p ~/Developer/references
  git -C ~/Developer/references/<repo> fetch --all --tags --prune
  git -C ~/Developer/references/<repo> log -5 --oneline --decorate
  git -C ~/Developer/references/<repo> tag --sort=-creatordate | head
  ```

IF the repository is not cloned:
### Clone the upstream source before answering current behavior
Clone it under `~/Developer/references`, then inspect the source, history, CHANGELOG, tags, and releases for the version in question.

IF the upstream repository cannot be cloned or updated:
### Label repo-grounding as blocked
Use GitHub or dated official docs as the next rung and explicitly label any unverified current claim.

## 4. Verify the exact version, date, or commit

Read the code and history for the version in question before trusting a secondary source.

### Attach a ground marker to every current claim
Every current claim carries one of: version, release tag, commit hash, release date, documentation date, or explicit `unverified` label.
Example: `As of tag v1.2.3 (2026-06-14), the option is read in src/config.ts.`
Never: `Currently the library supports this` with no version, date, or commit.

### Treat contradictions as findings
When source, docs, issues, and search results disagree, report the contradiction and the grounding behind each side. Do not hide the mismatch to make the answer cleaner.

### Keep unverified claims labeled
If a claim cannot be tied to a repo commit, tag, release, or dated official doc, call it unverified and do not use it as the basis for a Decision.

## 5. Qualify adoption claims

Adoption claims need usage signals, not vibes.

### Include the signal and its limits
Use signals such as stars, download counts, dependent counts, recent release activity, issue/PR activity, maintainer activity, production-user references, or ecosystem mentions. Name the date observed and what the signal cannot prove.
Never: make a niche experiment look established because a blog post is enthusiastic.

## 6. Report findings for the Decision

Return the answer in the smallest shape that lets the Architect decide: answer, grounded Evidence, contradictions, unverified claims, adoption signal when relevant, and recommendation.

Template:
  ```markdown
  Answer: <direct answer>

  Grounding:
  - <claim> — <repo/tag/commit/date or dated official doc>

  Contradictions:
  - <source A says X; source B says Y; which one is closer to ground truth>

  Unverified:
  - <claim that remains unverified, or "None">

  Adoption signal:
  - <signal, date observed, and limitation>

  Recommendation:
  - <Decision-oriented recommendation, with confidence>
  ```

## 7. Verify before final answer

### Check that no current claim rests on search alone
Before answering, confirm every current claim traces to a repo commit, tag, release, or dated official doc, or is labeled unverified.

### Check that the source ladder was respected
Confirm the answer did not treat web search, blogs, AI summaries, or community posts as ground truth for current behavior.
