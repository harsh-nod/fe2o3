# Native XGMI Backing Budgets

This integrates the existing session-local backing accounts into the exact
two-device, copy-only native XGMI backend. It does not add an allocator, aggregate
process accounting, unified multi-device compute, or a new proof of the adapter.

## Construction And Observation

`KfdNativeXgmiBackingBudgetV1` carries independent optional `device` and
`host_visible` limits. The additive `open_default_with_backing_budgets_v1` and
`from_checked_pair_with_backing_budgets_v1` constructors take two entries in
argument order, not sorted device or topology order. Each entry is forwarded to
its original session acquisition before allocation or queue certification.
Budgets cannot be changed after construction. Existing constructors delegate
with both entries unconfigured; they retain the existing default configuration.

The first session-acquisition error follows the existing return path. After the
first VM is acquired, a second acquisition error or panic aborts the process:
there is no inverse transition returning the consumed first device authority.
The panic case is now explicitly guarded, not allowed to unwind past that owner.

`backing_usage_v1` returns two inert snapshots in the same order, each with its
backend device ID and the original session's two usage observations. Inspection
does not issue native calls and remains available after terminal failure or
shutdown. `None` means unconfigured, not zero memory. Retained or quarantined
charges remain visible; a usage snapshot never grants cleanup authority.

## Accounting Scope

The device account charges page-padded PUBLIC device backing and allocation
records, not requested logical bytes. CPU or peer mappings do not create another
backing charge. Each allocation remains charged to its home session. Confirmed
disposal releases the charge; uncertain release retains it.

The host-visible account covers ordinary coherent GTT, including the completion
storage allocated in a directional queue's source session. The current queue
uses one 4-KiB completion allocation. AQL ring backing and userptr controls are
excluded, as are VM bootstrap, metadata, host staging temporaries, other sessions
and total process memory. These limits do not establish aggregate A1/A2 bounds.

## Failure Classification

The lower classified allocation entrypoint calls the existing allocator with a
real per-call native-attempt marker. It returns `RejectedCapacity` only on normal
error return, before native allocation, with an Active session, and with either
the exact backing `Capacity` error or a fixed profile byte/record ceiling error.
Other credit errors, invalid authority, overflow, malformed native results and
currentness failures supply no retry authority. In particular, currentness can
fail before the native marker is set; the session's quarantine prevents even a
capacity-shaped currentness error from being reclassified as rejection.

The runtime maps only this typed disposition to `Rejected/Capacity`. Existing
Context rejection handling returns the requested-byte and allocation-record
credits. The lower backing reservation is also absent. Runtime table capacity
may have grown and its candidate handle ID is deliberately burned; this is a
no-native-effect rejection, not a promise of unchanged host bookkeeping.

Fixed-profile pressure is now recoverable even for unconfigured budgets, instead
of the prior catch-all terminal result. Native errors and uncertainty remain
terminal. Allocation and diagnostic panics latch terminal state and preserve
the original panic payload. No allocation is automatically retried.

This classification is not applied to queue creation. Completion-buffer pressure
can occur after ring/control allocation, so an entire queue constructor cannot
claim no native effects. Existing queue-creation roots and terminal handling
remain responsible for all such errors and panics.

## Qualification Boundary

The [CPU packet](evidence/dev-xgmi-backing-budgets-cpu-2026-09-24/README.md)
passes all 1,552 KFD and 1,365 runtime library tests on each GNU/musl target,
with twenty hardware-only runtime ignores, plus 73 doctests and static checks.
All sixteen new regressions pass. The complete four-thread campaign uses
prospectively extended whole-command bounds after two retained timeout attempts;
it does not qualify default test-harness concurrency or measure runtime speed.

CPU tests use the original allocation/accounting/mapping driver with scripted
native leaves and the production-shared runtime acquisition/settlement helpers.
They do not construct fake checked devices or native sessions. Source-wiring
checks are distinct from native constructor execution. The fresh signed
[MI300X budget campaign](evidence/dev-xgmi-backing-budget-native-mi300x-2026-09-24/README.md)
passes asymmetric pressure, release/retry, retained-mapping accounting, both copy
directions and ordinary teardown. Historical diamond results are not reused to
qualify this change, and native fault injection remains unqualified.
Formal correspondence, aggregate memory, full A1/A2 acceptance and matched
HIP/HSA performance remain open.

## Native Witness

The [witness and sibling campaign](runtime-xgmi-backing-budget-witness-v1.md)
are CPU- and native-qualified for the exact workload below, not broader faults
or memory/performance claims.

Use a new signed-source copy-only smoke example and campaign, without editing
the historical diamond witness. A direct journaled Context permits inspecting
usage both before and after logical shutdown and native queue retirement.

| Endpoint argument | Device backing bytes / records | Coherent backing bytes / records |
| --- | --- | --- |
| First | 8192 / 3 | 4096 / 1 |
| Second | 24576 / 2 | 8192 / 2 |

Allocate one logical 4097-byte buffer on the first endpoint and two on the
second, each charging 8192 padded bytes. With separate loose Context limits,
an extra one-byte request must reach native admission and reject on bytes at
the first endpoint and records at the second. Require exact Capacity, unchanged
native usage and Context requested credits, a live Context and intact contents.
Release one allocation per endpoint and retry with fresh identities.

Then validate complete forward/reverse copy payloads and guard bytes. Device
backing charges must not change during mappings or execution. Directional queue
creation should move coherent usage from zero to 4096/0, then 4096/4096. After
explicit resource release and native shutdown, require zero, unpoisoned accounts.
Native failed-release and queue-pressure injection are outside this smoke;
their CPU custody tests are not hardware fault qualification.

Reuse the existing strict campaign protocol through budget-named siblings:
fresh availability at all six endpoints, exact signed source/ELF and bounded
commands, settled postflight even on failure, independent result checks,
byte-exact collection, owned-marker cleanup and separate absence checks. Run
from an owned clean detached checkout, leaving unrelated work untouched.
