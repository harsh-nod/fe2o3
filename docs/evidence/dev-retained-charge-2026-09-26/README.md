# Exact Retained-Credit Association

Base: signed `683d6559b904086698b14ec9a003ea9de40063c6`.
This is an accounting prerequisite for logical/native admission composition,
not complete MEM-DOM, A1/A2 or HIP/HSA acceptance.

## Production Contract

`ResourceCreditAccountV1::matches_retained_charge_v1` borrows an expected account
and move-only retained credit. It rejects missing tokens, unequal account
variants or Arc identities, poisoned mutexes/ledgers, vacant or out-of-bounds
slots, zero/stale owner generations, non-Retained phases and any full-vector
mismatch. A domain additionally requires the exact leaf key and its valid
generation-bound, bounded root-terminating ancestry. Shared ancestry or equal
limits do not suffice. Equal charges in the same exact leaf remain fungible
accounting records; no unnecessary allocation-to-token identity is introduced.

The query acquires at most one matching ledger lock. It uses borrowed records,
nineteen-coordinate equality and at most four ancestry lookups, with no heap
allocation, account clone, handle increment, arena scan, refund or native call.
It refuses a poisoned standard mutex directly, rather than using the existing
recovery helpers that set flags/anchors. Thus even rejection changes no ledger
state. This is an observation, not a durable lease or physical authority.

The runtime wrapper additionally checks its Context/device brand. All twelve
existing Context association guards now supply the allocation record's complete
`byte_len`, never a copied/generated subregion length. Request-vector creation
is shared by single reservation, roster reservation and checking: requested
bytes plus exactly one allocation record, with all other coordinates zero.
Unconfigured/no-credit admission still passes; asymmetric presence rejects.
The compact token representation and public Context defaults are unchanged.

These are the existing reader, producer, generated, submission and disposal
guards, not universal interception of every Context or raw backend operation.
Ordinary unjournaled disposal and host mutation retain their existing contracts.
A third request-credit leaf, mandatory native allocation witness and complete
logical/native construction remain future work.

## Tests

Seven accounting test groups cover exact identities with deliberately colliding
slot/owner/leaf values in foreign ledgers; ancestor/sibling/cross-profile
rejection; increases and decreases in all nineteen coordinates; repeated
unchanged observations; invalid record/token/phase/owner state; malformed
selected ancestry; and logical versus standard-mutex poison. Internal CPU fault
injection restores each fixture before disposing its credits. Mutex-poison
checks inspect the raw poisoned guard without invoking recovery helpers.

Six runtime groups cover presence combinations, Context/device branding,
full-allocation versus subregion extent, equal-charge foreign/sibling credits,
valid same-account exchanges, and producer-aware consumers with mismatched
stable input. Context tests restore substituted credits immediately before
assertions/cleanup and inspect identities, call counts, journals, usage and data.

The initial producer test deliberately substituted an existing producer's
destination credit. Existing dependency-custody validation correctly entered
terminal quarantine before consumer input admission, invalidating the test's
attempted restoration. Its failing log is retained. The final test substitutes
the independent stable input instead: this exercises ordinary preissue consumer
rejection without corrupting an already-issued producer. Production quarantine
was not weakened. A pending producer's corrupted retained root is not claimed
to have a nonterminal/no-effect response.

Final focused tests pass: seven accounting and six runtime groups. The broad
libraries pass 1,034 model, 70 accounting, 1,321 KFD and 1,482 runtime tests.
The four prior socket failures remain: KFD
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `SocketAdmission`, and runtime
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
at `InspectSocket(EPERM)`. Library exit is 101. This is not full CPU qualification.
All 69 selected construction/custody tests pass (67 KFD, two runtime), along
with 104 doctests (28 KFD, three accounting, 46 runtime, 27 model), strict
all-feature/all-target Clippy, no-default-feature checking and workspace
formatting. These selections overlap and must not be summed as
distinct coverage. The library run filters 296 construction cases and retains
19 model/28 runtime ignores; native ignores are not promoted to passes.
The validation driver terminated with exit 1 because the library stage returned
101; `raw/final-status.tsv` records every stage separately.

Reintroducing only the old two-boolean presence check makes both selected copy
regressions fail on their expected-rejection assertions (zero passed/two failed,
exit 101), not on compilation or infrastructure. The exact and mutated Context
files are retained. The exact check was restored byte-for-byte, verified against
its prior digest, before final qualification. This is a Rust regression mutation,
not an additional Verus obligation or a replay of an entirely old checkout.

## Proof Scope

The authenticated shared planner/immutable arena campaign passes against the
changed accounting adapter source pins. Both bracket positives verify planner
19/0 and arena 21/0; all 31 semantic mutations reject logically in the targeted
unit while the other unit passes, for 66 unit executions. Nine controller
calibrations pass. Shared declaration obligations overlap; unit counts are not
additive distinct properties. The manifest digest is
`0468ee4ff775fa0c9100de63299d028ecf0e43202c1230df7c87e8d367bd1276`.
The outer runner's before/after hashes match; pinned tool/source closure checks
pass before and after execution. The controller owns and reaps its process
groups within its own process view; successful terminal tool status is retained.
Cross-exec numeric-PGID probes alone are not treated as lifetime evidence.

The 19/21 unit contracts are unchanged. Requalification does not extend them to
this new mutex/token/record query, Context association or complete ledger conservation. Those remain
explicit adapter/composition proof obligations. No model-only predicate is
substituted for a proof of the new implementation.

## Cleanup And Remaining Work

The frozen source closure matches before and after final validation and again
before cleanup. All four actual library test executables have recorded SHA-256
digests and passed rechecks before removal. Receipts under `raw/` record the
terminal validation session, exact owned target, removal of 631,180 KiB and a
separate filesystem absence check. No unrelated artifacts were removed.

The MI300X probe fails resolving `sharkmi300x-1` before remote entry;
no shared-machine work or native result is created. No new performance result is claimed.
The next integration work is the third logical-request leaf under the exact
native root/device/session, followed by mandatory allocation witnesses and
complete accounting/proof/native qualification. Accepted milestones do not move.
