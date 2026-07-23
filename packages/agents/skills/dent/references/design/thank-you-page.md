---
description: Design a thank-you Funnel Step or Page from the shared brief and finished copy, with confirmation, next step, optional bridge, and a ban-compliant starter Design.
---

# Design a thank-you Funnel Step or Page

Use this after `interview.md`, `writing/copywriting.md`, and `designing.md`. This leaf owns only the thank-you anatomy and starter Design; `designing.md` owns the shared visual process, Dent Designer vocabulary, anti-slop bans, and render verification.

## Process

1. Read the shared brief and finished copy before changing the starter.

   - Confirm `surface` is Page or Funnel Step and `page_job` is confirmation plus the next best action.
   - Use `offer` and `constraints` to decide whether the next action is inbox, Course access, account login, calendar, community, or a bridge Offer.
   - Keep the confirmation clear and immediate.
   - Add only one primary next action.
   - Use optional bridge content only when it helps the buyer or subscriber continue.

   ### Make success unmistakable
   The first screen should answer: did it work, what happens next, and what should I do now?
   Never: use a vague celebratory headline without delivery or access details.

2. Shape the thank-you anatomy.

   - Confirmation: success headline and what just happened.
   - Next step: one action with a button or clear instruction.
   - Reassurance: delivery timing, support, receipt, or login note.
   - Optional bridge: one next Offer, Course, Space, or Article when it naturally follows.

   ### Keep the page calm after conversion
   This is a completion surface. Use generous spacing, clear type, and one useful panel instead of a busy marketing recap.
   Never: turn the thank-you Page into another full sales Page unless the Funnel intentionally continues.

3. Clone and adapt this starter.

   Replace copy and the next action Link. Keep the anti-slop structure: no repeated tracked uppercase eyebrows, no border plus `shadow-elevated` decoration on one element, no radius above `rounded-2xl`, and no body text wider than `max-w-[65ch]`.

   ```json
   {
     "design": {
       "version": 1,
       "elements": [
         {
           "key": "thanks-hero",
           "type": "section",
           "classes": ["bg-background", "px-4", "py-16", "font-sans", "text-foreground", "antialiased", "sm:px-6", "md:py-24", "lg:px-8", "lg:py-28"],
           "children": [
             {
               "key": "thanks-wrap",
               "type": "container",
               "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "md:grid-cols-[1fr_0.85fr]", "items-start", "lg:gap-16"],
               "children": [
                 {
                   "key": "thanks-copy",
                   "type": "container",
                   "classes": ["flex", "max-w-[65ch]", "flex-col", "gap-6", "lg:gap-8"],
                   "children": [
                     {"key": "thanks-confirm", "type": "text", "tag": "p", "text": "You are in.", "classes": ["text-base", "font-bold", "text-primary", "md:text-lg"], "children": []},
                     {"key": "thanks-heading", "type": "heading", "tag": "h1", "text": "Check your inbox for the launch map", "classes": ["text-4xl", "font-black", "leading-none", "tracking-tight", "text-foreground", "md:text-6xl", "lg:text-7xl"], "children": []},
                     {"key": "thanks-subhead", "type": "text", "tag": "p", "text": "The checklist is on its way. The first implementation lesson follows so you can choose the next Funnel Step with confidence.", "classes": ["text-lg", "leading-relaxed", "text-foreground", "md:text-xl", "lg:text-2xl"], "children": []},
                     {"key": "thanks-cta", "type": "button", "tag": "a", "attributes": {"href": "/"}, "text": "Back to the Site", "classes": ["inline-flex", "h-14", "w-full", "items-center", "justify-center", "rounded-2xl", "bg-primary", "px-8", "text-base", "font-bold", "text-primary-foreground", "shadow-elevated", "md:h-16", "md:w-fit", "md:text-lg"], "children": []}
                   ]
                 },
                 {
                   "key": "thanks-next-panel",
                   "type": "container",
                   "classes": ["rounded-2xl", "bg-muted", "p-6", "md:p-8", "lg:p-10"],
                   "children": [
                     {"key": "next-heading", "type": "heading", "tag": "h2", "text": "What happens next", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                     {"key": "next-list", "type": "list", "classes": ["mt-6", "grid", "gap-4", "text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": [
                       {"key": "next-1", "type": "list-item", "text": "Open the email and save the checklist where you plan launches.", "classes": ["rounded-xl", "bg-card", "p-4"], "children": []},
                       {"key": "next-2", "type": "list-item", "text": "Use the first page to identify the one Funnel Step with the most leverage.", "classes": ["rounded-xl", "bg-card", "p-4"], "children": []},
                       {"key": "next-3", "type": "list-item", "text": "Reply if the email does not arrive within a few minutes.", "classes": ["rounded-xl", "bg-card", "p-4"], "children": []}
                     ]}
                   ]
                 }
               ]
             }
           ]
         },
         {
           "key": "thanks-bridge",
           "type": "section",
           "classes": ["bg-muted", "px-4", "py-16", "font-sans", "text-foreground", "sm:px-6", "md:py-24", "lg:px-8", "lg:py-28"],
           "children": [
             {"key": "bridge-wrap", "type": "container", "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "md:grid-cols-[0.9fr_1.1fr]", "items-center", "lg:gap-16"], "children": [
               {"key": "bridge-heading", "type": "heading", "tag": "h2", "text": "Want the full build order?", "classes": ["text-4xl", "font-black", "tracking-tight", "text-foreground", "md:text-5xl", "lg:text-6xl"], "children": []},
               {"key": "bridge-copy", "type": "container", "classes": ["flex", "max-w-[65ch]", "flex-col", "gap-5"], "children": [
                 {"key": "bridge-text", "type": "text", "tag": "p", "text": "If you are ready to build beyond the checklist, continue into the Course Launch OS and assemble the whole sales path.", "classes": ["text-lg", "leading-relaxed", "text-foreground", "md:text-xl"], "children": []},
                 {"key": "bridge-link", "type": "button", "tag": "a", "attributes": {"href": "/"}, "text": "See the course", "classes": ["inline-flex", "h-14", "w-full", "items-center", "justify-center", "rounded-2xl", "bg-primary", "px-8", "text-base", "font-bold", "text-primary-foreground", "shadow-elevated", "md:h-16", "md:w-fit", "md:text-lg"], "children": []}
               ]}
             ]}
           ]
         }
       ]
     }
   }
   ```

4. Render through Dent and inspect the screenshots.

   - Confirm the first screen says success, next step, and the one primary action.
   - Confirm optional bridge content is clearly secondary.
   - Confirm support or delivery reassurance is present.
   - Confirm the starter still passes the bans inherited from `designing.md`.
