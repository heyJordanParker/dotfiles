---
name: plan-tests
description: List the tests a change needs before any code is written, one per capability the app must have once it lands, each with its expected result. TRIGGER from /tdd, or when the Architect asks which tests a change needs. DO NOT TRIGGER to write a test; use /write-test.
---

# Plan Tests

- A capability is one thing the app can do that a User or another system would notice losing.
- The suite states every capability the app has once the change lands, whether the code exists yet or not.

## 1. Write the capabilities the change adds
Write what the app must be able to do once the change lands. That is the list. Write one sentence each: "the User can X", "the system can X".

### Split a sentence in two when one half can break while the other still works
Write one sentence when the only difference is the input.

### Stop at what the app does
Never: a sentence about a step inside the app.

## 2. Add the capabilities the app already has here and must keep
Run `trace structure` on the files the change touches, then `trace callers` on what they expose. Every caller is using a capability. Write it as a sentence too.

IF the change removes a capability:
### Leave it out and mark its test for deletion

## 3. Give each sentence a starting state, a call, and an expected result
Write the expected result the app must produce.

IF the code does not exist yet:
### Write the call the change will make

IF the expected result is not settled anywhere:
### Stop and name it

IF a defect reached the User:
### Write the capability the suite never stated
Never: a test for the one reported input.

## 4. Compare the sentences with the tests already there
A sentence already stated the same way leaves that test alone. A sentence stated differently changes that test. A sentence no test states is a test to write. A test stating a capability the change removes is deleted.

### Cut a sentence whose test would not lower coverage if deleted
Never: a sentence describing a mistake an Agent made.

## 5. Reply with the tests to write, to change, and to delete
