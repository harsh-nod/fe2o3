# Batch Admission, Native Forwarding And Drain Composition

Local MEM-TXN-1, MEM-2A-FWD and DRN-3B implementation evidence. Baseline:
`440200f0eefef44024de019a21fbc48541c85a35`, branch
`codex/r65-runtime-drain-versions`. Changed source identities are retained in
`source-files.sha256`; raw logs and supporting files have a separate manifest.

## Scope

MEM-TXN-1 adds bounded, failure-atomic single-account batch admission to the
existing shared credit engine. Complete vector, record, owner-interval and
free-slot checks precede commit. Ordinary exhaustion preserves account state;
detected corruption poisons without issuing members. Each returned move-only
member has independent existing cancellation, retention and disposal semantics.
The [batch contract](../../runtime-resource-batch-v1.md) records metadata bounds,
nonquadratic admission work and native-consumer obligations. This is not parent
accounting, arbitrary debit splitting or native compound-resource integration.

MEM-2A-FWD installs optional immutable session-local N2 limits before native
materialization or queue certification in both runtime startup orders. Existing
defaults remain unchanged. Production configuration and foundation
transfer/loan/retake helpers are exercised with a fake backend; separate source
wiring tests cover the Linux constructors and runtime call sites. The inert
usage getter returns unavailable rather than implying zero or disposal. The
[native accounting contract](../../runtime-native-resource-accounting-v1.md)
keeps GTT, pools, control/bootstrap, parent domains and aggregate closure open.

DRN-3B adds four tests with twelve controlled composition scenarios: six
operation/waiter/capture cases, three graph/capture cases, repeated observation
exhaustion, and two Stop orderings. Graph and standalone-operation matrices
remain separate where exclusivity requires it. Tests check exact publication,
release and dependency rosters; wakeups; graph reservations; and reply/capture
credits. Stop cannot preempt synchronous capture. A completed capture may win
before concurrent Stop is processed, but partial bytes cannot become a result.

This record does not prove the native adapters, complete executor, aggregate
memory bound, hardware behavior or performance. The
[swarm plan](../../runtime-a1-a2-swarm-plan.md) identifies the next device-pool,
GTT/domain, drain-qualification, generated-authority and refinement packets.

## Final Local Gates

| Gate | Result |
| --- | --- |
| GNU runtime all-feature/all-target tests, five crates | 2,143 passed; zero failed; five existing ignores; 47 harnesses |
| musl same runtime gate | 2,143 passed; zero failed; five existing ignores; 47 harnesses |
| GNU runtime plus host doctests | 82 passed |
| musl runtime doctests | 71 passed |
| musl default direct-KFD host doctests | Ten passed |
| GNU all-feature host library | 207 passed; four existing ignores |
| Generated macro fixtures | Seven passed |
| Runner/checker Python suites | 134 passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Passed, warnings denied with existing scoped custody policy |
| Formatting and source whitespace | Passed |
| Production musl metadata audit | 43 packages; eight permitted build scripts |
| Workspace dependency policy and tests | 141 members, eight layers, 476 declarations; eight tests passed |
| CI-local test-gate regression and standalone lockfiles | Passed; 32 standalone manifests checked |

The 37 new tests comprise fourteen shared-credit, six model, seven KFD,
six runtime-forwarding and four DRN-3B composition tests. Two of the thirteen
KFD/runtime-forwarding tests inspect source wiring; the rest use CPU/fake-backend
fixtures. One new compile-fail doctest rejects cloning a batch member. None
executes the live Linux constructors or qualifies native pool behavior.

All sixteen final commands, exit statuses and timings are retained in
`raw/r70-final2-source-gate.json`. The harness uses pinned
`nightly-2026-04-03`, locked/offline dependencies, four Cargo jobs, disabled
incremental compilation and an unset stale `XDG_RUNTIME_DIR`. The observed
compiler version is retained. No Cargo manifest or lockfile changed. Raw logs
preserve original whitespace; final source-whitespace checks exclude these
verbatim `.log` artifacts. Both SHA-256 manifests were checked after retention.

## Authenticated Proof Gate

The full Verus suite completed with exit status zero: 59 positive sources,
1,347 obligations and 659 distinct expected-negative rejections. Pre/post source,
inventory and pinned release-closure checks, plus the exact transcript, passed.
Both release-closure measurements cover 190 files and 129,019,839 bytes. The
exact command was:

```sh
VERUS=/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus sh crates/fe2o3-runtime-model/verus/verify-verus.sh
```

R70 adds nine obligations and nine named mutations for complete bounded batch
admission, owner intervals, distinct member owners and member-refund algebra.
The production-used Rust preflight and Verus source have reviewed, not
mechanically linked, correspondence. Mutex/arena extraction, bitset correctness,
allocation success, token issuance, native cost/disposal, account-domain
composition and whole-executor refinement remain outside this proof. No new
trusted body or assumption was added.

## Earlier Attempts

Failed and diagnostic attempts are retained rather than promoted:

- The first Cargo check found a missing test-only `alloc::vec` import in the
  no-std model. The corrected check passed.
- Direct pinned Verus verification found nine positive obligations, not the
  initially anticipated seven. The runner registers the observed nine without
  weakening their statements.
- Initial proof-source audit used a normalized runner hash where the strict
  checker requires raw source bytes. Correcting the fingerprint passed the audit;
  negative-checking rules and the `check_negative` function remain unchanged.
- The first full GNU attempt found a formatting-sensitive constructor source
  assertion, a reply-capacity test race and a terminal mock's intentional Drop
  abort. The assertion now checks the exact argument independent of whitespace.
  The existing DRN-1A test uses an unbudgeted FIFO owner barrier before reusing its
  single reply cell. The resource-free terminal mock is retained rather than
  dropped; production terminal semantics are unchanged. A narrower DRN-1A
  diagnostic passed before the race fix and is not a substitute for full gates.
- The next full attempt passed GNU/musl tests and doctests, host and macro
  fixtures, then Clippy rejected a redundant `drop` of a destructor-free test
  authority. Removing that statement required another complete local gate run.

## Shared Hardware

One read-only `rocm-smi` query on MI300X reported GPU1 at 100 percent utilization
and 44 percent VRAM use. Its raw output is retained. No remote stage, build,
qualifier or GPU workload was started; no remote cleanup was needed and foreign
work was untouched. Signed pool/drain/coexistence qualification, matched HIP/HSA
performance, A1/A2 closure and runtime-wide parity remain open.
