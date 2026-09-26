# Domain Retirement Development

Base: signed `52ebcff8b8a8767e5a797f663233d9fb9ea062ba`.
Review found that fallible node reaping followed credit refund and record reuse,
allowing corruption detected late in reaping to leave partial changes before
poison. This prerequisite is fixed before extending the hierarchy's formal
claims. It is not full runtime or HIP/HSA acceptance.

Owned local target:
`/home/harsh/.codex-tmp/fe2o3-domain-retirement-target-20260926-wjvhhFK2`.

## Reproduction And Fix

Create a root/parent/leaf chain with one retained charge, then drop all account
handles while retaining only the coordinator Arc and credit. Fault-inject
`parent.used = charge + 1`. Old release preflight succeeds, then refunds every
ancestor, recycles the record and removes the leaf before parent retirement
detects residual usage. The result is error/poison after partial mutation. Final
account Drop has the analogous partial-node-recycle failure.

`raw/reproduction.log` records both new regressions failing against the base
production code: zero passed, two failed, exit 101. The oracle compares every
node's key, parent, limits, usage, phase counts, handles and children, every exact
record, ordered free lists and generation counters. The expanded oracle also
covers free-list capacities, profile depth and scratch. Only poison and the
retention anchor may change after rejected retirement. This is fault-injected
internal corruption, not demonstrated corruption reachable through public APIs.

The fix materializes the exact selected path before mutation and stages the
removable prefix in a fixed four-entry local plan. It projects the pending leaf
handle decrement or per-ancestor refund/count change, validates zero usage on
retired nodes, parent-child decrements, and logical/physical free-list capacity.
Refund, record reuse and node cleanup commit only after the complete plan passes.
Node cleanup is no longer a fallible operation after the first mutation.

Drop now also validates ancestry above a live survivor; this deliberately
strengthens corrupt-state handling without changing valid-state behavior. The
selected path contains at most four distinct nodes, with no new heap allocation.
No measured speedup is claimed. This validates selected ancestry and retirement
fields, not unrelated arena slots, global backlink/record correspondence or
native disposal authority.

## Coverage

- Late residual usage, zero root child count and free-node exhaustion after an
  otherwise valid removable prefix, through both disposal and account Drop.
- Actual node/record Vec push-capacity exhaustion before mutation.
- Over-depth/invalid profile, self/descendant cycles and stale root generation
  above a live survivor, with unchanged state even when later handles drop.
- Depths one through four: retain, cancel-unissued, confirmed/rejected release,
  quarantine, full cascades, root bootstrap preservation and exact free-list order.
- Stop conditions for live ancestor handles, sibling children and remaining
  records; eventual clean final retirement.

The initial fix passed all 54 accounting tests; expanded coverage passes all 57.
Both logs are retained. Independent read-only reviews checked the projected
prefix and infallible commit. The final three-package validation uses
[`validate.sh`](validate.sh), locked/offline dependencies, debug information and
incremental compilation disabled, and two test threads. It preserves actual
per-group exits and does not turn the known socket failures into passes.

## Final Validation

| Check | Result | Raw log |
| --- | --- | --- |
| All-feature libraries, three packages | Accounting: 57 passed. KFD: 1,321 passed, one socket-admission failure, 296 construction tests filtered. Runtime: 1,476 passed, three socket-inspection failures, 28 ignored. Exit 101 | `final-libraries.log` |
| Retirement/class/compound/rooted/XGMI focused selection | 54 passed: 29 KFD, 12 accounting, 13 runtime. Exit 0 | `final-focused.log` |
| Selected construction/custody regressions, same three-package feature unification | 69 passed: 67 KFD, two runtime. Exit 0 | `final-construction.log` |
| Strict Clippy, all features and targets, three packages | Passed; exit 0 | `final-clippy.log` |
| All-feature doctests, three packages | 77 passed: 28 KFD, three accounting, 46 runtime. Exit 0 | `final-doctests.log` |
| No-default-feature checking, three packages | Passed; exit 0 | `final-no-default.log` |
| Workspace formatting check | Passed; exit 0 | `final-fmt.log` |

These selections overlap and are not additive coverage. The four broad failures
are the previously recorded KFD
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `SocketAdmission`, and runtime
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
at `InspectSocket(EPERM)`. Production socket validation is unchanged. This is
not full CPU qualification. Every validation command returned; the driver
exited 1 because the library group failed. `raw/final-status.tsv` preserves
every group exit.

`raw/final-sources.sha256` freezes the three package trees and workspace
manifests before final validation; its checksum recheck passes. The three
library/test ELF hashes and their passing recheck are retained separately.
All three test selections use those same ELFs. These local identity receipts
are not a complete signed replay closure, formal refinement or native evidence.

## Access And Cleanup

`raw/mi300x-access.log` records hostname resolution failure before reaching the
shared host. Both GitHub remote observations also failed at hostname resolution,
as recorded in `origin-access.log` and `upstream-access.log`. No remote resource
was created, and no GPU result or performance improvement is claimed.

After all validation commands returned and source/binary checksum checks
passed, the exact owned local target was removed with `rm -r`.
`raw/cleanup-before.log` records 613,184 KiB of allocated storage. The independent
absence log records ENOENT, and `raw/cleanup-check.log` records `test ! -e`
returning 0. No unrelated directory was removed.

## Open Work

This fixes a production prerequisite; it does not add a detached model theorem
or claim new formal acceptance. Shared-body all-ancestor reservation, transition
and retirement proofs still need exact production correspondence, authenticated
negative mutations, path/key/arena and lock refinement, and native qualification.
Logical/native accounting composition, complete bootstrap/resource closure,
broader runtime behavior and matched HIP/HSA performance also remain open.
