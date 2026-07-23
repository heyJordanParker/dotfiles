---
description: Design a brand-led sales Page or sales Funnel Step from the shared brief and finished copy, including the sales anatomy and the product-trust Checkout Funnel Step starter.
---

# Design a sales Page or sales Funnel Step

Use this after `interview.md`, `writing/copywriting.md`, and `designing.md`. This leaf owns sales anatomy and the Checkout Funnel Step starter because Checkout is part of the sales path; `designing.md` owns the shared visual process, Dent Designer vocabulary, register rules, anti-slop bans, and render verification.

## Process

1. Read the shared brief and finished copy before changing the starter.

   - Confirm `page_job` is to sell the Offer or move the visitor into Checkout.
   - Treat the sales Page as brand-led: the page may use stronger color, weight, proof, and offer focus than a product-trust surface.
   - Pull the hero claim from `promise`, the mechanism from `differentiation`, and the Offer stack from `offer`.
   - Use only proof the brief names in `proof_points`; move unsupported claims to `proof_gaps` instead of decorating them.
   - Repeat one CTA on long pages. Each button should point to the same Checkout or sales action.
   - Use the Checkout starter in this file when the sales path needs a Checkout Funnel Step; Checkout itself stays product-trust and restrained.

   ### Make the argument visible before the Offer
   The reader should see why the current way fails, why this mechanism works, and what changes for them before the price block asks for commitment.
   Never: jump from hero to price with a row of identical benefit cards as the whole argument.

2. Shape the sales anatomy.

   - Hero: promise, outcome subhead, one CTA, one reassurance line, and a strong offer/price preview when the brief has a concrete price.
   - Problem: named cost of staying where the reader is.
   - Mechanism: why this approach works differently.
   - Benefit section: a varied layout, sequence, comparison, proof-backed bullets, or before/after rows; not identical icon cards and not plain pill bullets.
   - Proof: quote, story, demo, credible result, or guarantee near the claim it supports, with enough visual weight to be believed.
   - Offer stack: Product, included value, bonuses, delivery, access, price, guarantee. The price is a focal point, not a small line buried in a card.
   - FAQ: real objections from the brief.
   - Repeated CTA: same action, no competing path.

   ### Use impact without slop
   A sales Page can have a richer hero, deliberate color, a prominent price block, and heavier proof. The bans still apply: no repeated tracked-uppercase eyebrows, invisible CTA, over-tight display letter-spacing, overlong prose, dead whitespace bands, or divider-only hierarchy.
   Never: force product-minimal restraint onto a brand-led marketing Page.

