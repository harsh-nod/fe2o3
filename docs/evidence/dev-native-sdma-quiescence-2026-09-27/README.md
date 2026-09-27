# Native SDMA Ancestry for Typed Compute

Development checkpoint, not #182 closure, A1/A2 acceptance, full HIP/HSA parity,
formal refinement, native hardware qualification, or performance acceptance.

Implementation: `ce6075a75e80fc96d5d4bf3c27cc6f449a369d23`, SSH-signed and verified.
The primary agent implemented and tested; two read-only agents reviewed
ownership, cancellation, initialized-storage eligibility, complexity, and fixtures.

## Implementation

- Compute admission independently retains transitive ordinary SDMA owners of its
  bindings through the existing native quiescence roster. Explicit success
  dependencies and FIFO/quiescence-only ordering retain their distinct failure
  semantics. Same-stream SDMA intermediates are not treated as uninterruptible
  compute FIFO suffixes: cancelling them cannot discard an older live owner.
- Native ownership authentication checks both endpoints, exact custody, stream
  identity/index, ranges, initialization metadata, phase, published index, window
  custody, and exact in-flight markers. Corrupt records fail before admission.
  Known foreign FIFO predecessors reject; the established missing-predecessor
  accepted-custody terminal path remains unchanged.
- Typed three-binding launches can wait on exact ordinary H2D, D2H, and D2D
  owners. This grants waiting eligibility, not a ready-storage receipt. After
  quiescence, publication rechecks actual restored storage and performs existing
  Device-to-InitializedStorage conversion where needed. Partial copies are
  supported without inventing authenticated full-H2D content.
- Private peer DMA still requires its private permit. An ordinary event naming
  its child submission or reuse of its private stream cannot bypass that boundary.
- Admission validates each distinct custody/stream index once, then uses sorted
  exact-owner lookup. Local memo indexes are fallibly allocated and bounded by the
  admitted binding/owner rosters. Plain compute with no SDMA owners does not
  allocate these indexes. This removes repeated quadratic index validation;
  it is not evidence of measured latency or bandwidth improvement.

## Qualification

| Lane | Result |
| --- | --- |
| Final all-feature runtime library | 1662 passed, 3 existing failures, 28 ignored |
| Six added test groups | All passed in final full run |
| Runtime doctests | 52 passed |
| Strict all-feature/all-target runtime Clippy | Passed |
| No-default-feature runtime check | Passed |
| Formatting, whitespace, source continuity, signature | Passed |

The library suite is not green. The unchanged `authorized_execution::tests`
failures are `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, all failing
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`.

Coverage includes explicit/FIFO cancellation polarity, independent ancestor
retention after event release, exact refund, unrelated-owner rejection, all three
copy directions, same/cross-stream dependencies, nonzero native payload and owner
preservation, successful shared-endpoint and disjoint intermediates, thirteen
metadata-corruption controls, and both event/FIFO private-peer bypass attempts.
Successful compute paths report zero user-data materializations. This is scripted
CPU scheduling/custody evidence, not GPU kernel output or memory-order evidence.
Corrupted CPU metadata is restored only for test teardown, not terminal recovery.

The archived development logs retain a corrected D2D script mismatch and an
intermediate full-suite abort caused by the overly broad FIFO check. The serial
diagnostic identified the existing accepted-custody regression; the final run
passes that unchanged test. See `commands.md` for the exact sequence and scopes.

## Receipts

Frozen raw directory:
`/home/harsh/.codex-tmp/fe2o3-native-sdma-quiescence-20260927-P96wI2fb`.
[Receipts archive](receipts.tar.xz), SHA-256:
`0f0bab4c79155053053262f3624e45deaa3a043d5fa7f300f46a71be2b06b788`.
Archive comparison and all receipt checksum checks passed.
The archive contains final and development logs, source patch/checksums,
test-binary/compiler identity, and the signed source commit/signature receipt.

## Open Work

Shared-body quiescence/retain proofs and concrete adapter refinement remain open;
existing peer-gate proofs do not cover the new SDMA authentication or roster.
Deferred active compute output/WAW, broader mixed DAG/fanout and multi-window
qualification, native XGMI composition, and matched first-conversion versus
steady-replay KFD/HIP/HSA benchmarks remain open. The broader Worker,
device-language, atomics/collectives, multi-device/distributed, profiling, and
release gates remain part of the full objective.

MI300X DNS failed before connection. No remote jobs or files were created.
No new Verus, native KFD, ELF, GPU, distributed-host, or performance campaign ran.
