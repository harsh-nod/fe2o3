# Retained Pair Routing

## Shared Controller

Integrated at `e5b15e142` from signed candidate `67c2f4c1d`.

The retained Context facade uses the same `BatchPhase` and `Advanced`
declarations and `publish_once`/`advance` bodies as its Verus adapter.
The generic request, ticket, completion and error payloads need not implement
`Copy` or `Clone`. Moving through the controller preserves their identity.

For a prepared batch, the controller submits the original requests once and
waits only after successful submission. A pending batch waits on its existing
tickets without resubmitting. Ready and terminal states invoke neither callback.
Waiting receives the original deadline, and the controller forwards the exact
callback result. The production adapter and callback implementation are unchanged.

An 80-case CPU matrix covers five phases, four payload lengths and both outcomes
of each callback. It checks callback counts, allocation identity and forwarding.
The existing repeated-pending test separately checks one publication across five
waits. These are controller tests, not native device execution.

## Proof Boundary

The theorem is conditional on normal callback return. The proof adapter consumes
one result for each reached callback and records its arguments; it does not
assert callback success or model an uncalled callback as an observed result.
An opaque local owner value is preserved, which does not prove that an `Arc`
interior or native resource is unchanged. Deadline identity is not a clock or
timeout guarantee.

Native submission and waiting, retry eligibility, device currentness, panic and
abort behavior, automatic `Drop`, and machine-code correspondence remain outside
this proof. The [retained facade](runtime-retained-xgmi-pair-v1.md) retains its
separate custody and explicit-finish requirements.

## Qualification

The signed campaign passes three full `--no-cheating` runs, each reporting four
verified obligations and zero errors, including an exact three-file relocation.
All eleven actual-body mutations fail logically under the strict classifier.
All nineteen campaign phases and two additional verifier-release stages pass;
their 21 fresh process groups close. The three positives overlap and are not
additive proof counts.

Earlier qualification passes 24 focused runtime CPU tests, including the
80-case matrix. All 6,464 selected inputs are bound to the signed candidate;
four proof/checker metadata changes leave the CPU-tested runtime sources and
retained executable unchanged. This is explicit pre-signing CPU-byte reuse,
not fresh execution on a signed checkout. The earlier rejected proof-discovery
attempts and diagnostic-only selector capture remain separate records.

Separate signed-candidate checks pass strict all-feature/all-target runtime
Clippy, warning-free no-default compilation, four-file rustfmt and scoped
whitespace checks, with five more fresh process-group closures. The merged
runtime and proof files match the candidate. Only the routing and older fold
source-tree guards additionally incorporate the independently qualified charge
wrapper; executable proof closures and expected counts remain unchanged.
Both lightweight calibrations and a full integration whitespace check pass.
There is no new merged-tree solver or CPU-suite claim.

Records remain local under
`/home/harsh/.codex-tmp/fe2o3-retained-routing-records-20260930-prefix`.
The signed campaign result is `signed-campaign-attempt-1/results.json`, SHA-256
`8c646a7df7da91c8abdcdda62512606a96bdaee7cbe5f46a9babda8017dddbf0`.
No complete merged-runtime suite, native execution, performance result or
milestone exit follows from this component qualification.
