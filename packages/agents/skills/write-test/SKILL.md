---
name: write-test
description: Write one test that states a capability the app has and goes red when the app stops doing it: real types in the starting state, the call the app makes, an expected value written by hand. TRIGGER before writing or editing a test. DO NOT TRIGGER to decide which tests a change needs; use /plan-tests.
---

# Write Test

- A test states one capability the app has, and fails when the app stops doing it.

## 1. Read what the test will call
Run `trace callers` on the code under test and `trace read` on one caller, to see the call the app makes. Read the test file beside it for the setup it already uses.

IF the code does not exist yet:
### Write the test against the call and expected result the change requires
That is the contract until the code exists.

IF the code is what is broken:
### Fix the code
A test is not a fix for a defect.

IF an existing test states the opposite:
### Treat that test as wrong
It records what the code did, not what the app must do.

## 2. Create the starting state
Build it from the real types, through the project's factories, seeds, or harness, in the shape production stores. Every part starts in its before state.

### Reuse the setup the neighbouring tests use
Never: a type invented for the test, a method added to production code for the test to call.

## 3. Make the call
Make the call the app makes, in this process.

### Exercise the whole path the app takes
Never: a private method, one step inside a mechanism, a subprocess where a direct call runs the same code.

## 4. Check the result
Write the expected value by hand.

### Check a value that changes when the app stops doing what the test says
Never: a value the code under test produced, a check weakened until it passes.

IF the expected result is a set the system enumerates:
### Check the rule over the real set
Never: a copy of that set kept by hand in the test.
