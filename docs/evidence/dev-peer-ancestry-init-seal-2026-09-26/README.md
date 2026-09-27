# Bounded Peer Ancestry And Sealed Compute Initialization

Development evidence only. Public pending-peer KFD launch admission remains
closed. This packet does not establish HIP/HSA parity, GPU execution, native
XGMI/compute composition, machine-code refinement or a performance improvement.

## Signed Sources

- Ancestry implementation: `ab0d5708a1a24dfebe408311d4b8e6f4af1314f7`.
- Initialization seal and qualification source:
  `921ccfd023da3ab35d7ac2fb4c537b59eb4f95d6`.
- Both SSH signatures verify as `harmenon@amd.com`.
- Raw directory: `/home/harsh/.codex-tmp/fe2o3-ancestry-seal-20260926-lU2MZ5CF`.
- Frozen receipt archive: `receipts.tar.xz`, SHA-256
  `6a558dd70b90edf70651b56125eb3644aec70642b9226695160a579bd8abe9ad`.

## Implementation

The router captures an iterative, deduplicated predecessor closure with a sorted
node index and flat original event/submission rosters. Explicit success roots
and the original cooperative FIFO predecessor remain distinct. Live traversal
authenticates earlier directed parents, same-stream FIFO identity and depth;
terminal roots are history boundaries, not a requirement to resurrect released
events, grandparents or source allocations. Original event identity remains
checked after settlement. Later stream tails cannot extend an accepted capture.

Capture has separate limits of 32,768 nodes, 262,144 explicit/implicit edges
(including stored terminal history), and depth 256. Private permits are bounded
by 65,536 entries. Every cooperative allocation owner must be in the captured
closure; unrelated read siblings still reject. Full-closure retains precede
child entry. Rejected/quiescent admission failures refund them; Terminal/unwind
retain them. The captured success depth supplies the child admission floor.
Native-only requests do not allocate ancestry storage.

`InitializedAfterDispatch` now takes an opaque move-only payload instead of a
bare allocation. Existing internal authenticated completion/recovery preserves
the payload. External callers cannot construct it, replace its allocation,
clone it, use the private recovery constructor or reuse it after normalization.
Direct external construction with bare storage is intentionally no longer
source-compatible: keeping it would retain the initialization-authority hole.

## CPU Qualification

- `focused-final.log`: 119 peer-related tests passed, including 10 new ancestry
  tests and one new transitive scripted-DMA test.
- `runtime.log`: 1621 passed, 3 failed, 28 ignored; exit 101. The same three
  baseline failures occur at `authorized_execution.rs:1317` with
  `InspectSocket(PermissionDenied)`: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
  `failed_session_end_is_explicit_and_terminal`, and
  `pre_native_telemetry_failure_is_returned_and_poisoned`.
- `kfd-focused.log`: 81 passed using the exact built KFD unit-test binary with
  four threads; its SHA-256 is recorded. Coverage includes initialization,
  completion/recovery, cancellation, replay and coexistence controls.
- `kfd.log`: the original full serial attempt reached its 1200-second cap;
  exit 124 is not a passing suite. Its receipt is preserved.
- `kfd-full-parallel.log`: 1643 passed, 1 failed, 0 ignored; exit 101 after
  1492.90 seconds, using the same complete binary with eight threads. The failure
  is `credential_bound_channel_accepts_typed_failure_before_publication` at
  `target_debug_telemetry_v2.rs:1173`, returning `SocketAdmission`. An isolated
  one-thread rerun reproduces it. That telemetry source is unchanged by the two
  implementation commits; no unrestricted-host pass or underlying syscall errno
  is claimed. All construction, cleanup and unwind matrices completed.
- `model.log`: 1034 passed, 19 ignored.
- `doctests.log`: 116 passed (KFD 37, runtime 8 + 44, model 27), including
  all six new initialization-authority compile-fail controls.
- Strict all-target/all-feature Clippy for KFD/runtime/model, minimal runtime
  compilation, formatting, whitespace and source continuity passed.
- Final source and exact KFD binary hash checks passed after the complete suite.

`qualify.sh` records Cargo commands, per-command statuses and source hashes.
Cargo uses `--offline`, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=1`; complete runtime
and model suites use one test thread. Separate `.command` files record the
additional direct KFD test-binary invocations. The serial timeout is retained
independently of the focused and parallel results.

The transitive copy test admits genuine private child compute custody and drives
two scripted producer copies while retaining both ancestors. It covers source-
and destination-endpoint access; the source-device case is private-access
coverage, not the public destination-device producer rule. The consumer stays
gated and is cancelled. No consumer GPU kernel executes. Other tests cover
diamonds, independent resource bounds, released history, changed original event
identity, unrelated owners, captured FIFO ordering, terminal depth and uncertain
admission rollback.

## Formal Boundary

`gate-campaign/results.json`: all 33 phases pass against the signed qualification
source. Three full proof runs each verify the same 15 obligations with zero
errors. All 25 body-only mutants fail logically under the authenticated
classifier. Before/after verifier-closure checks pass, all 6074 source inputs
remain identical, and all 33 owned process groups are confirmed absent.

This requalifies the shared scalar gate ownership/access/resolve/action bodies.
It does not prove the new ancestry construction, concrete custody maps, native
publication, initialized-byte authority or machine-code behavior. Rust privacy
compile-fail tests are not native initialization proofs.

## Access And Remaining Work

`mi300x-access.log` records DNS failure before SSH execution; no remote artifacts
were created. `source-publication.json` records the verified implementation push
to `harsh-nod` and failed DNS-resolution push to `powderluv`, whose branch remained
at `2307ff7d88e093019208e2f8391e498956f415f6` at readback.

The next native prerequisite is completion-backed initialized-byte coverage for
real SDMA zeroing, multipart copies and partial overwrites, with separate content
digest handling, pool-generation reset and sealed quiescent conversion. Runtime
booleans, generic owner bookkeeping completion and peer success cannot mint it.
Keep SDMA-initialized storage distinct from completed-dispatch provenance through
prepublication cancellation/recovery. Padded typed layouts remain separate.

Router work still includes exact already-active DMA owner admission, structural
three-binding deferral followed by actual backing checks, durable mixed-depth
history, retained gate state and stream-indexed flush/drain progress. Public
poll/wait must remain observational. See [composition status](../../runtime-peer-producer-composition-v1.md).
