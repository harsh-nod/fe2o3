# Opaque CPU Receipt Fixture

Development support for genuine lower receipt composition in a normal KFD
dependency build. This is not runtime Active/Pending integration, GPU execution,
new formal refinement, native cleanup, or HIP/HSA parity evidence.

## Source And Qualification

Signed source: `17338e0cd739a60a11cec84699e6b83eacfbc30c`.
Tree: `e6f4590240adda9e3725d7e4cf57ae0e0d6b47c3`.

| Check | Result |
| --- | --- |
| External fixture integration | 7 groups passed, including 4 isolated child runs |
| Full KFD library suite, all features | 1713 passed, 0 failed, 0 ignored |
| KFD doctests, all features | 42 passed, 0 failed, 0 ignored |
| Strict KFD all-feature/all-target Clippy | Passed |
| KFD and runtime default-feature compilation | Both passed |
| Workspace formatting and source continuity | Passed |
| Source signature verification | Passed |

The serialized `qualify.sh` exits zero with `aggregate 0`. No implementation was
edited during this run. Documentation was drafted while tests ran. The full KFD
suite took 1028.81 seconds; this is test duration, not a GPU performance result.
The earlier socket-inspection permission failure does not recur. No test was
waived. Runtime behavioral tests, other model suites, solver controls and native
workloads were not rerun.

Preparatory failures are retained: the first check attempted to use private
completion template/binder APIs; a gated owner helper corrected that boundary.
The first strict Clippy attempt found an unnecessary cast and lazy error closure;
both were fixed without lint allowances. The initial six passing groups were
strengthened after read-only swarm review into the seven final groups above.

## Architecture

The nondefault `fe2o3-kfd/cpu-runtime-fixtures` feature exposes an opaque,
documentation-hidden fixture on Linux/x86_64. Default features and native
execution bodies remain unchanged. It owns a CPU-only queue session, primary
and auxiliary completion arenas, separate production dispatch-generation owners,
CPU atomic signals, optional full-arena saturation custody, and event pins.
No raw session, owner, arbitrary receipt constructor, native address, receipt
clone, or direct slot-reset helper is exposed.

Ordinary submission reuses the existing classified multi-inflight logical-owner
helper. Recipe occurrences are minted by the production global identity owner,
not test identity constants. Pinned setup composes genuine bound-event,
publication and dispatch-owner operations. It is not the full native submission
facade. Acquire observation and release recycling run through the production
completion owner, with CPU atomic signal storage replacing Linux/GTT observation.
Signals alone cannot mark dispatch metadata completed or recycled.

The full-arena reservation consumes 8192 completion slots independently of the
64-epoch dispatch owner, so retry comes from real completion capacity exhaustion.
Draining observes and recycles its actual batch; it never resets bookkeeping
directly. Saturation requires an empty arena, so it does not yet induce ordered
successor retry with a live predecessor.

## Test Coverage

Seven external integration groups compile KFD as a normal dependency:

- Pending, Ready and recycle preserve the exact receipt; clean-state checks
  refuse both published and completed live custody.
- Out-of-order retirement and republication preserve still-live neighbors and
  issue distinct receipt identities. This is not a physical-slot reuse oracle.
- Foreign lane and borrowed foreign-receipt controls reject without mutation.
- Three real signal-capacity retries burn dispatch generations without live
  epochs, followed by genuine full-arena drain and successful reuse.
- A real event pin returns the same completed receipt with typed SignalPinned
  counts, unchanged custody/I/O snapshots and no premature recycle; releasing
  the pin permits retirement.
- Outer callback unwind preserves the deposited receipt, original panic payload
  and restored primary/AUX custody while poisoning the fixture.
- Same-callback terminal reentry cannot mutate bookkeeping after logical-owner
  epoch exhaustion, including attempted submit, poll, recycle and pin controls.

Both lane choices are covered for lifecycle, out-of-order, retry, pin and terminal
cases. Successful cases explicitly verify both dispatch owners, completion arenas,
dependency-owner idleness, empty fixture pins/saturation and reset CPU signals.
Untargeted lane snapshots are checked in nonterminal lifecycle scenarios.

Terminal cases run in isolated child processes and require explicit completion
markers, preventing a zero-test subprocess from falsely passing. They retain
terminal custody until process teardown and do not claim clean retirement.
Native process-poison helpers are unchanged; the tests assert fixture-local
terminal behavior, not direct observation of the process-global gate.

Snapshots preserve dispatch slots, completion slot records and ledger storage,
CPU signal values, counters and fixture pin identities. They do not capture the
full dependency-ledger contents or completion-owner phase. `same_custody` permits
the dispatch poison bit to change; ordinary snapshot equality does not.

## Remaining Work

Connect the fixture only at runtime initial/prepared submission, ordered
submission and completion I/O. Preserve real indexed Active/Pending owners,
custody predicates, exact lane handles, outer result gating, timestamps, phase
updates, profiling and logical commit. Native and CPU providers must be mutually
exclusive. CPU materialization must not select Scripted execution tags or
manually settle Pending retains.

Checked CPU teardown must require genuine receipt retirement before discarding
clean logical cache metadata. Native DATA detachment, prepared cancellation,
writes/readback, currentness faults and populated-arena ordered retry require
additional work. Shared-source caller refinement, protected Worker/compiler,
multi-device and distributed qualification, atomics/collectives and matched
HIP/HSA performance remain open. No milestone or accepted checkpoint is promoted.

No Verus, GPU workload, benchmark or remote cleanup operation was run in this
packet. The full runtime test suite was not rerun; runtime default compilation
is a separate check, not runtime behavioral qualification.

## Reproduction

At the signed source with its pinned Rust toolchain:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --locked -p fe2o3-kfd --all-features --test cpu_runtime_fixtures
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --locked -p fe2o3-kfd --all-features --lib
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --locked -p fe2o3-kfd --all-features --doc
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo clippy --locked -p fe2o3-kfd --all-features --all-targets -- -D warnings
cargo check --locked -p fe2o3-kfd --no-default-features
cargo check --locked -p fe2o3-runtime --no-default-features
cargo fmt --all -- --check
```

`receipts.tar.xz` includes commands, per-command statuses and times,
source/signature/continuity observations, full logs, preparatory failures and
source publication receipts. Verify it using `receipts.tar.xz.sha256`. The raw
campaign is `/home/harsh/.codex-tmp/fe2o3-cpu-receipt-fixture-20260928-Cmarc9Mt`;
it is frozen after archive creation. Documentation publication receipts are kept
separately and are not retroactively appended to this archive.

Archive SHA-256:
`4894d253f147e3fc53e95bf58626aeea329d5a675d4451677115b7e6418a0155`.
