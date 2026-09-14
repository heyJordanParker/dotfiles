---
name: tdd
description: Implement an approved change one capability at a time, each test written and watched failing before the code that makes it pass. TRIGGER when the Architect says "tdd" or hands over an approved change to implement with tests. DO NOT TRIGGER to write or fix a test with no implementation to follow; use /write-test.
---

# TDD

## 1. List the capabilities and the tests to write, per /plan-tests

## 2. Take one capability and write its test per /write-test

## 3. Run that one test and watch it fail

### Read the failure against the test's own check
It fails there, because the app does not do what the sentence says.

IF it passes:
### Remove the code that provides the capability, run the test again, then put the code back
The app already does it, so this is the only way to see that the test can fail.

IF it fails anywhere else:
### Fix the starting state and run it again

## 4. Write the code

### Write the least code that makes it pass
The expected value does not move.

## 5. Run that one test again

## 6. Repeat from step 2

## 7. Run the whole file once every capability's test passes
