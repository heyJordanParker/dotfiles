# Hierarchy & Placement Principles

**How Claude.md Files Work:**

Claude.md files are hierarchical. When opening any file, Claude automatically reads every Claude.md in the file tree from root to that file's directory. This means:
- Parent context flows down automatically
- Don't repeat parent content in child files
- Each file only needs to add what's specific to its level

**Core Principles:**

- **Push context as deep as possible while keeping it discoverable.** Root stays navigable, details live where they're needed.
- **Write minimum documentation that provides full context.** AI context is limited and precious. Too little → agents can't complete tasks. Too much → agents overflow and forget critical details. Every line must earn its place. No fluff.

**What Goes Where:**

- **Project root:** Vision, principles, architecture overview, dev commands, navigation guide to the codebase
- **Feature/module/namespace directories:** What this area does, how it's organized, key patterns for working here, rules specific to this area
- **Code-heavy directories:** Tactical docs with code examples, function signatures, usage patterns
- **Organizational directories:** Structural docs explaining what's inside and how subdirectories relate

**The deciding factor is what's in the directory:**
- Mostly code files → include concrete examples and API patterns
- Mostly subdirectories → explain the structure and relationships

**Each Claude.md answers:** "What do I need to know to work effectively in this directory?"

**Signals to Split:**

- A section grows beyond ~10 bullet points
- Content primarily serves a subdirectory's developers
- Adding implementation details to a strategic document

**Signals to Create New Claude.md:**

- Working in a directory with established patterns but no Claude.md
- A parent file is covering concerns that belong deeper
- Developers in that directory would benefit from local context

**Placement Decision Process:**

1. Identify scope of what's being documented
2. Find closest existing Claude.md that matches scope
3. If no good match, consider creating new file at appropriate level
4. Propose placement with reasoning, wait for approval
