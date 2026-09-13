# Runtime Swarm Dispatch After R114

R114 locally accepts [pristine control cleanup and terminal parent transport](runtime-pristine-control-cleanup-v1.md).
The [254-artifact evidence packet](evidence/local-r114-pristine-control-cleanup-2026-09-13/README.md)
passes independent review. This supersedes the [candidate dispatch](runtime-swarm-dispatch-r114-candidate.md),
not the retained contracts and history in the [complete packet map](runtime-swarm-next-packets.md).
Issue [#182](https://github.com/harsh-nod/fe2o3/issues/182) remains open;
A1/A2 and full HIP/HSA parity are not complete.

## Accepted Boundary

GNU and musl each pass 2,712 tests with five ignored across 48 libtest harnesses
and the unchanged harnessless CSV benchmark. All 17 source gates, ten auxiliary
gates, frozen/restored 37/7/9 focused rosters and 15 compiled behavioral negatives
pass. Nine runner, 30 freeze and 80 qualification-contract tests pass. All 5,683
source identities are restored, and the closed collector and both independent
archive reviews pass.

The full GNU/musl runs are separately recorded pre-freeze prerequisites with
matching source endpoints, not executions inside the later fifteen-gate batch.
Preliminary failures, formatter changes and helper-review history are retained.
An earlier unwrapped formatter failure lacks saved inventories and is not
reconstructed as evidence. No live GPU, authenticated proof or performance
acceptance is added. Ordinary/returning cleanup and data cleanup remain open.

## Assigned Lanes

These three read-only workers completed their reviews. Their following queues
are assigned work, not unattended running jobs. Primary owns edits, integration,
serialized builds, proof/hardware scheduling and signed dual-remote publication.

| Owner | Immediate Packet | Following Queue |
| --- | --- | --- |
| Native: `native_replacement_handoff` | N4-R2 returning-control cleanup, reusing the accepted R114 lower adapter | Persistent/ordinary bridges, data cleanup, applicable live/queue teardown, then N5 DATA-ADOPT |
| Admission: `submission_identity_handoff` | Qualify and integrate isolated C1; C2/C3 coverage can advance independently | Joint I2 ISSUE, C4 COMPLETE, C5 typed future/join, C6 generated GRAPH/DRAIN |
| Resources: `r102_evidence_review` | Freeze V3 settlement and implement its model/cost tests | V4 proofs, V5/V6 production journal/hooks, V7 writers/recovery, V8 leases; M1-M4 retained-memory bounds |
| Primary | Integrate one reviewed packet at a time; own Q1/Q2/Q3 and release gate #277 | Coordinate A3-A7 and publish only the evidence actually obtained |

## Native Packets

1. **N4-R2 returning controls:** Start with `release_non_data_after_recycle`,
   `release_non_data_for_returning_destroy` and shared `release_non_data` in
   `crates/fe2o3-kfd/src/queue_dispatch_binding.rs`. Root the entire owner before
   validation; reserve returned-data capacity before disposal. Reuse
   `ControlCleanupCustodyV1`, but preserve kernarg-first, **forward code order**.
   R114's pristine orchestration deliberately uses reverse code order.
2. **Persistent/ordinary bridges and data cleanup:** Preserve each API's exact
   returned-data-on-error behavior. Cover every supported host/device and
   initialized/uninitialized variant, retained charges, untouched suffix and
   completed prefix. No repeated disposal or duplicate refund.
3. **N4-L/N4-Q:** Retain original parents and taken lanes across model retake,
   commit and every teardown prefix. A failed disposal cannot expose a reusable
   allocation hole or queue slot. Each route consumes its applicable lower
   cleanup contract; not every teardown mode blocks the first N5 route.
4. **N5 DATA-ADOPT:** Join the required insertion/cleanup paths into actual
   nonpublishing generated resource adoption, with identity and Stop/drain
   custody. Do not replace the existing memory or queue engines.

R2's decisive tests cover first/middle/last controls, unmap/free/VA-release and
currentness failures, model-commit rejection, incomplete callbacks, one-shot
retry rejection, exact returned data and pre-effect validation/capacity failure.

## Admission Packets

| Packet | Implementation Boundary | Required Exit |
| --- | --- | --- |
| C1 | Isolated `context/tests/submission_identity_tests.rs`, narrow Context test wiring/cancel counter | Qualify 80 rejection cells, genuine cache/backend-ID reuse, destroyed streams and precedence without replacing validators. The nine focused and 724 runtime-library passes remain preliminary until integration and full immutable packet qualification. |
| C2-A | New `authorized_execution/tests/generated_identity.rs`; existing roster matchers | Every descriptor coordinate in both directions and against its source; valid controls pass and substituted identities reject. |
| C2-B | Same module; existing generated-storage transfer fixtures | Actual one-shot transfer, immutable source validation, then artifact/authority/currentness substitutions with exact buffer custody. |
| C3-A | New `async_engine/tests/owned_tests/preparation_tests/completion_tests.rs`; existing reservation/adoption fixtures | Two-owner isolation, Stop before disposal, A-success/B-failure/C-retained retirement, exact holds/credits and no retry or second reply. |
| C3-B | Same module; existing completion cell and identifiable wakers | Only the latest correct waker fires; panicking wake cannot lose custody or settle twice. Use a test-only borrowed accessor, not a cloned consumer. |

Paths in this table are relative to `crates/fe2o3-runtime/src`. C2/C3's new files
remain absent; their subpackets are independently reviewable but shared-file
edits need one integrating owner. Do not introduce a dummy native completion
adapter. Existing ordinary async, graph/drain and decoder machinery is extended.

## Resources Packets

Freeze the [V3 settlement draft](runtime-context-version-settlement-v1.md), then
implement Success, NoEffect and sticky Unknown. Validate the complete touched
chain before mutation; preserve burned epochs and disjoint writers. Reuse the
seven vectors with O(k) touched work, no commit-time growth and no unrelated
arena scans. Require independent reference traces, full test auditors,
rejection snapshots, replay/corruption tests, MAX exhaustion and fixed-k costs.

V4 proof work can begin from accepted V1/V2. M1 aggregate-domain design, M2
native cost inventory and M3 host-image limits are also independent preparation.
V5/V6 join the production journal and mutation hooks; V7 covers all writers and
explicit Unknown recovery; V8 adds exact input leases. M4 closes total retained
memory only after concrete owner, journal, lease and cache accounting is joined.
Model evidence does not authenticate a production completion or NoEffect receipt.

## Joins And Qualification

- Required N3/N4 paths feed N5. N5 plus C1/C2 and preissue C3 feed I2 ISSUE;
  freeze its mutation-hook policy first. C4 COMPLETE enables typed output and
  generated GRAPH/DRAIN. C6 requires ISSUE/COMPLETE, not semantically C5.
- A first non-reusing ISSUE does not wait for V7/V8. Journal-enabled execution
  needs V5/V6; cross-run reuse needs V7/V8 and exclusive graph reservation.
  Kernel reuse additionally needs admitted compiler effects.
- Q1 authenticates named proofs and actual Rust/model correspondence. Q2 adds
  native depth, pressure, teardown, bounded-memory and device-overlap evidence.
  Q3 fixes matched KFD/HIP/HSA workloads and complete-output checks before tuning.
  CPU test duration is not a runtime speedup measurement.
- Rotate the same lanes through A3 local multi-GPU, A4 authenticated two-host
  control, A5 distributed transfers/collectives, A6 fault qualification and A7
  performance/production closure. Copy-only XGMI does not close those milestones.

Concrete Worker V3/capsule and compiler semantic-to-machine authority remain
external production gates. Fixtures cannot supply their backend or owned proof
artifacts. Primary separately owns [#277's release audit/CI gap](https://github.com/harsh-nod/fe2o3/issues/277).
MI300X work stays serialized in task-owned staging with cleanup; disruptive
faults need an isolated window on the shared machine. This packet launched no
SSH, GPU or solver jobs.
