# Regressions

Gate for catching breaking changes before they reach callers.

**Core principle:** Every change to a public interface has callers. Verify they still work.

## The Gate

Before commit, check every modified function/export/type against its callers.

### 1. Changed Function Signatures

- **Added required parameters** – Callers don't pass them
- **Removed parameters** – Callers still passing them (may be silently ignored)
- **Changed parameter types** – Callers passing old type
- **Changed parameter order** – Callers using positional args

**Check:** Find all call sites. Verify they match the new signature.

### 2. Deleted Exports/Functions

- **Removed export** – Other files import it
- **Removed function** – Other code calls it
- **Removed type/interface** – Other code references it
- **Renamed without updating callers** – Old name still used

**Check:** Search for all references before deleting. Update or remove every one.

### 3. Modified Contracts

- **Changed return type** – Callers expect old shape
- **Changed error behavior** – Callers catch specific errors
- **Changed side effects** – Callers depend on side effects (writes, events, state)
- **Changed async behavior** – Sync → async or vice versa
- **Changed null behavior** – Now returns null where it didn't, or vice versa

**Check:** Read each caller. Verify they handle the new contract.

### 4. Changed Defaults

- **New default values** – Existing callers relied on old defaults
- **Changed config defaults** – Deployed systems expect old behavior
- **Changed environment assumptions** – Code assumes new env vars or paths

**Check:** Identify callers that don't pass explicit values. Verify old behavior preserved.

## Process

1. **Get the diff** – `git diff HEAD`
2. **List changed interfaces** – Every modified function signature, export, type, return value
3. **Find callers** – For each changed interface, search for all references
4. **Read pre-change code** – `git show HEAD:<path>` to see what callers expected
5. **Verify compatibility** – Each caller still works with the new interface
6. **Report breaks** – List every caller that needs updating

## Red Flags

- Renamed function without searching for references
- Changed return type without checking callers
- Deleted export without verifying no imports
- Added required parameter to public function
- Changed error type thrown/returned

## Output

For each regression found:
- **File:line** of the breaking change
- **What changed** – Old → New
- **Affected callers** – List of files:lines that break
- **Fix** – Update caller or preserve old behavior
