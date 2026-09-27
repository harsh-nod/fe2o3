# SDMA Initialized-Byte Coverage

Development evidence only. This is ordinary SDMA buffer coverage and digest
invalidation, not the completed persistent-owner/typed-compute bridge. Public
pending-peer launch admission remains closed. No GPU, XGMI/compute composition,
machine-code refinement, performance or HIP/HSA parity claim is made.

## Sources And Scope

Implementation source: `4d64c5cff1e1a57bd26ddd88841353ad7c063096`, SSH-signed by
`harmenon@amd.com`. The raw qualification directory is
`/home/harsh/.codex-tmp/fe2o3-prefix-20260927-hO4GUD0k`.

Final proof campaign source: `2edf53d4245fe8e720be6d59de64983ebe796e94`.
Its only change is a negative-control replacement in the campaign script. The
runtime, KFD and shared proof/body sources are byte-identical to CPU qualification.

Frozen archive: `receipts.tar.xz`, SHA-256
`8ba68f37ed20e86042c3140cf7b2e418e068bdd4d950378c21c88030eedd2c0a`.

Each ordinary SDMA buffer privately retains a constant-size initialized prefix.
Successful CPU writes and copy retirement may extend it only through contiguous
known bytes. Unknown-source writes truncate intersected coverage before effects.
Request creation cannot grow it. Digests remain separate: even a known-source
overwrite invalidates the destination's previous full-content SHA.

Review found a pre-existing stale-digest route through direct `submit`, which
bypasses request construction. A shared allocation-free preparation helper now
runs in direct/single/batch/window publication, and retirement defensively clears
the digest. Ordinary, batched, striped, logical-mux and persistent-window lower
retirement paths all update coverage. Native XGMI owns another mapping type and
is not covered by this change.

Pool generation changes, logical resize and bare reconstruction reset coverage.
Cleanup metadata preserves it. Wrong-kind directional and legacy promotion now
return the original Host buffer before splitting it. Successful persistent owner
promotion/detach/restore does not yet preserve this fact, so this implementation
does not authorize new typed compute reads.

## CPU Qualification

- Focused KFD: 239 passed, including 11 new scalar/buffer tests and constructed
  synchronous SDMA, promotion, demotion, recycle and cleanup cases.
- Full runtime: 1621 passed, 3 failed, 28 ignored. The three failures remain
  `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
  `failed_session_end_is_explicit_and_terminal`, and
  `pre_native_telemetry_failure_is_returned_and_poisoned`, all at
  `authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
- Doctests: 89 passed (KFD 37, runtime 8 + 44).
- Strict all-target/all-feature Clippy for KFD/runtime and the minimal runtime
  build passed.
- Formatting, whitespace and CPU source continuity passed.

The full KFD suite was not rerun for this increment. The preceding ancestry/seal
checkpoint's [complete-suite receipt](../dev-peer-ancestry-init-seal-2026-09-26/README.md)
records 1643 passes and one isolated-reproducible `SocketAdmission` failure.

New buffer tests fabricate private native-neutral records to test exact transfer,
unknown-source invalidation, range gaps, resets, cleanup, failed known writes and
raw-request digest revocation. They do not prove GPU completion or execute the
direct submit path. The constructed synchronous driver tests are CPU mapped-byte
execution, not GPU data movement.

## Formal Boundary

The positive proof root checks the exact production scalar macro bodies, including
bounds, overflow rejection, no gap bridging, known partial overwrite preservation
and unknown-write truncation. It does not prove concrete storage identity,
currentness, publication, firmware behavior, byte transport or machine code.

The first campaign is retained as incomplete: removing the extent guard produced
an arithmetic-overflow diagnostic outside the inherited classifier's accepted
postcondition failures. A separate probe preserved machine bounds while violating
logical extent and produced a postcondition failure. The final signed-source
campaign passes all 19 phases, including 11 body-only mutants rejected by the
authenticated classifier. Three full proof runs each report 8 verified and zero
errors. All 6079 source inputs are unchanged before/after; all 19 owned process
groups are absent. Pinned verifier-closure checks pass before/after. The reused
classifier and gate calibration controls do not prove native initialization.

## Remaining Work

Preserve coverage across exact persistent owner custody, seed it from genuine
completed compute/SDMA, implement sealed quiescent typed-input conversion and
preserve SDMA-versus-dispatch origin through cancellation. Then complete active
DMA ancestor admission and router gate/flush/drain integration. Padded typed
layouts, native XGMI composition and matched hardware benchmarks remain separate.

MI300X access again failed DNS before SSH execution; no remote artifacts were
created. This packet does not upgrade earlier hardware or parity claims.

Implementation and proof-campaign commits were pushed to `harsh-nod` and read
back exactly. The `powderluv` push failed DNS resolution; its branch remained at
`2307ff7d88e093019208e2f8391e498956f415f6` at readback. See `source-publication.json`.
