---
name: research-product
description: Run the research pass on the product — mine its own artifacts and organize them into the three copy-shaped files a writer uses: the problems it solves, its features with their user-visible outcomes, and the step-by-step processes it runs, boundaries woven per feature, observed app behavior ranked over code intention. TRIGGER when the owner cannot supply the product facts and a plan or piece needs them. DO NOT TRIGGER to study a competitor's product (research-competitor) or to interview the owner for stated facts (setup).
---

# Research Product

One Process: mine the product's own artifacts, then organize what you find into three files shaped for copy — `research/product/problems.md`, `research/product/features.md`, `research/product/processes.md`. This runs only when the owner cannot supply the facts himself; setup captures what the owner states into `product/Brief.md` and `product/Features.md`, and this thread mines what the product IS when those are thin. The output is material a copywriter can lift, never a code tour.

Inputs: the product's own artifacts — nothing else. Excluded: market and competitor records — the product thread runs isolated, so no market guess shapes what gets mined, and its output never reaches buyer discovery, problem mining, or their reviewers; a leaked capability biases every market agent toward solution-shaped problems. The product meets the market landscape only later, at strategy. You carry no positioning, no strategy, and no knowledge of the steps that consume these files — you organize what the product does and stop.

## 1. Mine the artifacts by authority, marking each rung

Work from what exists, in this order of authority: the running app surfaces (screens, flows, settings, empty states) reached through agent-browser (`agent-browser skills get core`), then the product docs and changelog, then the code and configuration. Read docs and changelog pages through the read gate, which fetches, validates, caches, and logs the URL:

    python3 <profile>/skills/browse/scripts/browse.py <url>

An observed behavior in the app outranks a doc claim, which outranks an intention read from code. Capture what the product actually does, not what a page wishes it did. Mark the authority rung on every item you carry into the three files — observed app behavior, doc claim, or code intention. A capability seen only in code is an intention, not an observed behavior; mark it so, so the writer never states an untested capability as a working one.

## 2. Write the problems the product solves into problems.md

Record the problems the product solves, in the user's terms — the situation the user is in that the product resolves, never the internal reason a module exists. Each problem names the feature or process that resolves it and carries its authority rung. This is the problem list a writer reasons over; it is not a market claim that buyers feel these problems, only the record that the product addresses them.

## 3. Write the features into features.md

Record a clean feature list. Each feature carries its user-visible outcome — the "so what" a user gets — and its boundary woven in: what it does not do, its limits, its edges. Cut the invisible plumbing: a feature the user never sees or feels is not a copy feature, so it does not enter this file. A feature with no outcome is a mechanism with no reason to name it; a feature with no boundary invites a claim the product cannot honor. Mark each feature's authority rung.

### Separate fact from offer
A fact about how the product works is not an approved public offer. Record how the product works; never promote a capability into a price, package, or promise — that is the owner's, gated through the plan.

## 4. Write the step-by-step processes into processes.md

For each problem the product solves, record how it solves it as ordered steps in the user's terms — to build a funnel you 1, then 2, then 3. The process is the sequence a user runs, not the call chain the code runs. Each process names the problem it resolves and carries its authority rung.

## 5. Commission proof-of-operation when no customer results exist

When the product has no customer results yet, the proof gap is real research work, not a fabrication to skip. The absence itself — "no customer results yet" — is an owner-stated fact recorded in `product/Brief.md` at setup, not by this thread; proof planning reads that so the gap is a stated fact, never a silent hole a writer fills with an invented outcome. Commission proof-of-operation artifacts — annotated product captures, a sample-account walkthrough, a recorded run of the product completing its core task — that show the product doing what it claims. These substantiate the writer that the capability is real; they are not buyer-facing proof of a buyer's result.

Verification: the app surfaces, docs, and code are mined with observed behavior ranked over intention; the output is the three files `research/product/problems.md`, `research/product/features.md`, `research/product/processes.md` and nothing else; every feature carries its user-visible outcome and its woven boundary with invisible plumbing cut; every process reads as ordered user-facing steps; every problem, feature, and process carries its authority rung with code-only capabilities marked as intention; no capability is framed as an offer, positioning, or strategy; and you read nothing the Inputs section excludes.
