# Native XGMI Creation Recovery

## Scope

This checkpoint extends the accepted `009e06610` settled-frame checkpoint with
recoverable host preparation failure before native XGMI queue creation is armed.
It does not make an uncertain native failure recoverable.

The lower layer records private provenance only for actual fallible host-resource
preparation. Route/engine rejection, stale currentness, unsuccessful model
retakes, native arming and later failures cannot acquire that classification.
Both models must be successfully retaken and the exact live session pair, route,
vacant creation root and absent attachments rechecked. A move-only outcome
classifies its own enclosed error; no reusable public recovery certificate is
returned.

The runtime validates both retained original owners and both destination slots
before restoring either owner. Restoration uses already allocated shells and
preserves destination initialization/content metadata. The operation settles
Failed, refunds its retains and clears only its endpoint reservations. It does
not increment the native successful-copy counter or poison independent work.
Ambiguous failures and panics retain the existing terminal custody policy.

Context failure semantics are unchanged: an accepted failed writer becomes
Unknown. Preserved native bytes do not restore journal lineage. A journal-enabled
caller must dispose the failed destination and initialize a new one; the
no-journal witness separately tests retrying the same original destination.

## Qualification Design

The feature-only denial is a per-operation explicit preparation mode, not an
environment variable or global fault switch. It executes
`Vec::<u8>::try_reserve_exact(usize::MAX)` at actual host preparation before
native arm. This is deterministic capacity overflow, not OOM, GPU, or driver
failure. Cancellation or dependency failure can leave the request unused.
The exact-submission observer requires the real native classification, Failed
status, quiescence, restored original owners and cleared endpoint reservations.
Arming plus a generic Failed result is insufficient.

The copy-only witness uses the production constructor with authorities that deny
every kernel launch. Four distinct MI300X devices form independent pairs A and
B. B is published and retained before A is admitted and armed. A must fail with
the exact certificate while B remains Pending, retained and unarmed; B must then
complete. Only after both pairs are quiescent may full-buffer reads occur.

A retries on a fresh stream with a changed full source payload. Both modes
inspect every original destination byte before retry. Synchronous inspection
does not validate or promote journal lineage. Journal mode must reject queued
D2H reading and peer retry on the Unknown original destination, dispose it and
initialize a fresh destination. No-journal mode retries the same original
destination. Native completion counters
must advance `0 -> 1 -> 2`, with exactly three original-result callbacks.
Scalar and overlapping/duplicate segmented copies run in both pair directions.
Two prior packet-spanning segment cases are additional success controls.

An independent Python oracle reconstructs every logical source/destination byte,
including untouched holes and guards. The digest includes a domain separator
and a little-endian u64 length before each full allocation, ordered A source,
A destination, B source, B destination. Each run uses one Context and explicit
logical/native shutdown. Same-process reopening is not tested.

## Acceptance

Final-source CPU checks pass 2276 runtime tests with 32 unchanged hardware-only
ignores, 30 example tests and 114 doctests. Strict Clippy, default/no-default
library checks and all 32 source-control workflow commands pass. The compile-only
API probe requires exactly three missing-method errors without
`hardware-qualification`, then compiles successfully with the feature enabled.

All 1950 KFD tests pass in five disjoint, exact-roster generation-06 shards.
Generation 07 changes only the recovery example across the complete 6140-file
source inventory. A fresh KFD build/list has the same executable bytes and
1950-name roster. The evidence explicitly reuses generation-06 KFD execution;
it does not label it as another fresh suite. All other selected qualification
commands use the final generation-07 source.

All ten final-source MI300X cases pass on 2026-10-03 UTC: eight recovery cases
on GPUs 1/2/6/7 and two packet-spanning controls on GPUs 6/7. The offline native
replay checks all 101 transport receipts, exact executable identities, independent
full-byte digests, per-case admission and owned cleanup. The aggregate auditor
accepts the CPU/native receipts, unchanged test ignores, explicit KFD reuse,
source metadata and feature-gating probe.

GPU 0's foreign process roster remains unchanged. The final owned-executable
process check is empty, both uploaded binaries and their exact private scratch
directory are removed, and selected GPU activity/VRAM and whole-host process
baselines are restored. Point observations do not establish a reservation.

`raw.tar.xz` contains command records, source manifests, both native campaigns,
independent oracle/replay, metadata proposal and readback checks, aggregate
auditor, packager and staged source patch. `raw-manifest.json` records every
archived file's hash and size; `SHA256SUMS` covers the README, manifest and
archive. Executable bytes are excluded; their identities and build commands are
retained. Packaging verifies every archive member against the original raw file.

## Retained Diagnostics

Superseded builds, lint diagnostics and nonselected source captures remain in
the evidence. An earlier runtime run had one new test fail at cleanup after its
authentication assertions passed. Its pending mock copies are now explicitly
cancelled and their results released before the original cleanup assertions;
the complete runtime suite was rerun successfully.

The first native campaign passed four no-journal cases, then rejected the first
journal case because the witness incorrectly expected synchronous inspection
to reject Unknown contents. That campaign remains rejected, with successful
owned cleanup recorded. Only the witness changed: it now verifies every original
destination byte in both modes, asserts unchanged journal counts, and separately
requires queued D2H and same-destination peer retry to reject. The runtime
journal contract was not changed to satisfy the test. The final campaign repeats
the entire matrix; it does not combine earlier partial passes with later runs.

## Limits

- Only certified pre-arm host preparation rejection is recoverable here.
- Post-arm/native mapping, publication, retirement and GPU faults are not injected.
- Journal Unknown handling is not relaxed or inferred from native metadata.
- Point host-admission checks are not an exclusive reservation or continuous census.
- Logical retained work does not prove physical overlap or performance parity.
- No general application-kernel authority or machine-code refinement is added.
- Ordered destination-list chaining and broader multi-GPU failure isolation remain open.

Source-guard refreshes change only recorded hash/count literals. The cadence
exception explicitly acknowledges substantive, unrelated `sdma.rs` creation
changes while authenticating unchanged cadence signatures/bodies and selector
closure. It does not reuse whole-source packet/window proof qualification for
the changed lower layer. No new solver campaign or whole-adapter proof is claimed.
