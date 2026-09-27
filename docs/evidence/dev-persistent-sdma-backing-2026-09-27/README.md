# Persistent SDMA Initialization Backing

Development evidence, not A1/A2, #182 or HIP/HSA parity acceptance. This closes
the SDMA buffer-to-persistent-owner metadata bridge, not typed-compute admission
or authenticated post-compute initialization. No hardware execution, machine-code
refinement or performance result is claimed.

## Source And Archive

Implementation: `8de35fdaae044498a1ff7455f5ffbb914fa3eed7`, SSH-signed by
`harmenon@amd.com`. Root implemented and integrated; a read-only agent reviewed
authority, cancellation, restoration and the final heap-state migration.

Raw directory: `/home/harsh/.codex-tmp/fe2o3-backing-20260927-nsBKbNEw`.
Frozen archive: `receipts.tar.xz`, SHA-256
`6fe4a6002c569d6e2a8d8c212052d181c45ce0e47c5dbcf305e5230e206b1d69`.

## Changes

- Opaque backing moves storage and initialized-prefix evidence together. Raw
  extraction/reconstruction discards the evidence; logical bytes do not certify
  physical padding. There is no lease-plus-boolean/prefix constructor.
- Directional single/window, synchronous, legacy and same-device paths now
  transfer whole buffers. Paired transfer preflights both owners. Failed
  restoration returns exact original buffers without reconstructing metadata.
- Backing export checks exact storage, queue occurrence, generation and extents.
  Core restoration checks storage and physical extent; production adapters
  separately authenticate queue, generation, logical extent and copy geometry.
- Compute cancellation preserves evidence only for the exact detaching slot and
  generation, with an opaque owner-minted preflight permit. Completed raw compute
  restoration clears coverage. Public ledger transitions cannot initialize bytes.
- A missing native mapping returns WrongState on extraction instead of panicking.
- Native backing and the detaching-use stamp share the existing ledger Box.
  No new allocation is introduced per owner or transfer. The initial inline
  design failed the queue-size test at 43,584 bytes; the corrected layout is
  owner inline 136 bytes, owner heap 3,328 bytes (3,072-byte ledger), and queue
  inline 38,976 bytes. Existing inline thresholds were not increased.

## Qualification

Frozen-source results:

| Lane | Result |
| --- | --- |
| Focused KFD | 228 passed, 0 failed |
| Layout | 1 passed, existing inline limits plus bounded heap overhead |
| Runtime default features | 1512 passed, 3 failed, 5 ignored |
| Runtime all features | 1621 passed, 3 failed, 28 ignored |
| Doctests | 89 passed (KFD 37, runtime 8 + 44) |
| Strict all-target/all-feature Clippy | Passed for KFD and runtime |
| Minimal runtime, formatting, whitespace, source continuity | Passed |
| Scalar proof campaign | 19 phases passed; 11 logical mutants rejected |

Both runtime configurations reproduce the same three existing telemetry failures
at `authorized_execution.rs:1317`: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each with
`InspectSocket(PermissionDenied)`. That source is unchanged by this checkpoint.

Development logs retain the rejected inline layout and a subsequent 243-pass
run, including 15 expensive construction tests omitted from the repeated final
filter. They are distinguished from final frozen-source qualification. The full
KFD suite was not repeated; its preceding complete-suite baseline is documented
in the [ancestry/seal packet](../dev-peer-ancestry-init-seal-2026-09-26/README.md).

Six new tests cover direct publication, owner scope/raw extraction, sequential
directional copies, paired rejection/invalidation, exact compute cancellation,
and bookkeeping/pool reset. The direct-submit test uses actual CPU mappings,
injects completion, checks digest revocation before retirement, crosses persistent
ownership, releases resources and verifies refunds. It does not execute a GPU
copy. Other new owner tests use synthetic native-neutral completion records.
Existing constructed driver tests exercise broader CPU cleanup and failure paths.

## Formal Boundary

The shared scalar arithmetic and proof/controller sources are unchanged from the
[previous prefix packet](../dev-sdma-initialized-prefix-2026-09-27/README.md).
The new signed-source campaign reports three full positive proofs, each with
8 verified, 0 errors and entire-crate success. All 6079 source inputs are stable;
all 19 owned process groups are absent. Pinned closure checks pass before/after.

This proves scalar range arithmetic, not the new backing representation, concrete
resource identity, publication, firmware or machine code. Native adapter checks
remain Checked/CPU-tested, with external device behavior Contracted. The permit
and Rust move-only API are not substitutes for executable-refinement proofs.

## Remaining Work

Consume original authenticated `Gfx942FixedDispatchDataV1` values at compute
completion, preflighting all three before restoration. Then add an opaque
quiescent InitializedStorage input and preserve its origin through binding,
replay, cancellation and runtime transitions. Native predecessor admission,
router gate progress and composed generated graph qualification remain open.

MI300X DNS still fails before connection; this work created no remote artifacts.
Matched HIP/HSA measurements and broader milestone acceptance are unchanged.
