# Charged Generated Result Storage

Local GEN-2R implementation evidence. Baseline:
`96212b87bb67eef0dc4e8f6e0137ebf3c35e2d37`, branch
`codex/r65-runtime-drain-versions`. Changed source identities are retained in
`source-files.sha256`; logs/supporting files have a separate retained manifest.

## Scope

R73 adds a separate charged result owner to the existing generated argument
packing path. Complete-roster R70 admission precedes encoding; each typed output
keeps its original caller seed and its full conservative encoded-plus-typed
debit through disposal. Read-only returned storage has a separate byte/member
debit. Exact binding order, scalar, extent, access and private readiness-gate
identity prevent substitution between equally sized result accounts.

The private decoder validates all buffers and custody before writing outputs,
disposes returned encoded storage and then publishes one ready gate. Typed
storage is disposed before credit refund. Dropping an observer does not cancel
its producer. No public raw decoder/storage escape, result cloning, completion
authorizer or execution method is added. The legacy bare-Box route stays separate.
The [contract](../../runtime-charged-generated-results-v1.md) specifies the
public API, precise byte/membership bounds and private integration obligations.

Twenty new charged-path host tests and three model tests cover the production
packing/decoder helper and cost/shape guards. Two release-review additions reach
actual seed-slot mutex poisoning and lower duplicate-argument packing rejection
after every output is bound. A third review addition proves polling returns
without waiting behind the seed mutex, using bounded thread cleanup rather than
a benchmark threshold. Four new generated macro negative cases and four
compile-fail doctests cover borrowed/clone/raw-storage/decoder escapes. Existing
eleven legacy owned-argument tests remain unchanged in behavior.

Synthetic returns use production private helpers after disposing original input
storage. Injected `Error::Allocation` is not an allocator failure; a panic after
decoding an output is not an in-scalar decoder panic; a joined ordinary producer
thread is not runtime-owner shutdown. These tests do not authenticate a current
compiler deployment, native generations, Linux completion or device execution.

## Final Local Gates

| Gate | Result |
| --- | --- |
| GNU runtime all-feature/all-target tests, five crates | 2,222 passed; zero failed; five existing ignores; 48 harnesses |
| musl same runtime gate | 2,222 passed; zero failed; five existing ignores; 48 harnesses |
| GNU runtime plus host doctests | 86 passed |
| musl runtime doctests | 71 passed |
| musl default direct-KFD host doctests | 14 passed |
| GNU all-feature host library | 227 passed; four existing ignores |
| musl default direct-KFD host library | 110 passed |
| Generated macro fixtures | Seven passed, including four new negative cases |
| Runner/checker Python suites | 151 passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Passed, warnings denied |
| Formatting and source whitespace | Passed |
| Production musl metadata audit | 43 packages; eight permitted build scripts |
| Workspace dependency policy and tests | 141 members, eight layers, 478 declarations; eight tests passed |
| CI-local test-gate regression and standalone lockfiles | Passed; 32 standalone manifests checked |

All seventeen final commands, exit statuses and timings are retained in
`raw/r73-final2-source-gate.json`. The harness uses pinned
`nightly-2026-04-03`, locked/offline dependencies, four Cargo jobs, disabled
incremental compilation and an unset stale `XDG_RUNTIME_DIR`. SHA-256 identities
for all 5,569 tracked/nonignored non-documentation inputs were captured before
the gate and rechecked afterward and at retention. Documentation was finalized
after code gates; final source whitespace was rechecked. Verbatim `.log` files
are excluded from source-whitespace checks, not altered to hide their contents.

The host manifest adds direct edges to existing workspace resource-accounting
and runtime-model crates. The 22 affected real-host lockfiles each add only those
two dependency entries; structured before/after checks confirm no new packages
or unrelated manifest/lock changes. The stub host fixture is unchanged. The
production dependency audit is unchanged in size, not an actual qualifier ELF
audit. Its commands were:

