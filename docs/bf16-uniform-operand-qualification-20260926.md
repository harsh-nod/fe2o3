# Uniform operand preparation qualification — 2026-09-26

This checkpoint adds a private, resource-metered uniform operand component while
preserving the ordinary helper's source decisions. It is not a complete argument
producer, an admitted bounds factory, or a new public kernel-authoring API.

## Implemented component

The existing helper keeps its signature and delegates to a shared decision body.
Its ordinary mode retains the original constant-first branch, allocation policy,
SSA allocation, origin/slot lookup, checked argument-counter update and errors.
The paid mode uses the existing original resource adapter and caller-owned
argument slots, counter, operation vector and SSA allocator. It never constructs
replacement producer state, returns an operation owner, refunds credits or issues
a completed-phase token.

The original helper was retained independently as the test oracle. Source checks
reverse three parent transformations and nine shared-body transformations to the
exact original source. Nine unchanged decision/allocation helpers are separately
bound. The actual parent was composed by anchors, preserving the already qualified
bounds components; it was not replaced with the older donor parent.

## Mutation and refusal order

Constant lookup precedes projection inspection. After a constant miss, the reached
projection scan is prepaid before the original place helpers execute. Conversion,
counter advancement and slot installation are prepaid together.

If a left operand creates an argument or emits a constant and the right operand
later refuses, those earlier mutations remain. A successful vector reservation
also stays in the caller-owned vector if SSA allocation then fails. The enclosing
caller must retain physical state and accepted credits through its postflight,
then drop payloads before refund. These tests exercise synthetic retained state;
they do not establish authentic whole-source producer custody.

## Accounting and independent review

The per-invocation logical header uses twelve grouped, checked source-derived
rows. Independent review found that the initial constant-lookup row combined two
nested helper calls without distinct return envelopes. The correction explicitly
accounts for both callers and the borrowed scalar/value views, with an independent
typed row-formula control. Both the initial proposal and correction are retained;
no unrelated header slack is used to justify the correction.

Work and storage are checked separately. Controls cover one-short header and
projection budgets, unmetered refusal, malformed origins/counters, cached and new
arguments, partial mutations, SSA refusal after reservation, and unwind cleanup.
These are logical selected-source resource checks, not native stack, allocator
internals, process RSS or GPU performance measurements.

The separate shared preparation-policy refactor is **not included**. Existing
PreparationResources allocation methods and their call graph remain unchanged.

## Qualification

- 13 new operand-component controls passed.
- Full regression: 331 model and 2,878 backend tests passed; 189 backend tests
  remain ignored. Backend build and 83 JavaScript controls passed.
- Both ordinary-source compilation ladders passed. All 38 normal observation
  bodies and 52 artifacts are unchanged from the bounds-component checkpoint.

Regression receipt:
`69fb48571ca1641f97f323df12a1eb7ca5958cc6a106255b64fe8d2eb4563441`.

Normal-ladder receipt: `e275c9e14409dc37424da489a885bf04a6c95482e30f04bcc0d696cd065a7eac`.
Normal comparison receipt: `b7df8a534a4702f22898ed39ebd19054955674d733de2db727ff292378e1769d`.

## Remaining work and authority

The source-ordered direct comparison stage, physically retained duplicate and
conflicting predicate candidates, induction/body/deterministic-switch/launch
writers, final extent-count rule, and authentic producer handoff remain unfinished.
Lazy joint proof/extent/operation ownership, complete operation streams and
mandatory verification/normal continuation are still required.

No target dispatch, stopped-wave capture, edited-source promotion, new nominal
LLVM continuation, public capture gate, global compiler pin or route-maturity
change is supplied. Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.
