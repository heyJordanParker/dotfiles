---
description: Design a brand-led opt-in Funnel Step or Page from the shared brief and finished copy, with one compact form, trust support, and a ban-compliant starter Design.
---

# Design an opt-in Funnel Step or Page

Use this after `interview.md`, `writing/copywriting.md`, and `designing.md`. This leaf owns only the opt-in anatomy and starter Design; `designing.md` owns the shared vision, Dent Designer vocabulary, theme classes, register rules, anti-slop bans, and render verification.

## Process

1. Read the shared brief and the finished copy before changing the starter.

   - Confirm `surface` is Page or Funnel Step and `page_job` is to trade attention for the opt-in.
   - Treat this as brand-led unless the brief explicitly says product-trust; the opt-in needs a memorable promise area, not a bare utility form.
   - Pull the headline from `promise`, not from the lead magnet title alone.
   - Use `audience`, `objections`, and `proof_points` to decide the trust note near the form.
   - Keep one CTA. The form submit is the CTA; do not add a competing button.
   - Keep every behavior binding matched to a child field `config.name`.

   ### Put the form in the first decision area
   The visitor should see the promise, the compact form, and the trust note without needing a second section to understand the action.
   Never: make the first fold a generic brand hero and push the opt-in form below it.

2. Shape the opt-in anatomy.

   - Hero: promise headline, short subhead, compact form, privacy or delivery reassurance.
   - Value preview: 3-4 specific outcomes or pieces inside the lead magnet, written as a sequence or checklist rather than identical cards.
   - Trust: one quiet proof row, testimonial, or operator credibility note.
   - CTA: exactly one submit action.

   ### Use a panel only where it behaves like an affordance
   The form can be a `bg-card rounded-2xl shadow-elevated` panel because it is interactive. Brand-led impact can come from the hero color, scale, contrast, proof placement, and one strong form focal area.
   Never: wrap every benefit and proof point in matching icon-heading-text cards.

