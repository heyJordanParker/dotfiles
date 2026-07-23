---
description: Write Dent Page and Funnel Step copy from the interview brief before design, using derived strategy and page-type leaves.
---

# Write the copy

Use this after `interview.md` has produced the shared brief. The failure it prevents: designing a Page or Funnel Step around placeholder copy, or re-asking the operator for strategy labels the brief already derived.

## Process

1. Consume the shared brief before drafting.

   - Read every field in the interview brief.
   - Treat `customer_language`, `proof_points`, `proof_gaps`, `offer`, and `objections` as hard inputs.
   - Derive reader stage and market level from `reader_awareness_stage` and `market_sophistication`; do not ask the operator to classify them again.
   - Match the copy to the source and surface named in `surface` and `page_job`.

   IF `open_questions` contains anything that blocks the promise, Offer, proof, or required action:
   ### Ask only for the missing plain-language fact
   Ask the operator for the missing fact in their words, then update the brief before writing.
   Example: "What real result or example can we point to for this claim?"
   Never: invent proof, urgency, guarantees, customer names, or results.

2. Lock the promise, position, and Offer.

   - Use `promise` as the single before-to-after outcome.
   - Use `differentiation` to name what this replaces and the one axis where the Offer wins.
   - Use `offer` to state what the visitor gets, what they give up, what reduces risk, and why acting now is honest.
   - For a Checkout Funnel Step, treat trust, clarity, and completion as the persuasion job; do not re-sell with a second argument.
   - For an Article, keep the Article's reader promise and one contextual next action separate from sales-page pressure.

   ### Keep one argument per surface
   A Page or Funnel Step is one line of reasoning, not a pile of features.
   Never: average several audiences, Offers, or CTAs into one generic Page.

3. Find the big idea and lead.

   - Choose the one fresh idea that makes the visitor see their situation differently.
   - Pick the lead from the derived reader state: story or identity for cold readers, problem for pain-aware readers, promise and mechanism for solution-aware readers, proof and edge for product-aware readers, terms for most-ready readers.
   - Use the market level to decide whether a plain claim is enough, whether the mechanism must be named, or whether identity and trust must carry the argument.
   - Keep the idea tied to the customer's desire and to proof the brief can support.

   ### Let the idea choose the structure
   Use PAS, BAB, AIDA, 4Ps, PASTOR, Star-Story-Solution, or Feature-Benefit Bridge only when it carries this idea for this reader.
   Never: force a page-type recipe to carry an idea it does not fit.

4. Write the copy in claim-proof pairs.

   - Write the headline first, then the hook or subheadline that makes the first reason to believe concrete.
   - Write section headlines so a scanner can reconstruct the argument from the headings alone.
   - Pair each major claim with proof near the claim: testimonial, metric, demo, screenshot, founder credibility, mechanism proof, risk reversal, or a clearly marked proof gap.
   - Translate every feature into an outcome before it appears.
   - Write bullets as curiosity-driven benefit lines that open an honest loop and pay it off later.
   - Use `customer_language` before category language.
   - Write CTAs as the outcome or next step, not a vague command.

   ### Proof limits the claim
   If the brief lacks evidence, lower the claim or flag the gap. A smaller true claim beats a bigger unsupported one.
   Never: dress a hypothesis as a result.

5. Use the page-type writing leaf for block order.

   These leaves own page-specific section order and required copy blocks. The overarching process does not duplicate their recipes.

   - Writing an opt-in Page or lead-capture Funnel Step → `writing/optin-page.md`
   - Writing a sales Page or sales Funnel Step → `writing/sales-page.md`
    - Writing a Checkout Funnel Step → use the Checkout guidance in `writing/sales-page.md`
   - Writing an upsell Funnel Step → `writing/upsell-page.md`
   - Writing a thank-you Page or Funnel Step → `writing/thank-you-page.md`
   - Writing an Article or content Page → `writing/article.md`

6. Route drafted copy through the final editing pass.

   AI tells, pompous wording, and wall-of-text cleanup before design → `writing/editing.md`

   ### Distinctiveness must survive cleanup
   A clean line that any competitor could paste onto their own Page still fails. Rewrite it around the brief's specific customer, proof, mechanism, or Offer.

   - Output copy by section: headline, subheadline, CTA, body copy, bullets, proof, FAQ, form labels, and microcopy.
   - Mark any proof gap inline so design does not turn it into a visual claim.
   - Do not author the Dent Designer JSON here. The design process consumes this finished copy next.
