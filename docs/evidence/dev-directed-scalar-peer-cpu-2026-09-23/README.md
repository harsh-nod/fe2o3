# Directed Scalar Peer-Copy SPI: CPU Checkpoint

Development CPU evidence for the
[directed scalar backend contract](../../runtime-directed-scalar-peer-v1.md).
This does not enable Context pending-producer reads or close A1/A2.

## Implementation

The additive backend SPI binds the complete directed route and original ordered
event/expected-producer roster. Only the native XGMI backend opts in; every
dependency must have retained directed-profile provenance. The existing scalar
admission, native success gating and one-action progress selector remain the
execution path. Legacy peer-copy, ordered-copy and Worker contracts do not
inherit the new guarantees.

Preallocated provisional roots precede shared admission. Definite rejection
removes only the new root; ambiguous failures and panics retain it and seal the
backend. Unexpected quiescent errors are promoted to terminal with the original
payload retained. Exact progress matching precedes any adapter action.
Completed provenance survives event, stream, allocation and predecessor release.
The shared release path preserves original guard/error precedence, validates
metadata before mutation and removes completion, depth and root together.
Shutdown and Drop account for retained roots.

## Qualification

Pinned nightly `2026-04-03`, all features, test optimization level 1, debug
assertions enabled, incremental compilation disabled and two build jobs:

- GNU runtime library: 1,285 passed, zero failed, 20 hardware-only ignores.
- Musl runtime library: 1,285 passed, zero failed, 20 hardware-only ignores.
- Runtime doctests: 46 passed, in groups of 4 and 42.
- Formatting and all-target warnings-denied Clippy pass.
- All seven command receipts have status zero and owned process-group absence.
- All 3,810 captured source/configuration/fixture hashes match before and after
  execution and match the current source.
- Runner SHA256: `d47c79fe98d4e67d85d4de877705921d50f43e10f107c26af99c293c6699f266`.

Twenty new CPU test functions cover both directions, original dependency order,
coordinate substitutions, event binding and aliases, unsupported legacy cohorts,
missing producer records, roster bounds, rejection rollback, terminal and panic
retention, exact progress, completed provenance without live public resources,
malformed active/completed state, live-index drift, release precedence and
no-partial-removal, and legacy completion release. The readiness cases execute
the existing native success predicate. The broader runtime suites also exercise
the existing scalar progress and admission tests.

The fixtures script native leaves. They intentionally do not construct KFD
sessions, perform publication/cancellation or enforce all native stream and
allocation-owner rules. Shared admission/root/matching/release logic is executed;
native ticket, mapping, currentness, Drop-abort and physical-copy behavior are not
thereby qualified. Two independent source reviews found no remaining correctness
or custody findings after corrections. No new formal theorem or refinement
evidence is supplied.

The archive contains 23 raw records: seven command triples and two input brackets.
Captured stdout bytes, including Cargo's final blank lines, are preserved.
The runner keeps its absolute workspace and private-target convention; its
argument selects the output directory. These are development source brackets,
not a portable or hermetic compiler/toolchain attestation.

## Retained History and Cleanup

Private scratch root:
`/home/harsh/.codex-tmp/fe2o3-directed-scalar-20260923-TwUUvhkx`.

- `focused-1` failed compilation with two wrapper borrow conflicts and two
  temporary-array lifetime errors in the fixture. Status 101, process-group
  absence and unchanged input brackets are preserved.
- `focused-2` passed all twenty new tests on its preliminary source. The final
  source additionally elides an unnecessary lifetime and collapses an equivalent
  conditional guard before the complete campaign.
- `cpu-1` is the final successful full campaign archived here. It does not
  substitute a focused or earlier-source result for final qualification.

After qualification, `cargo clean` removed only the private target's 1,339 files
(660.5 MiB). Raw records and the runner remain; no shared cache was removed.
No SSH, GPU operation or HIP/HSA benchmark ran for this checkpoint.

## Remaining Gates

Context still rejects pending sources. Next are exact writer/member/event
binding, producer-read reservations, retained terminal observations,
producer-first reconciliation across all completion paths, and bounded async
integration. Native chain/canary/fault/cleanup qualification follows. Metadata
count limits are not an aggregate byte budget; private provenance validation is
not arbitrary-corruption authentication.

Historical proofs remain evidence for their captured inputs, not this changed
Context/backend source. Native R125, Admission R118B and Resources R116/V3 remain
the broader accepted checkpoints. A1/A2, #182 and HIP/HSA parity remain open;
there is no new native, formal-refinement or matched-performance acceptance.
