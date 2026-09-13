# Uninitialized Device Insertion Custody V1

## Accepted Status

R112 locally accepts N3-L3-D above accepted R111
`29505205cc54bab1885a67845aaceda8b22b3a9c`, with
[retained evidence](evidence/local-r112-uninitialized-device-insertion-2026-09-13/README.md).
The source/test campaign, restored checks and independent archive review are
complete at the named CPU boundaries below.

Three read-only workers reviewed production, low-level failure oracles,
constructed insertion and public boundaries. Primary owns edits, integration,
serialized tests and publication. The preliminary all-feature/all-target KFD
regression passes 1,126 tests across 21 harnesses, including all 26 new names.
After a final routing-guard-only strengthening, the insertion and allocation
focused runs pass 14/14 and 12/12, and strict all-target KFD Clippy passes again.
The subsequent source campaign passes all seventeen gates, preserving all 5,679
frozen non-documentation identities. GNU and musl each pass 2,683 tests with
five ignored across 48 harnesses. Ten auxiliary and nine frozen focused suites
also pass. Sixteen evidence-clock contract tests pin eight helpers. All sixteen
planned negatives compile and fail at their exact behavioral oracles, with all
5,679 source identities restored after each and independent evidence review.
All nine restored suites pass. A passing allocator child was excluded after its
evidence wrapper rejected a raw UTC regression; a separately pinned monotonic
continuation supplies the fresh accepted allocator and remaining suites. The
closed collector and independent archive review pass with 263 raw artifacts.
This is local source/test acceptance, not native or formal acceptance.
The implementation adds
25 dynamic test functions and one source guard; one dynamic test is a regression
for the generalized ownership auditor.

This packet adds no Linux/KFD hardware, authenticated formal,
aggregate-memory or performance qualification. It does not complete A1/A2,
issue #182, or HIP/HSA parity.

## Production Boundary

The sole existing direct API,
`allocate_uninitialized_fixed_dispatch_data(requested_bytes, alignment)`, now
uses the existing `data_insertion.rs` settlement sequence. It preserves the
remembered-hole-or-append policy. There is no new selected-lane facade,
standalone allocator, source buffer, initialization descriptor or generated
submission adoption hook.

`shared_memory/device_allocation.rs` owns actual allocation custody:

```text
None -> Unmapped -> Mapped -> extracted
```

Allocation uses the existing DEVICE_LOCAL lower core, not DEVICE_LOCAL_PUBLIC.
The returned Unmapped lease is stored before borrowed GPU mapping. Only a
successful map, including closing currentness, promotes that owner to Mapped.
The root separately records one-shot entry, failure, per-call native admission
and exact map-attempt/result/prefix observations. Progress is not a lease.

A reservation failure can retain an admitted attempt and charge without a
returned lease or native record. A later allocation failure can retain an
ambiguous record without a lease. GPU-map or its closing-currentness failure
retains the actual Unmapped owner even if the native map returned success.
Failed roots never expose completed authority; extraction is one-shot.

Validation and pre-effect accounting preserve existing lower ordering.
Historical activity is not a new attempt's admission marker. The existing
configured/unconfigured backing-unwind distinction remains at first currentness:
configured accounting quarantines on that unwind; unconfigured accounting
remains active before native admission.
After per-call admission, an unwind quarantines immediately, before outer
terminal retention can run.

## Settlement And Retention

Mapped output stays outside the model loan until retake, completed-identity
validation and ordered identity/count commit. Extraction returns only
DeviceUninitialized data. No CPU mapping, copy, hash or readback is introduced.
Device operations preserve coherent model contents; successful loan/retake
still advances loan generation.

Both device root kinds share the existing fallibly preallocated terminal slot.
Whole-slot occupancy recognizes both; typed test projections deliberately return
None for the other kind. Failed complete output retains its Mapped owner but
denies output access. Retention performs no native cleanup, retry or refund.

The mixed enum intentionally reserves its larger initialized variant before
native effects. A narrow Clippy expectation documents why boxing during failure
would be inappropriate. The inline engine still holds a Vec, not a large root.
Tests check stable pointer/capacity and ordinary-stack construction. The defensive
late occupied-slot branch is non-dropping; it does not create recoverable storage
for a second owner.

The direct wrapper retains the terminal parent before exposing an error or
resuming a panic. Pure public preflight remains nonterminal. Once preparation
is entered, even lower size/alignment rejection transfers the parent after
retake, while retaining no unadmitted allocation root.

## Test Boundary

Low-level tests reuse the fake-memory engine and an unrelated mapped anchor
under both accounting modes. They cover exact flags/layout/identity, one-shot
output, all four currentness boundaries, allocation/map errors and unwinds,
malformed results, map-prefix precedence, immediate denied retry, mixed-kind
occupancy and stable storage. Independent byte/record credit tests use actual
fixture disposal before retry. Aggregate-byte and ID arithmetic tests inject
counter boundaries; they are not large native allocation evidence.

Constructed tests reuse the primary/auxiliary fixture, original memory engine,
loan, accounts and production sequencer. They cover append, genuine release-hole
replacement, closing/commit failures, fifteen-to-sixteen capacity, opening and
reservation failures, invalid lower layout, missing Complete, native/currentness
prefixes, and operation-panic versus closing-failure precedence. Later ordinals
move the same auxiliary behind a vacancy; they are not a third native queue.

The shared auditor observes both terminal kinds and exact original/new record
ownership. Extracted roots cannot invent pending charges. Coherent-model
baselines are nonoptional. Separate public tests exercise real direct-API
rejection order, bound dispatch and missing-engine parent transport. Source
guards do not establish successful native execution.

## Qualification Record

- The completed source/tool/environment freeze, full/focused/auxiliary campaigns
  and original manifest of 42 planned causal-chain entries are retained.
- All sixteen compiled negatives cover admission, immediate quarantine,
  incomplete owners and progress, flags, one-shot state, complete retention,
  mixed-kind occupancy, insertion policy, authority and commit/retake ordering.
- All nine restored rosters pass with exact source identities. Both clock
  contracts, the closed collector transcript and independent archive audit pass.
- The original unfinished tail remains explicitly superseded by the supplemental
  manifest; it is not reported as completed under its original contract.

Read-only review has approved the source expectations for nine focused rosters
and all sixteen executed behavioral negatives, including two checks of the
reused model-loan substrate. The original negative/restoration cohort and first
restored suite retain their original UTC contract and helper pins. The allocator
attempt retained child exit zero but wrapper rejection 125: its raw child-close
UTC moved backward by 1.175 seconds while monotonic time advanced. The attempt
is not accepted retroactively. The supplemental manifest pins all 218 original
artifacts and five separate helpers, establishes a fresh boot-bound monotonic
epoch, and binds the successful allocator plus seven remaining suites and
collector. Raw UTC is preserved, never clamped or synthesized; original
records have no boot ID and are not retroactively boot-bound.
The source, helpers, environment, both manifests and raw records are retained
in the evidence archive. Eight explicit doctest relocations are accepted
only after their compile-fail fence contents match the parent exactly.

The preliminary namespace-resolution compile failure and first strict-Clippy
failure remain in task-owned local R112 artifacts with source maps and raw logs;
neither is a passing run. The obsolete insertion helper was removed. Its legacy
ordinal test now exercises the production explicit-insertion primitive; the
constructed real-hole test covers selection. No SSH, GPU or solver jobs were run.

Next independent work remains [C1 and V2](runtime-swarm-next-packets.md), with
Native N4 cleanup following applicable custody contracts.
