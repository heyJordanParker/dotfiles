---
description: Design an Article or content Page from the shared brief and finished copy, with a designed header, readable content column, and one contextual CTA.
---

# Design an Article or content Page

Use this after `interview.md`, `writing/copywriting.md`, and `designing.md`. This leaf owns only the Article anatomy and starter Design; `designing.md` owns the shared visual process, Dent Designer vocabulary, anti-slop bans, and render verification.

## Process

1. Read the shared brief and finished copy before changing the starter.

   - Confirm `surface` is Article or content Page and `page_job` is understanding before conversion.
   - Use the finished copy to decide heading hierarchy; do not invent sections to make the layout busier.
   - Use `audience`, `reader_awareness_stage`, and `objections` to place one contextual CTA where it helps the reader continue.
   - Keep the body column readable with `max-w-[65ch]` and `prose` when the content is long-form.
   - Use one CTA. It should match the Article's context, not hijack the piece.

   ### Let reading be the main interaction
   The Article should feel designed without interrupting comprehension.
   Never: turn each paragraph into a card, add repeated section eyebrows, or break the reading column with decorative grids.

2. Shape the Article anatomy.

   - Header: designed title area, summary, author or context note when useful.
   - Body: readable content column using semantic headings, text, quotes, lists, and optional callouts.
   - Contextual CTA: one relevant next action, placed after the argument or next to a practical takeaway.

   ### Use prose styling for the article body
   The `prose` class is the Dent-supported content reading affordance. Keep the body to `max-w-[65ch]` and avoid full-width paragraphs.
   Never: stretch long-form text across `max-w-6xl` because the page has room.

3. Clone and adapt this starter.

   Replace the title, summary, body, and CTA. Keep the anti-slop structure: no repeated tracked uppercase eyebrows, no border plus `shadow-elevated` decoration on one element, no radius above `rounded-2xl`, and no body text wider than `max-w-[65ch]`.

   ```json
   {
     "design": {
       "version": 1,
       "elements": [
         {
           "key": "article-header",
           "type": "section",
           "classes": ["bg-muted", "px-4", "py-16", "font-sans", "text-foreground", "antialiased", "sm:px-6", "md:py-24", "lg:px-8", "lg:py-28"],
           "children": [
             {
               "key": "article-header-wrap",
               "type": "container",
               "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "md:grid-cols-[0.75fr_1.25fr]", "items-end", "lg:gap-16"],
               "children": [
                 {"key": "article-meta", "type": "text", "tag": "p", "text": "Field note for course operators", "classes": ["text-sm", "font-bold", "text-primary", "md:text-base"], "children": []},
                 {"key": "article-title-block", "type": "container", "classes": ["flex", "max-w-[65ch]", "flex-col", "gap-6"], "children": [
                   {"key": "article-title", "type": "heading", "tag": "h1", "text": "The Funnel Step to build when your course launch feels random", "classes": ["text-4xl", "font-black", "leading-none", "tracking-tight", "text-foreground", "md:text-6xl", "lg:text-7xl"], "children": []},
                   {"key": "article-summary", "type": "text", "tag": "p", "text": "A practical way to choose the next sales surface by buyer commitment, not by what everyone else seems to be publishing.", "classes": ["text-lg", "leading-relaxed", "text-foreground", "md:text-xl", "lg:text-2xl"], "children": []}
                 ]}
               ]
             }
           ]
         },
         {
           "key": "article-body",
           "type": "section",
           "classes": ["bg-background", "px-4", "py-16", "font-sans", "text-foreground", "sm:px-6", "md:py-24", "lg:px-8"],
           "children": [
             {"key": "article-body-wrap", "type": "container", "classes": ["mx-auto", "grid", "max-w-7xl", "gap-10", "md:grid-cols-[minmax(0,65ch)_minmax(280px,0.5fr)]", "items-start", "lg:gap-16"], "children": [
               {"key": "article-content", "type": "container", "classes": ["prose", "max-w-[65ch]", "text-lg", "leading-relaxed", "text-foreground", "md:text-xl"], "children": [
                 {"key": "article-p1", "type": "text", "tag": "p", "text": "A random launch usually starts with a reasonable question: should you write more emails, improve the sales Page, add a webinar, or rebuild Checkout? The options all sound productive, which is why they are hard to prioritize.", "children": []},
                 {"key": "article-h2-1", "type": "heading", "tag": "h2", "text": "Start where the buyer gets stuck", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                 {"key": "article-p2", "type": "text", "tag": "p", "text": "If people understand the promise but do not buy, the problem is probably the Offer or Checkout. If they do not understand why the course matters, the sales Page needs clearer proof before you touch payment.", "children": []},
                 {"key": "article-quote", "type": "quote", "text": "Build the next Funnel Step at the exact point where buyer commitment drops.", "classes": ["rounded-2xl", "bg-muted", "p-6", "text-xl", "font-bold", "leading-snug", "tracking-tight", "text-foreground", "md:text-2xl"], "children": []},
                 {"key": "article-h2-2", "type": "heading", "tag": "h2", "text": "Use one diagnostic before adding a new asset", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground", "md:text-4xl"], "children": []},
                 {"key": "article-list", "type": "list", "classes": ["grid", "gap-3"], "children": [
                   {"key": "article-li-1", "type": "list-item", "text": "No opt-ins: sharpen the promise and lead magnet.", "children": []},
                   {"key": "article-li-2", "type": "list-item", "text": "No Checkout starts: improve proof and Offer clarity.", "children": []},
                   {"key": "article-li-3", "type": "list-item", "text": "Checkout starts but no Orders: remove payment uncertainty.", "children": []}
                 ]}
               ]},
               {"key": "article-cta", "type": "container", "classes": ["rounded-2xl", "bg-card", "p-6", "shadow-elevated", "md:sticky", "md:top-6", "lg:p-8"], "children": [
                 {"key": "article-cta-heading", "type": "heading", "tag": "h2", "text": "Choose your next step", "classes": ["text-3xl", "font-black", "tracking-tight", "text-foreground"], "children": []},
                 {"key": "article-cta-copy", "type": "text", "tag": "p", "text": "Get the launch map and identify which Funnel Step has the most leverage right now.", "classes": ["mt-3", "text-base", "leading-relaxed", "text-foreground", "md:text-lg"], "children": []},
                 {"key": "article-cta-button", "type": "button", "tag": "a", "attributes": {"href": "/"}, "text": "Get the launch map", "classes": ["mt-6", "inline-flex", "h-14", "w-full", "items-center", "justify-center", "rounded-2xl", "bg-primary", "px-6", "text-base", "font-bold", "text-primary-foreground", "shadow-elevated", "md:text-lg"], "children": []}
               ]}
             ]}
           ]
         }
       ]
     }
   }
   ```

4. Render through Dent and inspect the screenshots.

   - Confirm the header feels designed and distinct from a bare title.
   - Confirm the body column is readable at desktop and mobile widths.
   - Confirm there is one contextual CTA.
   - Confirm the starter still passes the bans inherited from `designing.md`.
