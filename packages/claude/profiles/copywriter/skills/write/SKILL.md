---
name: write
description: Run the production phase for one project — draft each asset from its plan with the fitting write skill, drive it through the review loop to convergence, and hand version picks to the owner. Produces the finished copy. TRIGGER when a project's plan files are set and the copy is ready to be written. DO NOT TRIGGER to plan the piece's strategy (strategy) or to run one check in isolation (the check skills).
---

# Write

One Process: turn a project's plan files into finished copy — drafted, revised, and converged under review. This owns the phase; each write skill owns its asset shape and each check owns its judgment, never restated here.

## 1. Draft from the plan with the fitting write skill

Pick the write skill the asset's type names — write-ads, write-emails, write-headlines, write-hooks, write-leads, write-optin-page, write-catalog-page, write-sales-page, write-social-posts, write-story, write-video-script, or write-vsl — each drawing on the shared craft model in copywriting. The writer drafts from Reader.md, Brief.md, and Proof.md, and folds line-pass findings to locally clean through revise before review.

## 2. Run the review loop to convergence

Run review-copy each round: it dispatches the applicable checks, and the editor reruns the deterministic `scripts/copycheck.py` on every changed draft. Blocking findings go back to the authoring writer, who runs revise on its own draft. The loop repeats until two consecutive clean rounds with `copycheck.py --strict` exiting 0.

## Owner boundary

The phase stops at the version picks. Where a piece was drafted in versions, the converged options go to the owner to pick; the phase never auto-selects one.

## Verification

Every asset has a converged draft — two consecutive clean review rounds with all applicable checks present and `copycheck.py --strict` exiting 0 — and version picks are handed to the owner.
