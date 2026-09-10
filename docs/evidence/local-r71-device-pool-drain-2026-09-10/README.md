# Device Pool Limits And Copy-First Drain Qualification

Local MEM-2B and DRN-2A implementation evidence. Baseline:
`6a47a6611238a16cbb279876f70db9aba6538ff5`, branch
`codex/r65-runtime-drain-versions`. Changed source identities are retained in
`source-files.sha256`; raw logs and supporting files have a separate manifest.

## Scope

MEM-2B adds optional immutable device-cache byte/record limits to the existing
native SDMA pool. Its policy derives padded backing cost from exact mapped
native records, excludes host and checked-out buffers, and retains the existing
N2 debit through recycle/reuse. Overflow disposes the incoming idle device buffer
through the existing release path; uncertain disposal retains native custody.
Either zero limit disables device caching, not allocation. Runtime forwarding
precedes SDMA enable in both startup orders and preserves the native owner on
configuration failure. The [pool contract](../../runtime-device-pool-policy-v1.md)
records exact accounting scope and bounded, non-lock-free scan costs.

DRN-2A adds an eight-cell copy-first outstanding-work drain example, immutable
copy-only observations, an independent complete-output/membership checker and
a signed-source runner inheriting existing shared-host guards. Cells cover
queued/native-retained cutoffs, one/two streams and retained/dropped operation or
graph observers. Verification downloads belong to the accepted prefix; capture
itself issues no GPU work. The [qualification contract](../../runtime-drain-capture-qualification-v1.md)
distinguishes a retained native receipt from unfinished GPU work or overlap.

Cross-review found two concrete observation bugs before hardware acceptance:
pending work lives in active owners rather than the terminal submission table,
and pending polls/waits restore indexes without publishing again. R71 corrects
both the new copy observer and the older R66 membership assumption. Exact native
receipt, geometry and digest checks remain intact; no launch authority changes.
Synthetic R66 fixtures no longer manufacture pending terminal-table entries.
A real runtime async-copy path with a scripted native driver tests repeated
pending polls, exact history retention, completion and cleanup. It cannot mint
native evidence. Nested roster bounds now precede scans, and runtime forwarding
source tests isolate the exact startup functions and enable ordering.

The 37 added Rust tests comprise six model, five policy-adapter, eight native,
six runtime-forwarding, eight drain-observer, one actual scripted-copy and three
R66 membership tests. Some native/facade tests inspect source wiring; fake-native
tests use real private records/accounts and model loans but not Linux queues.
Seventeen new Python checker/runner tests use explicitly synthetic documents.
These tests do not fill a hardware acceptance cell.

The [swarm plan](../../runtime-a1-a2-swarm-plan.md) assigns next work across three
lanes: coherent host-GTT admission and native hooks, generated async authority,
then control/code budgets, persistent versions, capacity and measurements.
Primary owns shared integration, signed campaigns, executable refinement and
publication. Host pools, root/bootstrap/global closure, native/whole-executor
refinement, generated production cells and matched HIP/HSA performance remain open.

## Final Local Gates

| Gate | Result |
| --- | --- |
| GNU runtime all-feature/all-target tests, five crates | 2,180 passed; zero failed; five existing ignores; 48 harnesses |
| musl same runtime gate | 2,180 passed; zero failed; five existing ignores; 48 harnesses |
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
`raw/r71-final3-source-gate.json`. The harness uses pinned
`nightly-2026-04-03`, locked/offline dependencies, four Cargo jobs, disabled
incremental compilation and an unset stale `XDG_RUNTIME_DIR`. The observed
compiler version is retained. No Cargo manifest or lockfile changed. Raw logs
preserve original whitespace; final source-whitespace checks exclude these
verbatim `.log` artifacts. Both SHA-256 manifests were checked after retention.

The separate production metadata commands were:

```sh
cargo metadata --locked --offline --format-version 1 --no-default-features --manifest-path crates/fe2o3-runtime/Cargo.toml --filter-platform x86_64-unknown-linux-musl
python3 -B scripts/runtime_pure_rust_audit.py metadata --input /home/harsh/.codex-tmp/r71-production-metadata.json --root fe2o3-runtime
```

This is a dependency audit, not the actual qualifier ELF audit required by the
hardware runner. These local gates do not replace that later binary measurement.

## Authenticated Proof Gate

The full Verus suite completed with exit status zero: 60 positive sources,
1,362 obligations and 669 distinct expected-negative rejections. Pre/post source,
inventory and pinned release-closure checks, plus the exact transcript, passed.
Both release-closure measurements cover 190 files and 129,019,839 bytes. Command:

```sh
VERUS=/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus sh crates/fe2o3-runtime-model/verus/verify-verus.sh
```

R71 adds 15 obligations and ten mutations for bounded whole-roster cache policy,
exact cost/count, generation-insensitive alias rejection, overflow exclusion and
zero-limit behavior. The production-used Rust projection and Verus source have
reviewed, not mechanically linked, correspondence. Native layout extraction,
locks, cache mutation, N2 ownership/disposal, GPU execution, the drain observer
and whole-executor refinement remain outside these proofs. No new trusted body
or assumption was added; negative-checking rules are unchanged.

## Earlier Attempts

Failed and diagnostic attempts are retained rather than promoted:

- The first library check found a private forwarding helper inaccessible to its
  sibling compute module. Narrow `pub(super)` visibility fixed the integration.
- Direct Verus invocation first omitted `--crate-type lib`; the corrected command
  then exposed a zero-byte-limit lemma requiring explicit bounded unfolding.
  The corrected proof verifies 15 obligations without changing its premises.
- The first all-target check found a warmup release error type without an
  `Error` conversion. The qualifier now maps it to a fixed address-free failure.
- The initial full GNU gate found a fixture using 8192-byte alignment outside
  the native profile. Legal 4096-byte alignment still tests 4100 requested bytes
  rounding to 8192 backing bytes; production alignment constraints are unchanged.
- The next GNU gate exposed an older DRN-1A reply-credit test race. Result
  extraction can precede the owner's final reply-sender drop. The unchanged
  zero-credit assertion now follows owner shutdown/join; capture bytes remain
  charged before and after shutdown and refund only on result disposal.
- The next attempt passed GNU/musl tests, doctests, host and macro fixtures;
  Clippy rejected a redundant `len + 1 <= limit` test comparison. A diagnostic
  lint run also found three negated optional predicates and a collapsible match
  in the observers. Equivalent strict comparisons and match guards address
  these without changing acceptance conditions; the full source gate is rerun.

## Shared Hardware

One read-only MI300X `rocm-smi` query reported zero instantaneous utilization on
all cards, with 44 percent VRAM still allocated on card0. This snapshot is not a
selected-device idle admission or a queue/process census. No remote stage,
build, qualifier or GPU workload was started; no remote cleanup was needed and
foreign work was untouched. Signed pool/drain/coexistence qualification, matched
HIP/HSA performance, A1/A2 closure and runtime-wide parity remain open.
