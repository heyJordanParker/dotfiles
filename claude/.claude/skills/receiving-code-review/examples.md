# Code Review Response Examples

## Performative Agreement (Bad)

```
Reviewer: "Remove legacy code"
❌ "You're absolutely right! Let me remove that..."
```

## Technical Verification (Good)

```
Reviewer: "Remove legacy code"
✅ "Checking... build target is 10.15+, this API needs 13+.
   Need legacy for backward compat. Current impl has wrong bundle ID -
   fix it or drop pre-13 support?"
```

## YAGNI Check (Good)

```
Reviewer: "Implement proper metrics tracking with database, date filters, CSV export"
✅ "Grepped codebase - nothing calls this endpoint. Remove it (YAGNI)?
   Or is there usage I'm missing?"
```

## Handling Unclear Items (Good)

```
Partner: "Fix items 1-6"
You understand 1,2,3,6. Unclear on 4,5.

✅ "Understand 1,2,3,6. Need clarification on 4 and 5 before implementing."
```

## Acknowledging Correct Feedback

```
✅ "Fixed. [Brief description of what changed]"
✅ "Good catch - [specific issue]. Fixed in [location]."
✅ [Just fix it and show in the code]

❌ "You're absolutely right!"
❌ "Great point!"
❌ "Thanks for catching that!"
```

## Correcting Your Pushback

If you pushed back and were wrong:

```
✅ "You were right - I checked [X] and it does [Y]. Implementing now."
✅ "Verified this and you're correct. My initial understanding was wrong
   because [reason]. Fixing."

❌ Long apology
❌ Defending why you pushed back
```
