# Owner-Local Operations

Local GEN-2B-1 implementation evidence. Baseline:
`f00e24d5e454b4575285170f17d8612c3bae0a49`, branch
`codex/r65-runtime-drain-versions`. The [contract](../../runtime-owner-local-operations-v1.md)
defines the private driver boundary and remaining generated integration.

## Scope

R75 separates Send factory transport from owner-local operation drivers.
Existing ordinary launch/copy/frozen-request APIs use that path. Progress and
operation-capacity checks precede inert materialization; the default local
factory requires owned shutdown. The ordinary handle-only adapter opts out,
preserving the Context-returning Send API. No unsafe auto-trait or native
publication path is added.

The engine caches stream identity and borrows the installed driver during
advance. Pending, terminal and panicked custody remains in the bounded roster.
The owned engine keeps that roster through Context cleanup and native shutdown,
then drops drivers before the backend on success or retains both on failure.
Replies are detached, resolved and disposed independently of retained drivers.
Ordinary never-called host callbacks are discarded on stop, without disposing
Context-owned native resources.

Sixteen new CPU tests use Rc-holding drivers and a thread-bound mock backend.
They cover construction/use/drop affinity, normal cleanup ordering, four
cleanup/finalizer failure modes, factory/advance/outer-loop panic, terminal
retirement, observer drop, mixed capacity and reclamation, retired-stream
flushing, reply/queue/closed admission, exact notification and post-detachment
rejection panic. Existing cancellation races now materialize on the owner;
queued-drop tests still exercise unmaterialized factory disposal.

Three read-only agents reviewed transport/API compatibility, native-owner
retention and resource/reply lifetime. Their final review had no remaining
findings. These are mock-owner and host-storage observations, not Linux/GPU
custody or R73 charged generated-result integration tests.

## Final Gates

| Gate | Result |
| --- | --- |
| GNU five-crate runtime all-feature/all-target tests | 2,238 passed; five existing ignores; 48 harnesses |
| musl same runtime gate | 2,238 passed; five existing ignores; 48 harnesses |
| GNU runtime plus host doctests | 86 passed |
| musl runtime / host doctests | 71 / 14 passed |
| GNU all-feature host library | 233 passed; four existing ignores |
| musl default direct-KFD host library | 116 passed |
| Focused async suite | 186 passed, including sixteen new owner-local cases |
| Generated macro fixture harness | Seven passed |
| Runner/checker suites | 151 passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Passed with warnings denied |
| Formatting and whitespace | Passed |
| Dependency policy and tests | Passed; 141 members, eight layers, 478 declarations; eight tests |
| CI-local test-gate and standalone lockfiles | Passed; 32 standalone manifests |
| Production musl metadata | Passed; 43 packages and eight permitted build scripts |

All seventeen final source commands, statuses and timings are retained in
`raw/r75-final2-source-gate.json`; aggregate results are in `test-summary.json`.
The gate uses pinned `nightly-2026-04-03`, locked/offline dependencies, four Cargo
jobs, disabled incremental compilation and an unset stale `XDG_RUNTIME_DIR`.
Before/after and retention checks matched all 5,580 tracked/nonignored
non-documentation input identities. Documentation was finalized afterward.
No manifest, dependency or lockfile change was needed.

## Earlier Attempts

An initial development compile found two legacy race harnesses moving a driver
to another thread. They now move a Send factory and materialize on the owner,
preserving their original cancellation/completion barriers. That first raw
terminal transcript is not retained in this packet.

`r75-focus-b.log` retains the first completed test run: 179 passed and one
failed. Stopping before submission unnecessarily retained 72 bytes of ordinary
snapshot payload. The fix disposes only the still-unissued ordinary callback
after reply detachment, while keeping driver/native custody intact.

`r75-focus-c.log` retains the next run: 185 passed and one failed. Moving registry
allocation before Context construction reversed successful destructor order.
An allocation-free rebind after Context construction restores driver-before-
backend disposal while keeping allocation before native construction.
`r75-focus-d.log` passes all 186 focused async tests after both corrections.
Failed logs are retained as failures, not promoted to final acceptance.

The first broader sequence, `r75-final`, passed its runtime, host, doctest,
fixture, Clippy and runner checks, then stopped on one startup-declaration line
wrap in the formatting gate. The corrected source is checked again by the
complete `r75-final2` sequence; earlier results are not substituted for its
source-identity checks.

## Proof And External Boundaries

No Verus source or theorem was added or changed, and the solver was not rerun.
Runtime-model, resource-accounting and completion sources are checked unchanged
from signed R73. Its property-specific results remain in the
[R73 record](../local-r73-charged-results-2026-09-10/README.md), not a proof of
this new Rust adapter. The current negative-quality/inventory check passes
all 686 files:

```sh
python3 -I crates/fe2o3-runtime-model/verus/check-negative-quality.py crates/fe2o3-runtime-model/verus/negative crates/fe2o3-runtime-model/verus/verify-verus.sh
```

Private factories must not issue native work. Rejection must detach/resolve its
unique reply infallibly before fallible handling; containment cannot recover a
producer hidden by a broken pre-detachment implementation. Post-detachment
panic tests do not prove progress for arbitrary callbacks. Record bounds do not
close argument, factory, driver, waker or aggregate byte accounting.

The production musl metadata audit passes 43 packages and eight permitted build
scripts. This is not an actual qualifier ELF audit:

```sh
cargo +nightly-2026-04-03 metadata --locked --offline --format-version 1 --no-default-features --manifest-path crates/fe2o3-runtime/Cargo.toml --filter-platform x86_64-unknown-linux-musl
python3 -B scripts/runtime_pure_rust_audit.py metadata --input /home/harsh/.codex-tmp/r75-production-metadata.json --root fe2o3-runtime
```

## Retention And Next Work

`source-files.sha256` is relative to the repository root;
`retained-files.sha256` is relative to this evidence directory. The staged
post-retention whitespace check passes with `raw/**` excluded. The unfiltered
check reports terminal blank lines and trailing spaces in verbatim Cargo logs;
those logs are not edited to satisfy whitespace checks.

No SSH session, MI300X stage, build or workload was started; no remote cleanup
was needed. GEN-2B-2 retained-device/Context preparation, exact persistent
publication/readback, R73 charged results and the production compiler handoff
remain open. No native acceptance, whole-executor proof, full HIP/HSA parity or
performance improvement is established by this packet.