3. Clone and adapt this sales starter.

   Keep the anti-slop structure: no tracked uppercase eyebrow on every section, no border plus `shadow-elevated` decoration on one element, no radius above `rounded-2xl`, no body text wider than `max-w-[65ch]`, and no divider-driven hierarchy.

   ```json
{
     "design": {
       "version": 1,
       "elements": [
         {
           "key": "sales-hero",
           "type": "section",
           "classes": ["bg-primary", "px-4", "py-14", "font-sans", "text-primary-foreground", "antialiased", "sm:px-6", "md:py-24", "lg:px-8", "lg:py-32"],
           "children": [
             {
               "key": "sales-hero-wrap",
               "type": "container",
               "classes": ["mx-auto", "grid", "max-w-7xl", "items-center", "gap-10", "md:grid-cols-[minmax(0,1fr)_minmax(340px,0.9fr)]", "lg:gap-16"],
               "children": [
                 {
                   "key": "sales-copy",
                   "type": "container",
                   "classes": ["flex", "max-w-3xl", "flex-col", "gap-6", "lg:gap-8"],
                   "children": [
                     {"key": "sales-kicker", "type": "text", "tag": "p", "text": "Course launch operating system", "classes": ["text-sm", "font-bold", "text-primary-foreground", "md:text-base"], "children": []},
                     {"key": "sales-headline", "type": "heading", "tag": "h1", "text": "Turn your expertise into a course funnel that can sell this week", "classes": ["text-4xl", "font-black", "leading-none", "tracking-tight", "text-primary-foreground", "md:text-6xl", "lg:text-7xl"], "children": []},
                     {"key": "sales-subhead", "type": "text", "tag": "p", "text": "Package the Offer, publish the sales Page, and launch Checkout with a revenue path you can see instead of a launch plan you hope works.", "classes": ["max-w-[65ch]", "text-lg", "leading-relaxed", "text-primary-foreground", "md:text-xl", "lg:text-2xl"], "children": []},
                     {"key": "sales-cta", "type": "button", "tag": "a", "attributes": {"href": "#checkout"}, "text": "Enroll now", "classes": ["inline-flex", "h-14", "w-full", "items-center", "justify-center", "rounded-2xl", "bg-background", "px-8", "text-base", "font-bold", "text-foreground", "shadow-elevated", "md:h-16", "md:w-fit", "md:text-lg"], "children": []},
                     {"key": "sales-note", "type": "text", "tag": "p", "text": "Includes templates, launch calendar, and the first revenue dashboard.", "classes": ["text-sm", "leading-relaxed", "text-primary-foreground", "md:text-base"], "children": []}
                   ]
                 },
                 {
                   "key": "sales-offer-snapshot",
                   "type": "container",
                   "classes": ["rounded-2xl", "bg-card", "p-6", "text-foreground", "shadow-elevated", "md:p-8", "lg:p-10"],
                   "children": [
                     {"key": "sales-snapshot-title", "type": "heading", "tag": "h2", "text": "Course Launch OS", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                     {"key": "sales-snapshot-copy", "type": "text", "tag": "p", "text": "The complete sales path: promise, Offer, Checkout, delivery, and the revenue review that tells you what to improve next.", "classes": ["mt-3", "text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []},
                     {"key": "sales-snapshot-price", "type": "text", "tag": "p", "text": "$297", "classes": ["mt-6", "text-6xl", "font-black", "leading-none", "tracking-tight", "text-primary", "md:text-7xl"], "children": []},
                     {"key": "sales-snapshot-price-note", "type": "text", "tag": "p", "text": "Today. Instant access. 30-day guarantee.", "classes": ["mt-2", "text-base", "font-bold", "leading-relaxed", "text-foreground"], "children": []}
                   ]
                 }
               ]
             }
           ]
         },
         {
           "key": "sales-problem",
           "type": "section",
           "classes": ["bg-background", "px-4", "py-16", "font-sans", "text-foreground", "sm:px-6", "md:py-24", "lg:px-8"],
           "children": [
             {"key": "sales-problem-wrap", "type": "container", "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "items-start", "md:grid-cols-[0.8fr_1.2fr]", "lg:gap-16"], "children": [
               {"key": "problem-heading", "type": "heading", "tag": "h2", "text": "The painful part is not making the course. It is knowing what has to sell it.", "classes": ["text-4xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl", "lg:text-6xl"], "children": []},
               {"key": "problem-copy", "type": "container", "classes": ["flex", "max-w-[65ch]", "flex-col", "gap-5", "text-lg", "leading-relaxed", "text-foreground", "md:text-xl"], "children": [
                 {"key": "problem-p1", "type": "text", "tag": "p", "text": "Most launches stall because the operator is still choosing between a sales Page, a webinar, another freebie, and a new email sequence.", "children": []},
                 {"key": "problem-p2", "type": "text", "tag": "p", "text": "This course gives you the order: promise, Offer, Checkout, delivery, and the revenue signals that tell you what to improve next.", "children": []}
               ]}
             ]}
           ]
         },
         {
           "key": "sales-mechanism",
           "type": "section",
           "classes": ["bg-muted", "px-4", "py-16", "font-sans", "text-foreground", "sm:px-6", "md:py-24", "lg:px-8"],
           "children": [
             {"key": "mechanism-wrap", "type": "container", "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "items-start", "md:grid-cols-[0.9fr_1.1fr]", "lg:gap-16"], "children": [
               {"key": "mechanism-copy", "type": "container", "classes": ["flex", "max-w-[65ch]", "flex-col", "gap-5"], "children": [
                 {"key": "mechanism-heading", "type": "heading", "tag": "h2", "text": "A launch system ordered by buyer commitment", "classes": ["text-4xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl", "lg:text-6xl"], "children": []},
                 {"key": "mechanism-text", "type": "text", "tag": "p", "text": "You build the moments in the order a buyer experiences them: belief, decision, payment, access, and next action.", "classes": ["text-lg", "leading-relaxed", "text-foreground", "md:text-xl"], "children": []}
               ]},
               {"key": "mechanism-flow", "type": "container", "classes": ["grid", "gap-4"], "children": [
                 {"key": "mechanism-1", "type": "container", "classes": ["rounded-2xl", "bg-card", "p-5", "md:p-6"], "children": [
                   {"key": "mechanism-1-heading", "type": "heading", "tag": "h3", "text": "Belief", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                   {"key": "mechanism-1-copy", "type": "text", "tag": "p", "text": "Sharpen the claim until the visitor can repeat it.", "classes": ["mt-2", "text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []}
                 ]},
                 {"key": "mechanism-2", "type": "container", "classes": ["rounded-2xl", "bg-primary", "p-5", "text-primary-foreground", "md:p-6"], "children": [
                   {"key": "mechanism-2-heading", "type": "heading", "tag": "h3", "text": "Decision", "classes": ["text-2xl", "font-black", "tracking-tight", "text-primary-foreground"], "children": []},
                   {"key": "mechanism-2-copy", "type": "text", "tag": "p", "text": "Make the Offer stack concrete, scoped, and safe.", "classes": ["mt-2", "text-base", "leading-relaxed", "text-primary-foreground", "md:text-lg"], "children": []}
                 ]},
                 {"key": "mechanism-3", "type": "container", "classes": ["rounded-2xl", "bg-card", "p-5", "md:p-6"], "children": [
                   {"key": "mechanism-3-heading", "type": "heading", "tag": "h3", "text": "Payment", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                   {"key": "mechanism-3-copy", "type": "text", "tag": "p", "text": "Remove Checkout uncertainty and keep the buyer moving.", "classes": ["mt-2", "text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []}
                 ]}
               ]}
             ]}
           ]
         },
         {
           "key": "sales-benefits-proof",
           "type": "section",
           "classes": ["bg-background", "px-4", "py-16", "font-sans", "text-foreground", "sm:px-6", "md:py-24", "lg:px-8"],
           "children": [
             {"key": "benefits-proof-wrap", "type": "container", "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "items-start", "md:grid-cols-[1.05fr_0.95fr]", "lg:gap-16"], "children": [
               {"key": "benefit-stack", "type": "container", "classes": ["grid", "gap-6"], "children": [
                 {"key": "benefit-heading", "type": "heading", "tag": "h2", "text": "What changes after the build", "classes": ["text-4xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl", "lg:text-6xl"], "children": []},
                 {"key": "benefit-1", "type": "container", "classes": ["grid", "gap-3", "rounded-2xl", "bg-muted", "p-5", "md:grid-cols-[11ch_1fr]", "md:p-6"], "children": [
                   {"key": "benefit-1-label", "type": "text", "tag": "p", "text": "Offer", "classes": ["text-base", "font-black", "text-primary", "md:text-lg"], "children": []},
                   {"key": "benefit-1-copy", "type": "text", "tag": "p", "text": "You can explain what is included, who it is for, and why buying now makes sense.", "classes": ["text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []}
                 ]},
                 {"key": "benefit-2", "type": "container", "classes": ["grid", "gap-3", "rounded-2xl", "bg-muted", "p-5", "md:grid-cols-[11ch_1fr]", "md:p-6"], "children": [
                   {"key": "benefit-2-label", "type": "text", "tag": "p", "text": "Checkout", "classes": ["text-base", "font-black", "text-primary", "md:text-lg"], "children": []},
                   {"key": "benefit-2-copy", "type": "text", "tag": "p", "text": "The buyer sees the Order, bump, payment step, guarantee, and access expectation without friction.", "classes": ["text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []}
                 ]},
                 {"key": "benefit-3", "type": "container", "classes": ["grid", "gap-3", "rounded-2xl", "bg-muted", "p-5", "md:grid-cols-[11ch_1fr]", "md:p-6"], "children": [
                   {"key": "benefit-3-label", "type": "text", "tag": "p", "text": "Review", "classes": ["text-base", "font-black", "text-primary", "md:text-lg"], "children": []},
                   {"key": "benefit-3-copy", "type": "text", "tag": "p", "text": "You know which revenue signal to improve next instead of relaunching from scratch.", "classes": ["text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []}
                 ]}
               ]},
               {"key": "proof-panel", "type": "container", "classes": ["rounded-2xl", "bg-card", "p-6", "shadow-elevated", "md:p-8", "lg:p-10"], "children": [
                 {"key": "proof-quote", "type": "quote", "text": "The first time I knew exactly what to build next, the whole launch got calmer.", "classes": ["text-3xl", "font-black", "leading-tight", "tracking-tight", "text-foreground", "md:text-5xl"], "children": []},
                 {"key": "proof-author", "type": "text", "tag": "p", "text": "Solo course operator after replacing a scattered launch plan with one measured Funnel path.", "classes": ["mt-5", "text-base", "font-semibold", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []}
               ]}
             ]}
           ]
         },
         {
           "key": "sales-offer",
           "type": "section",
           "classes": ["bg-muted", "px-4", "py-16", "font-sans", "text-foreground", "sm:px-6", "md:py-24", "lg:px-8"],
           "children": [
             {"key": "offer-wrap", "type": "container", "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "items-stretch", "md:grid-cols-[0.78fr_1.22fr]", "lg:gap-16"], "children": [
               {"key": "offer-includes", "type": "container", "classes": ["rounded-2xl", "bg-card", "p-6", "md:p-8", "lg:p-10"], "children": [
                 {"key": "offer-includes-heading", "type": "heading", "tag": "h2", "text": "Everything in the implementation path", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground"], "children": []},
                 {"key": "offer-includes-list", "type": "container", "classes": ["mt-6", "grid", "gap-4", "text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": [
                   {"key": "offer-include-1", "type": "text", "tag": "p", "text": "Course lessons for promise, Offer, sales Page, Checkout, and thank-you.", "classes": ["font-semibold"], "children": []},
                   {"key": "offer-include-2", "type": "text", "tag": "p", "text": "Launch templates and a calendar for the first implementation week.", "classes": ["font-semibold"], "children": []},
                   {"key": "offer-include-3", "type": "text", "tag": "p", "text": "Revenue dashboard review so the next improvement is obvious.", "classes": ["font-semibold"], "children": []}
                 ]}
               ]},
               {"key": "offer-card", "type": "container", "classes": ["rounded-2xl", "bg-primary", "p-6", "text-primary-foreground", "shadow-elevated", "md:p-8", "lg:p-10"], "children": [
                 {"key": "offer-heading", "type": "heading", "tag": "h2", "text": "Course Launch OS", "classes": ["text-3xl", "font-black", "tracking-tight", "text-primary-foreground", "md:text-5xl"], "children": []},
                 {"key": "offer-copy", "type": "text", "tag": "p", "text": "Build the sales path in order, then improve it from revenue signals instead of guesswork.", "classes": ["mt-4", "max-w-[65ch]", "text-lg", "leading-relaxed", "text-primary-foreground", "md:text-xl"], "children": []},
                 {"key": "offer-price", "type": "text", "tag": "p", "text": "$297 today", "classes": ["mt-7", "text-6xl", "font-black", "leading-none", "tracking-tight", "text-primary-foreground", "md:text-7xl"], "children": []},
                 {"key": "offer-guarantee", "type": "text", "tag": "p", "text": "30-day guarantee. Instant access after Checkout.", "classes": ["mt-4", "text-base", "font-bold", "leading-relaxed", "text-primary-foreground", "md:text-lg"], "children": []},
                 {"key": "offer-cta", "type": "button", "tag": "a", "attributes": {"href": "#checkout"}, "text": "Start the course", "classes": ["mt-8", "inline-flex", "h-14", "w-full", "items-center", "justify-center", "rounded-2xl", "bg-background", "px-8", "font-bold", "text-foreground", "shadow-elevated", "md:w-fit"], "children": []}
               ]}
             ]}
           ]
         },
         {
           "key": "sales-faq",
           "type": "section",
           "classes": ["bg-background", "px-4", "py-16", "font-sans", "text-foreground", "sm:px-6", "md:py-24", "lg:px-8"],
           "children": [
             {"key": "faq-wrap", "type": "container", "classes": ["mx-auto", "grid", "max-w-5xl", "gap-8", "sm:px-6", "lg:px-8"], "children": [
               {"key": "faq-heading", "type": "heading", "tag": "h2", "text": "Questions before you enroll", "classes": ["text-4xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl", "lg:text-6xl"], "children": []},
               {"key": "faq-list", "type": "container", "classes": ["grid", "gap-5", "text-base", "leading-relaxed", "text-foreground", "md:text-xl"], "children": [
                 {"key": "faq-1", "type": "text", "tag": "p", "text": "Is this for a small audience? Yes. The system assumes a solo operator with a practical list, not a giant launch team.", "children": []},
                 {"key": "faq-2", "type": "text", "tag": "p", "text": "Do I need to rebuild my whole Site? No. You build the Funnel pieces that make the Offer clear and purchasable.", "children": []},
                 {"key": "faq-3", "type": "text", "tag": "p", "text": "What happens after Checkout? You get instant access and a first implementation sequence.", "children": []}
               ]},
               {"key": "faq-cta", "type": "button", "tag": "a", "attributes": {"href": "#checkout"}, "text": "Enroll now", "classes": ["inline-flex", "h-14", "w-full", "items-center", "justify-center", "rounded-2xl", "bg-primary", "px-8", "font-bold", "text-primary-foreground", "shadow-elevated", "md:w-fit"], "children": []}
             ]}
           ]
         }
       ]
     }
   }
   ```

