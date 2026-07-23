---
description: Design a brand-led one-click upsell Funnel Step from the shared brief and finished copy, with purchase confirmation, one checkout-offer, dominant accept, and quiet decline.
---

# Design a one-click upsell Funnel Step

Use this after `interview.md`, `writing/copywriting.md`, and `designing.md`. This leaf owns only the upsell anatomy and starter Design; `designing.md` owns the shared visual process, Dent Designer vocabulary, register rules, anti-slop bans, and render verification.

## Process

1. Read the shared brief and finished copy before changing the starter.

   - Confirm `surface` is Upsell Funnel Step and the buyer has just completed an Order.
   - Treat this as brand-led but single-decision: it can use impact, contrast, and a dominant accept action, but it must not become a second long sales Page.
   - Use `offer` for the one upgrade being presented. Do not introduce a second Product or cross-sell.
   - Use `proof_points` and `objections` to support why the upgrade belongs now.
   - Keep the accept path dominant and the decline path quiet.
   - Keep `checkout-offer` in the design so the live upsell Offer can render price and details.

   ### Treat the prior purchase as trust already earned
   The page should feel like a useful next choice after a confirmed Order, not a second long sales Page.
   Never: re-run the entire sales argument or add a menu of upgrades.

2. Shape the upsell anatomy.

   - Confirmation: acknowledge the Order first.
   - One Offer: one relevant upgrade using `checkout-offer`.
   - Why now: explain the timing and the improved outcome.
   - Benefits: 3-5 concrete improvements, arranged as a compact decision support block, not a card grid.
   - Accept: dominant one-click action.
   - Decline: quiet text Link.

   ### Make refusal safe
   The decline Link must be visible and plain, but never visually compete with the accept action.
   Never: hide decline, shame the buyer, or make decline look like a second primary CTA.

3. Clone and adapt this starter.

   Replace copy and URLs with the live upsell accept and decline bindings your Dent context exposes. Keep the anti-slop structure: no repeated tracked uppercase eyebrows, no border plus `shadow-elevated` decoration on one element, no radius above `rounded-2xl`, and no body text wider than `max-w-[65ch]`.

   ```json
   {
     "design": {
       "version": 1,
       "elements": [
         {
           "key": "upsell-page",
           "type": "section",
           "classes": ["bg-primary", "px-4", "py-12", "font-sans", "text-primary-foreground", "antialiased", "sm:px-6", "md:py-20", "lg:px-8", "lg:py-28"],
           "children": [
             {
               "key": "upsell-wrap",
               "type": "container",
               "classes": ["mx-auto", "grid", "max-w-7xl", "items-center", "gap-10", "md:grid-cols-[minmax(0,0.95fr)_minmax(320px,1.05fr)]", "lg:gap-16"],
               "children": [
                 {
                   "key": "upsell-copy",
                   "type": "container",
                   "classes": ["flex", "max-w-3xl", "flex-col", "gap-6", "lg:gap-8"],
                   "children": [
                     {"key": "upsell-confirmation", "type": "text", "tag": "p", "text": "Your Order is confirmed.", "classes": ["rounded-2xl", "bg-background", "p-4", "text-base", "font-bold", "text-foreground", "md:text-lg"], "children": []},
                     {"key": "upsell-heading", "type": "heading", "tag": "h1", "text": "Add the implementation sprint before you start", "classes": ["text-4xl", "font-black", "leading-none", "tracking-tight", "text-primary-foreground", "md:text-6xl", "lg:text-7xl"], "children": []},
                     {"key": "upsell-intro", "type": "text", "tag": "p", "text": "You already have the course. This upgrade gives you the prompts, templates, and review checklist that make the first implementation faster.", "classes": ["max-w-[65ch]", "text-lg", "leading-relaxed", "text-primary-foreground", "md:text-xl", "lg:text-2xl"], "children": []},
                     {"key": "upsell-benefits", "type": "container", "classes": ["grid", "gap-4", "text-base", "leading-relaxed", "md:text-xl"], "children": [
                       {"key": "upsell-benefit-1", "type": "text", "tag": "p", "text": "Turn the first lesson into a working Funnel checklist.", "classes": ["rounded-xl", "bg-background", "p-5", "font-semibold", "text-foreground"], "children": []},
                       {"key": "upsell-benefit-2", "type": "text", "tag": "p", "text": "Use swipe files for the Offer, Checkout, and thank-you steps.", "classes": ["rounded-xl", "bg-background", "p-5", "font-semibold", "text-foreground"], "children": []},
                       {"key": "upsell-benefit-3", "type": "text", "tag": "p", "text": "Review the launch against revenue signals, not vibes.", "classes": ["rounded-xl", "bg-background", "p-5", "font-semibold", "text-foreground"], "children": []}
                     ]}
                   ]
                 },
                 {
                   "key": "upsell-offer-panel",
                   "type": "container",
                   "classes": ["rounded-2xl", "bg-card", "p-6", "text-foreground", "shadow-elevated", "md:p-8", "lg:p-10"],
                   "children": [
                     {"key": "upsell-offer-heading", "type": "heading", "tag": "h2", "text": "One decision before access", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl"], "children": []},
                     {"key": "upsell-offer-copy", "type": "text", "tag": "p", "text": "Add the sprint now and implement while the course is fresh.", "classes": ["mt-3", "text-base", "font-semibold", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []},
                     {"key": "upsell-offer", "type": "checkout-offer", "config": {"showImage": false, "showName": true, "showDescription": true, "showPrice": true, "showStrikethrough": true}, "children": []},
                     {"key": "upsell-proof", "type": "text", "tag": "p", "text": "Best for operators who want the course implemented this week, not bookmarked for later.", "classes": ["mt-5", "rounded-2xl", "bg-muted", "p-4", "text-base", "font-semibold", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []},
                     {"key": "upsell-accept", "type": "button", "tag": "a", "attributes": {"href": "{{ offer.acceptUrl }}"}, "text": "Yes, add this to my Order", "classes": ["mt-8", "inline-flex", "h-16", "w-full", "items-center", "justify-center", "rounded-2xl", "bg-primary", "px-8", "text-lg", "font-black", "text-primary-foreground", "shadow-elevated", "md:text-xl"], "children": []},
                     {"key": "upsell-decline", "type": "button", "tag": "a", "attributes": {"href": "{{ offer.declineUrl }}"}, "text": "No thanks, continue to my access", "classes": ["mt-4", "inline-flex", "w-full", "items-center", "justify-center", "text-sm", "font-semibold", "text-foreground", "md:text-base"], "children": []}
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

4. Render through Dent and inspect the screenshots.

   - Confirm the Order confirmation appears before the upgrade pitch.
   - Confirm there is exactly one `checkout-offer`.
   - Confirm accept is visually dominant and decline is quiet but readable.
   - Confirm the starter still passes the bans inherited from `designing.md`.
