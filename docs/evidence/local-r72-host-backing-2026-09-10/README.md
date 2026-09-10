# Ordinary Coherent Host Backing Admission

Local MEM-N1A implementation evidence. Baseline:
`ed01d85acdb37be01840cef79a38a16278c288da`, branch
`codex/r65-runtime-drain-versions`. Changed source identities are retained in
`source-files.sha256`; raw logs and supporting files have a separate manifest.

## Scope

R72 adds optional immutable admission for ordinary non-userptr coherent GTT
backing. The native owner derives page-padded host bytes and one allocation
record from the exact canonical layout, charges before native effects, and
retains that charge across CPU/GPU views, access, mapping, foundation loans and
buffer reuse. Only confirmed complete native disposal returns it. Pre-effect
rejection cancels an unissued reservation; uncertain post-effect outcomes and
configured-path panics retain or quarantine the debit. Panic handling preserves
the original payload without another fallible closing-currentness check.

The existing device budget and new host budget are independently configured
before covered allocations and queue certification in both runtime startup
orders. Releasing all buffers or restoring foundation ownership does not reopen
configuration. An unavailable usage snapshot is not evidence of refund. The
[host-backing contract](../../runtime-host-visible-backing-v1.md) records profile,
domain, lifetime and external-contract boundaries.

The 39 added Rust tests comprise nine account-adapter, four model, eighteen
native and eight runtime tests. Native fixtures use real private records,
accounts, foundation loans and move-only buffer transitions with a fake backend.
They cover pre-record failures, disposal/currentness errors and panics, exact
account/token substitution, signal ingress, buffer retagging and both loan
orders. Review added genuine closing-currentness panic, exact production
completion-arena geometry with separate byte/record saturation, and explicitly
dropped live-token tests. Confirmed disposal remains refunded even when a later
retake fails; live backing remains charged.

These fixtures do not construct Linux queues, initialize the signal ABI or
execute the Linux pool facade. The retake test explicitly composes the model
rejection with the existing quarantine policy. Runtime configuration tests and
startup source-wiring checks do not substitute for actual Linux constructors or
terminal-memory observations. Ordinary coherent bootstrap allocations are
covered by their exact profile; this does not account for all bootstrap,
userptr, executable, AQL, kernarg, metadata or global ownership.

## Final Local Gates

| Gate | Result |
| --- | --- |
| GNU runtime all-feature/all-target tests, five crates | 2,219 passed; zero failed; five existing ignores; 48 harnesses |
| musl same runtime gate | 2,219 passed; zero failed; five existing ignores; 48 harnesses |
| GNU runtime plus host doctests | 82 passed |
| musl runtime doctests | 71 passed |
| musl default direct-KFD host doctests | Ten passed |
| GNU all-feature host library | 207 passed; four existing ignores |
| Generated macro fixtures | Seven passed |
| Runner/checker Python suites | 151 passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Passed, warnings denied with existing scoped custody policy |
| Formatting and source whitespace | Passed |
| Production musl metadata audit | 43 packages; eight permitted build scripts |
| Workspace dependency policy and tests | 141 members, eight layers, 476 declarations; eight tests passed |
| CI-local test-gate regression and standalone lockfiles | Passed; 32 standalone manifests checked |

All sixteen final commands, exit statuses and timings are retained in
`raw/r72-final-source-gate.json`. The harness uses pinned
`nightly-2026-04-03`, locked/offline dependencies, four Cargo jobs, disabled
incremental compilation and an unset stale `XDG_RUNTIME_DIR`. No Cargo manifest
or lockfile changed. Documentation was finalized after the code gates and source
whitespace was rechecked. Raw logs preserve original whitespace; final source
whitespace checks exclude verbatim `.log` artifacts. The source manifest is
relative to the repository root; the retained-file manifest is relative to this
directory. Both manifests were checked after retention.

The separate production metadata commands were:

```sh
cargo metadata --locked --offline --format-version 1 --no-default-features --manifest-path crates/fe2o3-runtime/Cargo.toml --filter-platform x86_64-unknown-linux-musl
python3 -B scripts/runtime_pure_rust_audit.py metadata --input /home/harsh/.codex-tmp/r72-production-metadata.json --root fe2o3-runtime
```

This dependency audit is not the actual qualifier ELF audit required by a signed
hardware campaign. No new production dependency was introduced.

## Authenticated Proof Gate

The full Verus suite completed with exit status zero: 61 positive sources,
1,367 obligations and 678 distinct expected-negative rejections. Pre/post source,
inventory and pinned release-closure checks, plus the exact transcript, passed.
Both release-closure measurements cover 190 files and 129,019,839 bytes. Command:

```sh
VERUS=/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus sh crates/fe2o3-runtime-model/verus/verify-verus.sh
```

R72 adds five obligations and nine mutations for the bounded ordinary span,
exact padded host-byte and record coordinates, zero other coordinates and
reserve/release arithmetic. The production Rust projection and Verus companion
have reviewed, not mechanically linked, correspondence. Native layout
extraction, actual disposal, private token/record correspondence, locks and
whole-executor refinement remain outside these proofs. No new trusted body or
assumption was added; negative-classification rules are unchanged.

## Earlier Attempts

Failed and preliminary attempts are retained rather than promoted:

- The first focused host build overlapped in-progress module wiring and failed
  with E0583 before the new test file existed. It produced no test result. The
  final complete source gate covers all eighteen native tests.
- The initial full proof run was deliberately interrupted after noticing that
  registration still had the old transcript pin. The interruption produced a
  wrapper failure and missing temporary-log diagnostics; it is not a completed
  R28 theorem failure or accepted negative classification. The wrapper and its
  matching children were confirmed absent before the corrected full run.
- Preliminary focused model, host and production-check logs are retained, but
  the complete final gates are the acceptance record for the frozen source.

## Shared Hardware And Remaining Work

One read-only MI300X query observed selected card1, UID `0xab83d2ffef0d3cdf`,
BDF `0000:26:00.0`, at 52 percent GPU use and 20 percent allocated VRAM.
`raw/r72-mi300x-readonly.json` contains extracted selected-card fields, not a raw
full census or hardware qualification. No remote stage, build or workload was
started; no remote cleanup was needed and foreign work was untouched.

Signed Linux admission/pool-pressure qualification, broader GTT profiles,
host-cache ceilings, root/device/Context domains, control/code budgets,
persistent versions, generated async authority, native capacity and
whole-executor refinement remain open. The
[next-wave dispatch](../../runtime-a1-a2-next-wave.md) assigns those packets.
Matched HIP/HSA performance, A1/A2 closure and runtime-wide parity are not
established by these local results.
