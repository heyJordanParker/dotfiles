# Tiered validation: agents prove their own diff, expensive gates run once

An orchestrated changeset had every agent re-run full e2e categories: 50 browser processes
at peak and over 70% of agent wall-clock spent on validation. Chosen: each agent proves only
its own diff with the narrowest run that reaches it, while the full suite, categories, and
browser walks run once as the Orchestrator's end gate, whose artifacts everyone else cites.
Cost accepted: a cross-agent regression surfaces at the end gate instead of mid-flight.
