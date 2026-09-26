# Composed Multi-Device Request Routing

Development evidence on parent `7b0e397ed94f9231f1a27412391d67260f454e70`.
This extends the ordinary multi-device router, not the separate native XGMI
backend. It does not close A1, A2, A3 or issue #182 and makes no HIP/HSA parity,
native-performance or production-refinement claim.

## Implementation

- Production and semantic-authority constructors accept keyed tuples of device
  ID, authority, device budget and session budget. Complete ID validation and
  checked/child/index storage reservation precede opening. All checked devices
  precede root session admission and any lazy VM/queue creation. Per-child
  initialization is not globally preallocated. A late failure releases unused
  sessions, not already registered canonical device parents.
- A persistent all-Legacy/all-Required policy and the children retain complete
  typed bindings. Assembly rejects missing, mixed, duplicate, aliased and
  unhealthy entries. Discovery validates the whole device index and does not
  downgrade after partial or complete native shutdown.
- The Required trait path authenticates the selected account, full model and
  extent before route capacity or child effects. Witness-free APIs reject.
- The shared allocation route transaction preflights capacity, overflow and
  vacancy. Only `Allocated(nonzero)` commits an outer ID. Other outcomes retain
  their exact diagnostics, including allocation-specific `SettledNoOwner`.
  Failed attempts no longer burn legacy outer allocation IDs; child-local IDs
  retain their prior semantics. No live-allocation scan or reverse index is
  introduced. Concrete child insertion enforces local uniqueness.
- An unwind after child effects seals the selected child and router and resumes
  the original payload. Unrouteable child custody remains in the sealed child.
  Release retains the outer route until child disposal succeeds.

## Qualification Scope

`validate.sh` records production commands, logs, statuses and executable hashes.
The isolated `qualify.sh` copies the current workspace sources, applies the
previous single-device packet's accounting/test fixtures plus this packet's two
test-module additions, and executes the production Context and router bodies.
`check-overlay.sh` verifies unchanged files, exact fixture contents, the complete
four-package file roster, and every overlay context/addition/removal (allowing
only hunk line-number movement). No shipped test-support API is added.

The isolated fixture can mint typed request accounts through private accounting
intake. It cannot mint a checked device or native N1/N2 admission. Descriptive
model bindings and `Composed(None)` mocks therefore establish accounting and
routing behavior, not production native-device association.

The seven new composed groups exercise:

1. Real `Context -> multi-device trait -> selected child trait` allocation on
   both devices and memory kinds, exact leaf/root charges and clean disposal.
2. Mixed, missing, aliased, duplicate and inconsistent policies.
3. Wrong-device, extent and foreign-root witnesses before route reservation.
4. Retry after partial reverse-order child shutdown with unchanged accounts.
5. Root lifetime through Context, returned backend and final clean Drop.
6. Quarantine retaining the root/registry after ordinary owners leave scope.
7. Scripted typed cold settlement/refund, successful same-ID retry, failed
   recycle retaining route/credits, successful release retry and clean shutdown.

Final cleanup requires zero request usage and reservation/retention/quarantine
counters on both leaves, plus exact restoration of the root's pre-allocation
usage. The root intentionally retains its control/bootstrap record. Scripted
clean shutdown does not disarm Drop or leave a live allocation behind.

The mutation removes only the router's witness match. The child still rejects
the request, but the negative test must detect the premature outer route
reservation. `qualification/mutation.patch` records that mutation; production
source is never mutated. Final source restoration and positive replay are
recorded in `raw/restoration.log` and `raw/qualification-restored.log`.

## Results

| Gate | Result |
| --- | --- |
| Production multi-device focused filter | 26 passed |
| Runtime all-feature library suite | 1,497 passed, 3 failed, 28 ignored |
| Runtime doctests | 49 passed, including both public constructor signatures |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-feature check and formatting | Passed |
| Isolated typed qualification before mutation | 21 passed (14 inherited, 7 new) |
| Router-authentication mutation | Intended assertion failure, exit 101 |
| Restored-source isolated qualification | 21 passed |
| Complete source/fixture restoration checks | Passed |

Counts overlap and must not be added as distinct-test totals. The three library
failures are `authorized_execution::tests::{cooperative_debug_telemetry_emits_only_bounded_logical_records,
failed_session_end_is_explicit_and_terminal, pre_native_telemetry_failure_is_returned_and_poisoned}`:
each reports `InspectSocket` with `Operation not permitted`, matching the
parent's known environment failures. This is not a fully passing CPU suite.

Final command results are recorded in `raw/`; earlier Clippy and cleanup-oracle
failures are retained separately. The cleanup-oracle failure expected zero
total root retention and was corrected to the exact bootstrap baseline, not by
weakening the request-credit assertions.

## Remaining Gates

- Actual checked-device constructors, late partial-admission cleanup, all native
  startup orders, pool reuse and multi-device native shutdown still need MI300X
  execution. The scripted SDMA path does not establish any native performance.
- Native XGMI composed admission and witness transport remain separate work.
  Generated shell APIs are still single-device. Worker proxies still lack
  borrowed-witness transport and must not advertise this profile.
- No new Verus proof is executed here. The model/accounting/KFD production
  trees are unchanged. Full Context/backend/accounting refinement, concurrent
  sealing exclusion, physical overlap and matched HIP/HSA measurements remain
  open. Session-health checks are snapshots, not atomic quarantine exclusion.
- Fresh SSH to `mi300x` fails resolving `sharkmi300x-1`; GitHub issue retrieval
  fails connecting to `api.github.com`. No remote test files were created and
  no current issue-state or hardware-result claim follows from these attempts.

## Reproduction

```sh
bash docs/evidence/dev-runtime-multi-request-2026-09-26/validate.sh /tmp/fe2o3-owned-target
bash docs/evidence/dev-runtime-multi-request-2026-09-26/qualify.sh /tmp/fe2o3-unused-copy
```

Use unused owned directories and remove only those directories after all
processes are terminal. This run removed its exact owned target (550,572 KiB)
and isolated source/target tree (513,456 KiB) after all handles were terminal;
both paths were confirmed absent. No unrelated temporary files were removed.
Three telemetry socket tests require an environment
that permits their local authenticated socket setup; their failures are not
counted as passing qualification.
