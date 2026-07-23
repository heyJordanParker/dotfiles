---
description: Establish what to build by interviewing the operator in plain language and deriving the shared writing and design brief.
---

# Interview the operator

Use this before writing copy or authoring a Dent Designer tree for a Page, Article, or Funnel Step. The failure it prevents: asking the operator to think like a copywriter or designer, then building from jargon instead of their real business.

## Process

1. Start with the operator's normal words.

   - Ask short, plain questions in rounds. Two or three questions per round is enough.
   - Ask about the business, the customer, what they sell, what makes it different, proof they have, who it is for, and the feeling the Page or Funnel Step should create.
   - Do not ask the operator to choose strategy labels. They describe the business; you derive the labels.
   - Use strict Dent words when naming surfaces: Funnel, Funnel Step, Offer, Page, Article, Checkout, Product, Course, Space, Link.

   ### Ask for the job in business language
   Find the one thing this surface must make happen.
   Example: "What should this Page get someone to do? Join the list, buy the Offer, finish Checkout, accept an upgrade, read an Article, or something else?"
   Never: "Which conversion architecture should this asset use?"

   ### Ask for the customer in lived language
   Get the role, situation, pain, desire, and words the customer would use.
   Example: "Who is this for, and what are they frustrated by right before they land here?"
   Never: "Define the persona segment and psychographic profile."

   ### Ask for the sale in concrete terms
   Capture what is being sold or promised, the price or trade, and the reason it is a good deal.
   Example: "What are they getting, what does it cost, and why is it worth acting on now?"
   Never: "Describe the value equation."

   ### Ask for difference and proof without inventing either
   Capture the named alternative, the edge, and every proof point the operator can honestly support.
   Example: "What would they use instead, and what real evidence do you have that this works? Numbers, screenshots, testimonials, demos, founder experience, or customer stories all count."
   Never: invent a metric, testimonial, deadline, customer name, guarantee, or scarcity claim to make the Page stronger.

   ### Ask for feeling and references in normal words
   Let the operator describe taste, mood, and examples without design vocabulary.
   Example: "How should this feel when someone lands on it? Name any sites, brands, objects, places, or screenshots that feel close or wrong."
   Never: ask the operator to choose an internal design label.

2. Derive the strategy inputs after the interview.

   - Audience comes from the customer description, not a generic persona template.
   - Promise comes from the before-to-after outcome the customer would repeat.
   - Offer comes from the Product, price, terms, risk reversal, urgency, and included value.
   - Proof points come only from evidence the operator supplied or from Dent records you actually read.
   - Reader awareness comes from what the visitor likely already knows when they arrive.
   - Market sophistication comes from how familiar or tired the claim is in the customer's market.
   - Register comes from the surface job: brand-led for marketing impression and product-trust for Checkout, account, admin, or task completion.
   - Mood and visual direction come from the operator's plain words, references, and the finished business argument.

   IF the answers do not support a claim:
   ### Mark the gap instead of filling it
   Write the claim as a proof gap in the brief and keep it out of ship-ready copy until evidence exists.

3. Write the shared brief.

   The brief is the contract consumed by `writing/copywriting.md`, `design/designing.md`, and the page-type leaves. Keep it compact. Use these exact field names:

   - `surface`: Dent surface being authored, such as Page, Article, Funnel Step, Checkout Funnel Step, or Upsell Funnel Step.
   - `page_job`: the one action or understanding this surface must produce.
   - `audience`: the specific customer, buyer, reader, or Contact this is for.
   - `customer_situation`: what is happening in their world before they arrive.
   - `customer_language`: phrases, pains, wants, objections, and labels in the operator's or customer's own words.
   - `promise`: the single before-to-after outcome the page will argue for.
   - `offer`: the Product, Offer, lead magnet, access, terms, price, risk reversal, and real reason to act.
   - `differentiation`: the named alternative and the one axis where this wins.
   - `proof_points`: real evidence available to support claims.
   - `proof_gaps`: claims the operator wants but cannot yet prove.
   - `objections`: doubts the visitor is likely to carry into the Page or Funnel Step.
   - `reader_awareness_stage`: derived stage of what the visitor likely already knows.
   - `market_sophistication`: derived level of how worn-out the main claim or mechanism is.
   - `register`: derived design trust mode, either `brand-led` or `product-trust`.
   - `mood`: the plain-language feeling the surface should create.
   - `visual_direction`: one intentional direction using the tenant theme, references, imagery needs, density, and layout posture.
   - `constraints`: required Dent entities, links, fields, forms, Checkout controls, assets, legal text, or technical limits.
   - `open_questions`: only the unresolved answers needed before writing or design can proceed.

   Template:
     `surface`: Funnel Step
     `page_job`: get problem-aware visitors to opt in for the checklist
     `audience`: solo course operator with a small list and no working funnel
     `customer_situation`: they have content people like but no predictable sales path
     `customer_language`: "I don't know what to build next", "my launch feels random"
     `promise`: know the next Funnel Step to build and why it matters
     `offer`: free checklist delivered by opt-in form, no paid Offer on this step
     `differentiation`: replaces generic launch advice with a Dent-specific build order
     `proof_points`: operator has shipped three funnels and can show a live demo
     `proof_gaps`: no customer conversion numbers yet
     `objections`: unsure this applies to their niche; worried setup is too technical
     `reader_awareness_stage`: problem-aware
     `market_sophistication`: level 3
     `register`: brand-led
     `mood`: calm, practical, not hypey
     `visual_direction`: quiet editorial structure, strong CTA, theme primary used only for action and proof accents
     `constraints`: email and first name fields; optin behavior must reference matching `fields.*` names
     `open_questions`: none

4. Stop when the brief is enough to write.

   - Do not write the Page copy in the interview reference.
   - Do not author the Designer JSON in the interview reference.
   - Hand the brief to the writing process first. Design consumes the finished copy after writing.
