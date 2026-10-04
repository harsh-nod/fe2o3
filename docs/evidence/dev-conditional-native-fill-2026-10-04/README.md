# Conditional Native Fill Prerequisite

This checkpoint implements the native spatial/storage prerequisite for the first
ordinary verified two-GPU application. It does not join pending compiler/proof
custody to execution authority, complete A3, or claim HIP/HSA parity. Native
agents independently reviewed the implementation and proof-handoff boundary;
the primary made and integrated the changes.

## Closed Profile

`Gfx942FixedDispatchPacketV1::require_conditional_fill_v1` is a consuming,
tightening constraint. There is no corresponding clear operation or authority
grant. It requires one ordinary program and packet, one nonempty whole write-only
coherent-host `u32` output, the existing exact gfx942 fill machine/descriptor
profile, 272-byte kernargs and no dynamic LDS. Geometry is `[G, 1, 1]` with
workgroup `[64, 1, 1]`, positive G divisible by 64 and N <= G; logical output
extent must equal 4*N. Persistent replay, extra rosters, DATA offsets and completed
snapshot enclosures are excluded.

The runtime projection preserves the original dispatch-contract digest, image
and buffer allocations, but retires its private source identity so earlier inert
plans cannot substitute an unconstrained packet. Generated storage preserves the
constraint. The lightweight `gfx942-fill-model` feature exposes the existing
machine model without adding Worker, artifact-transaction or Pliron dependencies
to the normal KFD dependency graph. KFD tests enable the existing decoder solely
to load the captured Worker request fixture; they do not execute Worker/Verus.

## Original Native Custody

Preparation checks the actual selected envelope and the actual 272-byte image
after pointer fixups and COV6 implicit initialization. Private retained premises
bind the original code/kernarg/output allocation session, allocation ID and
generation, mapped facts, materialization identity, packet, exact output extent,
recipe occurrence and initial dispatch generation. The existing model checks
output bounds and disjointness against the complete kernarg allocation without
iterating the grid.

Submission requires those same original retained owners and the checked packet.
The first successful reservation consumes its generation even if subsequently
cancelled; replay requires a fresh preparation. Replacement and host-projection
preflight apply the same constraint before native changes. Persistent entrypoints
reject it before their lifecycle operations.

The premises are owner-associated snapshots, not independent live mapping
observations. Existing device/queue validation remains required. Device-local
output is deliberately excluded: its current allocation identity lacks the
memory-session association needed to distinguish equal metadata from independent
sessions. This restriction matches the initial generated coherent-host path.

An optional boxed record is allocated at the capacity stage before native effects
and moved unchanged during commit. Commit adds no allocation or callback. This
preserves the existing inline-layout guard: the corrected queue measures 39,592
bytes against the unchanged 41 KiB limit. `Box` allocation exhaustion remains
process-aborting, not a recoverable capacity error. Failures and unwinds after
native work preserve the existing retained/quarantined ownership behavior.

## Validation

Source: `6ed4daa0eb139f092400cd2b34e44239d546a156` (SSH-signed and locally verified).
This checkpoint uses CPU fixtures, not a new GPU run or formal theorem.

- The standalone `gfx942-fill-model` feature passes 23 tests with default features
  disabled.
- Strict Clippy passes for KFD tests and the runtime/loader libraries with
  `--no-deps -- -D warnings`. Changed-file formatting and whitespace checks pass.
- Default-feature library regressions pass all 1,953 KFD, 2,201 runtime and both
  loader tests. Five explicit MI300X runtime tests are ignored, not counted as
  passes. The final-source runtime run completes with no failures.

The ten new native cases cover full and partial tails, extra full groups,
geometry/shape rejection before effects, owner/requirement substitution, every
patched kernarg byte, changed machine entry words, short profiles, persistent
replay, extra rosters, failure/unwind custody, pristine cleanup, equal mapping
facts from foreign sessions, and device-local rejection. A runtime case checks
source-identity retirement without repacking the original payload.

Tests use optimization level 1 with debug assertions and overflow checks enabled,
offline dependencies and HIP discovery disabled. Only the KFD/runtime/loader
commands select four test threads; the standalone model uses harness defaults. The full
combined run began before removal of a redundant runtime method `must_use`
attribute; its return type already carries that attribute. A separate final-source
runtime run qualifies that last lint-only change. The combined run's KFD and
loader suites completed successfully, but its duplicate runtime pass was stopped:
four intentional-abort children remained in `CoreDumping: 1`, blocked in
`pipe_write -> elf_core_dump` for the WSL crash collector despite their zero core
limits. The exact owned runtime parent was terminated and its four already-crashing
children killed; all those process IDs were reaped. The orchestrator exited 101.
No complete combined-suite pass is claimed. The independently completed
final-source runtime run passed its abort cases and remains the runtime evidence.
Global crash settings were not changed. The existing test-only unused `snapshot`
method warning is outside this change.

## Evidence And Cleanup

`qualification.tar.xz` retains exact source hashes and diff, signature check,
compiler/host metadata, dependency tree, tests/lints, WSL crash diagnostics and a
reproduction script. Archive SHA-256:
`9915545029375187f3731d6f563d544d87dd378c3f18055d9dab21329b42666f`.
`cleanup.log` records removal of the owned test/log scratch directory after
archive validation. Normal build caches remain. No MI300X resources were allocated.

Earlier failures are retained separately, not counted as successful qualification:
`library-tests.log` is an incomplete obsolete run stopped after detecting the
inline-layout regression; `inline-layout-before-fix.log` isolates its 43,224-byte
queue failure. `library-tests-final.log` is an interrupted intermediate build
before the device-local restriction, not a test result.
`runtime-loader-clippy-before-fix.log` records the corrected redundant attribute.

## Remaining Critical Path

1. Qualify genuine compiler receipt/current-record acquisition in the unchanged
   measured deployment. The local host lacks SquashFS zstd support; no compression
   fallback or production admission relaxation is included here.
2. Implement an independently authenticated retained-proof custodian for the
   permanently no-fork ordinary application. Preserve the original proof owners
   and exact process occurrence through both devices' invocation settlement;
   copied signed records and a one-time liveness check are insufficient.
3. Consume that pending proof, exact packed coverage, native preparation, selected
   device and publication currentness into private invocation authority.
4. Qualify the admitted two-GPU fill, completed output, staging, settled H2D,
   PUBLIC XGMI and guarded readback in both directions, including N=65/G=128.

The first pipeline remains small-buffer and current-thread/direct-polling. Larger
or threaded variants, new opcodes, extra GPU counts and performance campaigns are
deferred. See the [critical path](../../runtime-multi-gpu-critical-path.md).
