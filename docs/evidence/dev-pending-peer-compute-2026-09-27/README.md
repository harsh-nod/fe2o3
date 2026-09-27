# Pending Directed-Peer Compute Admission

Development checkpoint, not #182, A1/A2, full HIP/HSA parity, concrete formal
refinement, hardware qualification, or performance acceptance.

## Source And Receipts

Implementation: `2e6f65fab2f70af2a4af533c3fed2a8a1ef525e0`, SSH-signed and verified.
Root implemented and tested; two read-only agents reviewed admission, ownership,
progression, failure handling, and algorithmic complexity.

Frozen raw directory:
`/home/harsh/.codex-tmp/fe2o3-peer-launch-20260927-uIPsHhFG`.
[Receipts archive](receipts.tar.xz), SHA-256:
`821cbda8c79134fb4d0b0894a6786e3b26e0646496af22547d5719315cbda3a1`.
Archive comparison passed. The packet includes the signed source patch, source
and test-binary hashes, compiler identity, commands, and development/final logs.

## Implementation

- The multi-device producer-aware launch SPI accepts pending directed peer-copy
  producers, including copies with private native DMA already published. Pending
  ordinary cooperative copies and unrelated allocation owners remain rejected.
- Immutable captured ancestry and exact child-route stamps are retained before
  child entry. Explicit roots require success; the captured stream predecessor
  requires quiescence. Later cooperative stream tails are not recaptured.
- Private DMA admission is an opaque allocation/submission-pair roster, not a
  fake native event. The adapter authenticates producer, selected leg, endpoint,
  stream, scratch, chunk range, active owner, actual custody roster, publication
  index, and backing phase. Normal launch validation precedes this inspection.
- Exact R/R/W candidates can wait for a peer on any slot, including ordered
  write-after-write on the output. This grants waiting custody, never initialized
  storage or launch authority. Conversion and actual backing admission still run
  after all gates settle.
- Poll/wait refresh peer observations without driving cooperative copies. Flush
  and drain progress captured roots and retained native prefixes. Exact-head
  rechecks prevent progression of later native work. Target failure is preserved;
  cancellation releases the consumer without cancelling producer DMA.
- Stream-indexed retirement is linear in that stream's retained consumers.
  Completed native compute/SDMA records preserve explicit dependency depth.

## Qualification

| Lane | Result |
| --- | --- |
| Final all-feature runtime library | 1636 passed, 3 existing failures, 28 ignored |
| New scripted test groups | All 5 passed in the final full run |
| Intermediate peer-launch filter | 20 passed; final full run supersedes it |
| Runtime doctests | 52 passed |
| Strict all-feature/all-target runtime Clippy | Passed |
| No-default-feature runtime check | Passed |
| Formatting, whitespace, source continuity, signature | Passed |

The three failures remain `authorized_execution::tests` cases
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`:
`InspectSocket(PermissionDenied)` at unchanged `authorized_execution.rs:1317`.
They are failures, not passing or waived qualification.

New tests exercise pending/read-DMA/write-DMA admission, partial-copy bytes and
untouched suffixes, observation-only calls, expired drain, released events,
captured cooperative/native tails, cancellation during published DMA, explicit
failure versus FIFO-only failure, completed depth, all three R/R/W slots, and
endpoint-scoped admission with corrupted native custody. Scripted compute does
not prove GPU kernel outputs. Corruption inspection disarms CPU-only fixtures
after assertions; it is not terminal native cleanup or refund evidence.

Two fixture mistakes and their corrected reruns are recorded in `commands.md`.
The first used a full-H2D-ready shape with a normal-retirement script; the later
native-tail fixture initially lacked native backing and was correctly rejected.
Only the test file changed after the first final source hash manifest.

## Open Work

No new native KFD suite, Verus campaign, ELF audit, or GPU benchmark ran. Existing
gate proofs do not establish concrete router refinement. MI300X DNS failed
before connection, so no remote artifacts or jobs required cleanup.

Broader public-router mixed-graph, fanout, occupied-lane, and terminal/unwind
qualification remains. This does not automatically admit arbitrary shared-buffer
composition through native intermediates, compute-to-peer submission, or native
XGMI compute composition. Storage-origin hardware qualification still needs a
bounded profile and matched HIP/HSA first-conversion versus steady-replay runs;
CPU path counters are not speed evidence. Worker, device-language, atomic and
collective, distributed execution, and release acceptance work remains tracked
under the full objective and open issue #182.