```sh
cargo metadata --locked --offline --format-version 1 --no-default-features --manifest-path crates/fe2o3-runtime/Cargo.toml --filter-platform x86_64-unknown-linux-musl
python3 -B scripts/runtime_pure_rust_audit.py metadata --input /home/harsh/.codex-tmp/r73-production-metadata.json --root fe2o3-runtime
```

`source-files.sha256` is relative to the repository root;
`retained-files.sha256` is relative to this directory. Both were checked after
final retention. Earlier candidate snapshots and failed attempts remain separate
from `r73-final2` acceptance.

## Authenticated Proof Gate

The full suite completed with exit status zero: 62 positive sources, 1,374
obligations and 686 distinct expected-negative rejections. Pre/post source,
inventory and pinned release-closure checks, plus the exact transcript, passed.
Both release-closure measurements cover 190 files and 129,019,839 bytes. Command:

```sh
VERUS=/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus sh crates/fe2o3-runtime-model/verus/verify-verus.sh
```

R73 adds seven obligations and eight mutations for production-used cost/shape
guards: two-copy typed and one-copy read costs, overflow rejection, admissible
empty members, zero unrelated coordinates and exact length/capacity/access.
The Rust/Verus correspondence is reviewed, not mechanically extracted. Arc
identity, mutex/Box/ledger adapters, complete decoder refinement, GPU completion
and the whole executor remain outside these proofs. R70 supplies the existing
batch-reservation arithmetic. No new trusted body or assumption was added;
negative-classification rules are unchanged. The later tests and mutex-polling
adapter change do not change the authenticated proof sources or guards; that
adapter remains outside their refinement scope.

## Earlier Attempts

- The third focused host run passed 27 tests and failed one newly added
  scalar-only fixture because it used an invalid empty ABI. The test was
  corrected to use a valid scalar-only descriptor; production ABI admission
  was not weakened.
- The first lock-refresh helper stopped on a fixture's stub host package,
  which intentionally has no real host dependency roster. The corrected helper
  resolves the actual host manifest and checks only real-host consumers.
- Preliminary focused host and Clippy runs are retained separately. Final
  current-source gates, not earlier smaller test rosters, are acceptance evidence.
- The first complete 17-gate run passed on the earlier blocking-observer
  candidate. A new regression then deliberately failed against that candidate:
  `try_take` waited for a held seed mutex. Its ten-second guard released/joined
  both threads before the assertion. `try_lock` now returns `None` on transient
  contention and preserves poison errors. This is not a Future/wakeup mechanism;
  GEN-2B must arrange wake/retry separately. The final source gate supersedes
  the earlier candidate rather than mixing their source identities.
- A standalone final inventory recheck initially omitted the checker's required
  directory/runner arguments and exited with usage status 2. The corrected
  invocation is retained separately; the authenticated full proof run already
  used the correct invocation and was unaffected.

## Hardware And Remaining Work

No SSH, MI300X stage, build or workload was started for this packet. No remote
cleanup was needed. Earlier busy-device observations are historical and are not
new hardware evidence for this source.

GEN-2A owner-local invocation preparation and GEN-2B Context/native publication,
retirement and decoder authority remain open. The concrete production protected
verifier, semantic-to-machine backend and exact compiler artifact handoff are
still absent. Preparation's executable/hidden-kernarg and read-only initial
copies are not covered by the result budget; full host/native/domain/metadata
accounting remains MEM-DOM/3/4/5 work. Real runtime-owner shutdown must retain
the private decoder and returned storage through exact disposal/completion.

The [dispatch](../../runtime-a1-a2-next-wave.md) assigns host-cache bounds,
generated invocation preparation, independent fixtures and native-budget
qualification next. Signed active-work drain/overlap/pressure acceptance,
native capacity, full executor refinement and matched HIP/HSA measurements
remain open. These local results do not establish A1/A2 closure or broad parity.
