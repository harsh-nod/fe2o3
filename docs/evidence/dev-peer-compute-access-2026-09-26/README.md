# Private Peer Access Under Gated Compute Custody

Development evidence only. Public pending peer-to-compute admission remains
closed. This packet establishes neither HIP/HSA parity nor native execution,
machine-code refinement, XGMI composition, or a performance improvement.

## Signed Sources

- Implementation: `c9c1065946d1a90c5f50318fa7c554ff35d1d8f7`.
- Final qualification tree: `f4348001836b1492296747873ced0efce3cc806c`.
  The second commit only moves the new scalar test module after production items.
- Both commits have verified SSH signatures from `harmenon@amd.com`.
- Frozen raw directory: `/home/harsh/.codex-tmp/fe2o3-peer-access-20260926-wuw0hrGI`.
- `receipts.tar.xz` contains final qualification plus clearly named earlier
  exploratory attempts, including the corrected test-setup and Clippy failures.
- Archive SHA-256: `55694d6f9b087002ea0174e0564df57019c0e0479621f419de5e63e7a78621e3`.

## Implementation And Tests

Private origins bind producer, child, allocation, original interval and copy leg.
Bounded, sorted direct-parent permits live on the accepted child compute owner.
Each conflicting compute owner must authorize access independently; public
memory operations and copies receive no origin. Copy and native-reconciliation
authority are distinct. Accepted DMA captures exact chunk, stream and scratch
arguments and checks clean endpoints before publication. Retained reconciliation
checks descriptor/generation, scratch size/backing/clean exclusive custody, and
endpoint backing before native reads or mapped writes. Common leaf entry checks
origin identity for progress, cancellation and disposal.

Ten focused groups pass. They cover real accepted child custody with multipart
scripted DMA, both native-dirty HostVisible endpoints, public access exclusion,
consumer cancellation while DMA remains published, changed publication arguments
and authority, backing/scratch corruption, exact origin bounds and overflow,
separate permission purposes, and the scalar gate matrix. Synthetic host/device
copy compatibility is exercised without manufacturing native compute admission.

Publication corruption controls call the common guard directly with a retained
published record. They do not establish a naturally deferred Ready retry. The
positive consumers use private child admission, not public Context/router pending
launch admission, and no consumer GPU kernel executes.

## Qualification

- `focused-final.log`: 10 passed, no failures.
- `runtime-final.log`: 1610 passed, 3 failed, 28 ignored. Exit status 101.
  The three unchanged baseline failures are `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
  `failed_session_end_is_explicit_and_terminal`, and
  `pre_native_telemetry_failure_is_returned_and_poisoned`, all at
  `authorized_execution.rs:1317`, `InspectSocket(PermissionDenied)`.
- `clippy-final.log`: strict all-target/all-feature Clippy for runtime and model passed.
- `doctests-final.log`: 79 passed (runtime 8 + 44, model 27).
- Minimal-feature check, formatting, source continuity and whitespace passed.
- `mi300x-access.log`: DNS failed before SSH execution; no remote files were created.

CPU commands use `--offline`, `CARGO_INCREMENTAL=0` and `CARGO_BUILD_JOBS=1`.
Runtime tests use `-p fe2o3-runtime --all-features --lib -- --test-threads=1`;
the focused run adds the `peer_compute_access` filter. Clippy uses both runtime
and runtime-model with `--all-targets --all-features -- -D warnings`. Doctests use
both packages with `--all-features --doc`; minimal check uses runtime with
`--no-default-features`.

## Formal Boundary

`final-signed-campaign/results.json`: all 33 phases pass. Three full proof runs
(original, exact relocation, original again) each verify the same 15 obligations
with zero errors. All 25 body-only mutants fail logically, not from timeout,
resource exhaustion, parse failure or foreign diagnostics. Calibration covers
4 gate-controller groups plus 9 inherited classifier groups. The pinned
190-file verifier closure matches before and after; all 6072 signed source inputs
match across the campaign. All 33 owned process groups are confirmed absent.

Shared executable ownership/access/resolve/action bodies prove exact identity,
monotonic resolution, readiness/failure conditions and predecessor-access expiry.
Native failure cannot reopen externally settled access. This is scalar evidence,
not proof of concrete custody maps, ancestry construction, native publication,
scratch lifetime or cancellation disposal.

## Remaining Integration

Pending admission requires bounded transitive/FIFO ancestry and edge-work limits,
exact existing producer-DMA custody admission, structurally deferred three-binding
inputs followed by real backing checks, durable mixed-depth history, retained
router gate state and stream indexes, and flush/drain integration without changing
observational poll/wait. See [composition status](../../runtime-peer-producer-composition-v1.md).
