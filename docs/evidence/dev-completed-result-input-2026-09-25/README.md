# Completed Result To Read-Only Input

The consuming `GeneratedRuntimeReadSlice::from_charged_result` API preserves
completed typed storage and its original result credit while the existing
argument path admits and encodes the next immutable input. See the
[ownership and accounting contract](../../runtime-charged-generated-results-v1.md#completed-input-chaining).

## Scope

This is host-data implementation and CPU development evidence. Conversion avoids
one caller-side typed clone/allocation; ordinary preparation still performs
linear encoding into a new byte buffer. No benchmark, hardware execution,
formal adapter refinement, Worker authority, in-flight dependency transfer or
device-resident graph dataflow is claimed. The production result destructor and
credit ledger are unchanged. A1/A2 and accepted runtime lane checkpoints remain
unchanged.

## Validation

The final fourteen-phase campaign passes from signed
`49aa95f33b989974d3eedd4ca43344f18021b468`, which includes the implementation in
`06fc26d1c`. The [final results](retained/cpu-final/results.json) retain:

| Check | Result |
| --- | --- |
| GNU and musl host libraries | 175 tests each; no failures, ignores or filtered tests |
| Host doctests | 26 passed |
| Resource accounting and macros | 23 and 59 tests passed |
| GNU runtime, all features | 1,413 passed; 22 hardware-only tests ignored |
| Downstream generated arguments | Positive fixture compiles; reuse, clone and storage-extraction negatives reject with the expected diagnostic |
| Runner calibration, strict all-target Clippy and workspace formatting | Six calibration tests and both checks pass |

The [opening](retained/cpu-final/source-before.json) and
[closing](retained/cpu-final/source-after.json) source records match all 5,953
inputs and the same signed commit. The runner and checks are unchanged between
attempts. The final run reuses the task-owned build cache; it is not a clean-room
tool/dependency closure audit.

The [first run](retained/run.stdout) remains rejected: strict Clippy found an
inherited `items_after_test_module` diagnostic in `production_application.rs`.
That run did not reach formatting or the final source bracket. The correction
moves the unchanged test module to the end of that file without changing its
gate, production behavior, assertions or lint policy. The complete failed run,
its diagnostics and the earlier focused development logs remain retained.

Nine new unit tests cover typed pointer/bit preservation across ownership and
thread moves, unchanged credit before binding, independent shared-account peak
admission, byte/member exhaustion, pre/post-encoding errors and panics, payload
and type/index rejection, empty member accounting, legacy encoding, private
read-only/custody/mapping guards and actual CPU preparation/projection. Synthetic
decoding and structural HSACO fixtures grant no native authority. Injected
`Error::Allocation` and bind-closure panics are not allocator failure or scalar
encoder-panic tests. Refund-fault behavior is inherited, not newly fault-injected.

A downstream compiler-generated argument fixture consumes completed data through
the public API and genuine executable parameter. Compile failures retain exact
Rust error classification for result reuse, cloning and raw storage extraction.

## Retention And Cleanup

All 123 scratch artifacts are preserved byte-for-byte under `retained/` and
listed in [retention.json](retention.json), including both signed-source checks.
The [before](cleanup-before.json) and [after](cleanup-after.json) cleanup records
check all 27 recorded phase groups absent and retain source continuity, exact
owned path identities and path-accounted allocated bytes. Only the owned local
scratch and Cargo target were removed, reclaiming 6,813,011,968 allocated bytes.
Independent `lstat` and parent-listing checks confirm both paths absent, and all
retained hashes still match. No remote resources or GPU jobs were created.

`SHA256SUMS` covers the packet files other than itself. These development
receipts are not a relocated replay audit, full toolchain authentication,
global proof registration or native qualification.

## Reproduction

From a committed source tree with the pinned Rust toolchains/dependencies and an
existing disposable Cargo target:

```sh
python3 -I -B docs/evidence/dev-completed-result-input-2026-09-25/run.py --output OUTPUT --target CARGO_TARGET
```

The runner authenticates the inherited process controller before executing the
same bytes, retains phase commands/logs and process-group cleanup observations,
and checks source continuity. Fixtures use a child of the disposable target,
not the repository's shared fixture target. Its source inventory and warm-target
reuse are development boundaries, not a complete tool/dependency authentication
or relocated-packet replay audit. Hardware-only ignores and excluded legacy
HIP/HSA host features are not execution passes.