3. Clone and adapt this starter.

   Replace copy, field labels, and form behavior fields. Keep the anti-slop structure: no repeated tracked uppercase eyebrows, no `border` plus `shadow-elevated` decoration on the same element, no radius above `rounded-2xl`, no body text wider than `max-w-[65ch]`, and no trailing empty/min-height band after the last section.

   ```json
   {
     "design": {
       "version": 1,
       "elements": [
         {
           "key": "optin-hero",
           "type": "section",
           "classes": ["w-full", "bg-primary", "px-4", "py-12", "font-sans", "text-primary-foreground", "antialiased", "sm:px-6", "md:py-20", "lg:px-8", "lg:py-28"],
           "children": [
             {
               "key": "optin-hero-wrap",
               "type": "container",
               "classes": ["mx-auto", "grid", "max-w-7xl", "items-center", "gap-10", "md:grid-cols-[minmax(0,1.08fr)_minmax(320px,0.92fr)]", "lg:gap-16"],
               "children": [
                 {
                   "key": "optin-copy",
                   "type": "container",
                   "classes": ["flex", "max-w-3xl", "flex-col", "gap-6", "lg:gap-8"],
                   "children": [
                     {"key": "optin-kicker", "type": "text", "tag": "p", "text": "Free operator checklist", "classes": ["text-sm", "font-bold", "text-primary-foreground", "md:text-base"], "children": []},
                     {"key": "optin-headline", "type": "heading", "tag": "h1", "text": "Know the next Funnel Step to build before you spend another weekend guessing", "classes": ["text-4xl", "font-black", "leading-none", "tracking-tight", "text-primary-foreground", "md:text-6xl", "lg:text-7xl"], "children": []},
                     {"key": "optin-subhead", "type": "text", "tag": "p", "text": "Get the practical launch map for turning a lead magnet, Checkout, and follow-up sequence into a revenue path you can measure.", "classes": ["max-w-[65ch]", "text-lg", "leading-relaxed", "text-primary-foreground", "md:text-xl", "lg:text-2xl"], "children": []},
                     {"key": "optin-trust-line", "type": "text", "tag": "p", "text": "Built from three shipped funnels and written for solo course operators.", "classes": ["max-w-[65ch]", "rounded-2xl", "bg-background", "p-4", "text-base", "font-semibold", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []}
                   ]
                 },
                 {
                   "key": "optin-form-panel",
                   "type": "container",
                   "classes": ["rounded-2xl", "bg-card", "p-5", "shadow-elevated", "md:p-8"],
                   "children": [
                     {"key": "optin-form-title", "type": "heading", "tag": "h2", "text": "Send me the launch map", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                     {"key": "optin-form-copy", "type": "text", "tag": "p", "text": "The checklist arrives by email, followed by one implementation lesson.", "classes": ["mt-3", "text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []},
                     {
                       "key": "optin-form",
                       "type": "form",
                       "classes": ["mt-6", "flex", "flex-col", "gap-4"],
                       "behaviors": [
                         {"behavior": "validate-email", "event": "submit", "config": {"email": "{{ fields.email }}"}},
                         {"behavior": "optin", "event": "submit", "config": {"email": "{{ fields.email }}", "firstName": "{{ fields.first_name }}", "nameMode": "split"}}
                       ],
                       "children": [
                         {"key": "optin-name", "type": "text-input", "config": {"kind": "text", "name": "first_name", "label": "First name", "placeholder": "Avery", "required": false}, "children": []},
                         {"key": "optin-email", "type": "text-input", "config": {"kind": "email", "name": "email", "label": "Email", "placeholder": "you@example.com", "required": true}, "children": []},
                         {"key": "optin-submit", "type": "form-submit", "config": {"label": "Send me the launch map"}, "children": []}
                       ]
                     },
                     {"key": "optin-note", "type": "text", "tag": "p", "text": "No spam. Unsubscribe anytime.", "classes": ["mt-4", "text-sm", "leading-relaxed", "text-foreground"], "children": []}
                   ]
                 }
               ]
             }
           ]
         },
         {
           "key": "optin-preview",
           "type": "section",
           "classes": ["bg-background", "px-4", "py-12", "font-sans", "text-foreground", "sm:px-6", "md:py-20", "lg:px-8"],
           "children": [
             {
               "key": "optin-preview-wrap",
               "type": "container",
               "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "items-start", "md:grid-cols-[0.82fr_1.18fr]", "lg:gap-16"],
               "children": [
                 {"key": "optin-preview-heading", "type": "heading", "tag": "h2", "text": "What the checklist clarifies", "classes": ["text-4xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl", "lg:text-6xl"], "children": []},
                 {
                   "key": "optin-preview-list",
                   "type": "container",
                   "classes": ["grid", "gap-4", "text-base", "leading-relaxed", "text-foreground", "md:text-xl"],
                   "children": [
                     {"key": "optin-preview-1", "type": "text", "tag": "p", "text": "Which promise belongs on the opt-in step, not buried in follow-up copy.", "classes": ["rounded-xl", "bg-muted", "p-5", "font-semibold"], "children": []},
                     {"key": "optin-preview-2", "type": "text", "tag": "p", "text": "Where Checkout, the Offer, and the thank-you step need to connect.", "classes": ["rounded-xl", "bg-muted", "p-5", "font-semibold"], "children": []},
                     {"key": "optin-preview-3", "type": "text", "tag": "p", "text": "Which revenue signals to watch once the Funnel is live.", "classes": ["rounded-xl", "bg-muted", "p-5", "font-semibold"], "children": []}
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

   - Confirm the first screen contains the promise, form, and trust note.
   - Confirm there is no second CTA competing with the form submit.
   - Confirm every `{{ fields.* }}` behavior reference has a matching `text-input` `config.name`.
   - Confirm there is no trailing empty whitespace band after the last section on desktop or mobile.
   - Confirm the starter still passes the bans inherited from `designing.md`.
