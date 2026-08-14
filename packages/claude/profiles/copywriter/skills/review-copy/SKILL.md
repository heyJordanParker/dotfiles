---
name: review-copy
description: Run a review round on a deliverable — dispatch the applicable checks in parallel, collect findings, and drive the iteration to clean. TRIGGER when a draft or a set is ready for its checks, or when the chief runs a review round. DO NOT TRIGGER to run one check in isolation (the individual check skills) or to apply findings to a draft (revise).
---

# Review Copy

One Process: run the checks a deliverable needs, gather what they find, and iterate until it is clean. Iteration is the normal path — nothing "fails" back to the owner. A round produces findings; the findings drive revisions; the affected checks rerun; the loop repeats until the deliverable converges. Only converged work goes to the chief for the will-this-sell judgment.

The deliverable is the unit. For a standalone project, its folder — scaffolded by start — holds `drafts/`, `findings/`, `rounds/`, the three plan files (`Reader.md`, `Brief.md`, `Proof.md`), and the writer's `Wireframe.md`. For a campaign PIECE the plan files resolve two ways: the piece folder carries its own `Reader.md` (plus `drafts/`, `findings/`, `rounds/`, and `Wireframe.md`), and the checks read the campaign-level `Brief.md` and `Proof.md` the pieces share, which live at the campaign root, not in the piece folder. A check that needs a piece's Brief.md or Proof.md reads the campaign's; only Reader.md is per piece. Findings live in `findings/<Check>-<round>.md` (the finding format defined in step 2), immutable per round. A campaign holds one such subfolder per deliverable; review runs against each.

## 1. Select the checks the deliverable needs

### Match the checks to the deliverable
Always, on every deliverable: check-strategy on the built piece, check-reality on the built piece, buyer-review, check-claims, cro-review, and the read-only line passes edit-sentences, check-structure, and check-ai-writing — the line passes run here so the last edit before ship is gated. check-strategy is the review-side half of the gate the chief also runs on the assembled selection at plan-copy 1d, before any plan file exists. check-reality is the review-side half of the reality attack the chief also runs on the research and strategy artifacts at plan-copy 1d — on the built piece it attacks whether the copy reintroduced a market, buyer, or problem no evidence holds, rating its existence 1 to 100, its category separate from check-strategy's argument construction. A campaign adds the set-level continuity pass in check-structure against the campaign as its own reviewable unit. review-design is NOT part of this loop — it gates the rendered result after design renders, dispatched by the chief once the design exists, never here on the copy.

### Skip a check that has nothing to check
The chief skips a selected check when the deliverable gives it nothing to judge — cro-review on a link-free tweet, set-level continuity on a single standalone piece. Name each skipped check and the reason in the round record, so a skip is a recorded decision, never a silent omission.

### Rerun copycheck on every changed draft
Every changed draft reruns the deterministic script `scripts/copycheck.py` (real path `~/.claude/profiles/copywriter/scripts/copycheck.py`: counts, sentence stats, banned words, placeholders, repetition) each round before the round is judged clean. The editor runs it. Convergence additionally requires `copycheck.py --strict` to exit 0 on the final text — advisory ordinary-mode output does not gate, `--strict` does.

## 2. Dispatch in parallel

### Run every applicable check at once
Dispatch the selected checks concurrently — one message, one subagent per check — never in sequence. Each writes its findings to the deliverable's findings folder for this round.

### Prime every reviewer as a destroyer, never an improver
Each dispatch tells the reviewer to assume the artifact is broken until evidence survives — its job is to destroy what does not hold, not to validate what plausibly could. Reviewers hinge agreeable when handed work as if it were sound; the prime is what makes the finding real. A reviewer that returns "looks good" without having attacked the artifact ran no check.

### Record what each check caught
A check that fixes something records what it caught in its finding file, so the failure traces back and the upstream skill is updated to produce fewer of them. A silent fix teaches the system nothing.

### Rounds are read-only
No check mutates the draft — every check reports findings and suggested edits, none edits. The line passes (edit-sentences, check-structure, check-ai-writing) are findings-only here exactly as they are beside the writer in production. A round produces findings; revise is where the writer folds the accepted fixes into the next numbered draft.

### Write findings in THE finding format
Every check writes one file per round, in `findings/`, named `<Check>-<round>.md` (e.g. `CheckClaims-002.md`). Each finding in it carries these fields:

Template:
    - check: <the check that raised it>
    - severity: blocking | note
    - location: <section, line, or title the finding sits on>
    - finding: <what is wrong>
    - suggested fix: <what would resolve it — a suggestion for revise, never an edit>

## 3. Collect and record the round

### Write one immutable round record
Collect the findings and write `rounds/<NNN>.md`: each deliverable, its hash, the checks run, the finding files referenced by hash, and the blocking count. A check whose declared inputs are unchanged since its last run carries its previous finding forward by hash — selective reruns are legal and countable.

## 4. Iterate to clean

### Drive revisions and rerun the affected checks
Hand blocking findings back to the owning WRITER, who runs revise on its own draft — the writer that authored the piece is the sole reviser of it, each finding targeted at the skill that owns it. Then rerun only the checks whose inputs changed. Repeat until a deliverable has two consecutive clean rounds — zero blocking findings on its current hash with all required checks present and `copycheck.py --strict` exiting 0 on the final text. The SECOND clean round of a convergence must run every applicable check fresh; carry-forward never counts in the final round. A clean campaign requires the set-level checks clean too; five clean deliverables cannot hide a blocked sequence.

### Escalate nothing to the owner mid-loop
Iteration never blocks on the owner. Parallel branches keep moving; only a converged, clean deliverable is assembled into the chief's proposal.

IF a finding changes an owner-settled fact:
### Send an owner-settled-fact finding out of the loop, everything else stays in
One finding leaves the loop: one that changes a fact the owner settled — an unsourceable claim, a new offer, the wrong audience. It cannot resolve inside the loop. Hand it to the chief, and to the owner if it is his; the affected upstream stage reruns and the piece re-enters production. That is the exception path, not the loop. The loop keeps moving on everything else while he does. Every other finding stays inside the loop and feeds revise.

IF check-reality returns a low existence rating with a fabrication finding on the built piece:
### Route a fabrication finding to the chief, never revise around it
A check-reality finding that the copy rests on a problem read off a product feature, on writer-invented language, or on a problem that never mattered never feeds revise — polishing copy around a problem no evidence holds ships the fabrication cleaner. Hand it to the chief with its existence rating and reasoning: the chief re-commissions the research that produced the record, lowers the judged rating in `Problems.md` or `Buyers.md`, or strikes the record, and the affected work rebuilds. Draft revision cannot resolve it. Nothing is auto-killed and no verdict transcript persists — the chief holds the finding and corrects the record or the rating.

Verification: every applicable check dispatched and its finding recorded by hash in the round; the loop run until two consecutive clean rounds; the round records immutable and never reset.