4. Clone and adapt this Checkout Funnel Step starter when the sales path reaches payment.

   The Checkout starter belongs here because it serves the sales path. It keeps the required elements: `checkout`, `checkout-payment`, `checkout-summary`, `checkout-bump`, and `checkout-submit`. Checkout is product-trust: keep it clear, predictable, and restrained even when the sales Page is brand-led.

   ```json
   {
     "design": {
       "version": 1,
       "elements": [
         {
           "key": "checkout-page",
           "type": "section",
           "classes": ["bg-muted", "px-4", "py-12", "font-sans", "text-foreground", "antialiased", "sm:px-6", "md:py-20", "lg:px-8", "lg:py-24"],
           "children": [
             {
               "key": "checkout-wrap",
               "type": "container",
               "classes": ["mx-auto", "max-w-7xl"],
               "children": [
                 {"key": "checkout-heading", "type": "heading", "tag": "h1", "text": "Complete enrollment", "classes": ["text-center", "text-4xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl", "lg:text-6xl"], "children": []},
                 {"key": "checkout-subhead", "type": "text", "tag": "p", "text": "Secure Checkout. Instant access after your Order is complete.", "classes": ["mx-auto", "mt-4", "max-w-[65ch]", "text-center", "text-lg", "leading-relaxed", "text-foreground", "md:text-xl"], "children": []},
                 {
                   "key": "checkout-form",
                   "type": "checkout",
                   "classes": ["mt-10", "grid", "gap-6", "items-start", "md:grid-cols-[minmax(0,1.25fr)_minmax(320px,0.75fr)]", "lg:gap-10"],
                   "config": {"showLabels": true, "requiredAsterisk": true},
                   "behaviors": [{"behavior": "checkout", "event": "submit", "config": {"billing": "{{ checkout.billing }}", "gateway": "{{ checkout.payment.gateway }}"}}],
                   "children": [
                     {"key": "checkout-billing", "type": "container", "classes": ["rounded-2xl", "bg-card", "p-6", "shadow-elevated", "md:p-8", "lg:p-10"], "children": [
                       {"key": "billing-heading", "type": "heading", "tag": "h2", "text": "Billing details", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                       {"key": "checkout-email", "type": "text-input", "config": {"binding": "billing.email", "kind": "email", "label": "Email", "placeholder": "you@example.com", "required": true}, "children": []},
                       {"key": "checkout-names", "type": "container", "classes": ["grid", "gap-4", "md:grid-cols-2"], "children": [
                         {"key": "checkout-first-name", "type": "text-input", "config": {"binding": "billing.first_name", "kind": "text", "label": "First name", "required": true}, "children": []},
                         {"key": "checkout-last-name", "type": "text-input", "config": {"binding": "billing.last_name", "kind": "text", "label": "Last name", "required": true}, "children": []}
                       ]},
                       {"key": "checkout-payment", "type": "checkout-payment", "config": {"gateways": ["dent_test"]}, "children": []},
                       {"key": "checkout-terms", "type": "checkout-terms", "config": {"label": "I agree to the terms and understand access starts immediately."}, "children": []},
                       {"key": "checkout-submit", "type": "checkout-submit", "config": {"label": "Complete enrollment"}, "children": []}
                     ]},
                     {"key": "checkout-sidebar", "type": "container", "classes": ["rounded-2xl", "bg-card", "p-6", "shadow-elevated", "md:sticky", "md:top-6"], "children": [
                       {"key": "checkout-offer", "type": "checkout-offer", "config": {"showImage": false, "showName": true, "showDescription": true, "showPrice": true, "showStrikethrough": true}, "children": []},
                       {"key": "checkout-summary", "type": "checkout-summary", "children": []},
                       {"key": "checkout-bump", "type": "checkout-bump", "config": {"label": "Add the implementation templates for $37", "description": "Swipe files, prompts, and a launch tracker.", "prechecked": false}, "children": []},
                       {"key": "checkout-trust", "type": "text", "tag": "p", "text": "30-day guarantee. Secure payment. Instant access.", "classes": ["mt-4", "text-sm", "leading-relaxed", "text-foreground", "md:text-base"], "children": []}
                     ]}
                   ]
                 }
               ]
             }
           ]
         }
       ]
     }
   }
   ```

5. Render through Dent and inspect the screenshots.

   - Confirm the sales Page has hero, problem, mechanism, varied benefits, weighted proof, Offer stack, prominent price or guarantee, FAQ, and repeated same CTA.
   - Confirm Checkout shows billing fields, `checkout-payment`, `checkout-summary`, `checkout-bump`, and `checkout-submit` without brand-led over-decoration.
   - Confirm the first screen communicates the promise and next action.
   - Confirm the starter still passes the bans inherited from `designing.md`.
