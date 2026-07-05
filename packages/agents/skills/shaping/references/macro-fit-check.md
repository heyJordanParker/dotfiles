# Shaping macro fit check

Use this Reference only when the Architect explicitly asks for a macro fit check while Shaping is still high level, requirements are chunked, and most mechanisms are still unknown.

## 1. Confirm the condition

IF the Architect did not ask for a macro fit check:
### Use the normal fit check instead
The macro fit check is for high-level chunked requirements with mostly unknown mechanics.

## 2. Build the Addressed and Answered table

### Each shape gets two checks
Addressed means some part of the shape speaks to the requirement at a high level. Answered means the concrete HOW is traced and spelled out.

Template:
  ```markdown
  ## Macro Fit Check: R × A

  | Req | Requirement | Addressed? | Answered? |
  |-----|-------------|:----------:|:---------:|
  | R0 | Core Goal description | ✅ | ❌ |
  | R1 | Guided Process | ✅ | ❌ |
  | R2 | Agent boundary | ⚠️ | ❌ |
  ```

### Only show top-level requirements
Use `R0`, `R1`, `R2`. Do not include sub-requirements in the macro table.

### Keep the table narrow
Do not add a notes column.

## 3. Mark the cells

### Addressed uses three states
Use ✅ for yes, ⚠️ for partial, and ❌ for no.

### Answered is binary
Use ✅ when the concrete HOW is traced. Use ❌ when it is not.

## 4. List the gaps

### Gaps name missing parts
After the macro table, add a separate Gaps table with the missing part and its related sub-requirements.

Template:
  ```markdown
  | Gap | Related R | Missing part |
  |-----|-----------|--------------|
  | G1 | R2.1, R2.2 | Agent boundary mechanics are not traced |
  ```
