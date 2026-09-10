# R65 Local Drain And Version Validation

Result: local runtime tests, authenticated version-guard proofs and two guarded
MI300X qualifiers passed. A1 and A2 of #182 remain open. This is not complete
native resource admission, active-GPU drain qualification, compiler admission,
compute/copy overlap or HIP/HSA performance parity.

## Exact Source And Capture

- Signed implementation and hardware source:
  `19b6011378b88e53a8c93344db6793fc0f90324b`.
- Branch: `codex/r65-runtime-drain-versions`.
- Final CPU test refinement: `1a68e2f2b7ad18ff865be0fc75d017acfa4bc732`.
- Full capture: `/home/harsh/.codex-tmp/fe2o3-r65-hardware-19b60113.tar`.
- Capture SHA256:
  `13e036cb6e449b639b1c4b03849db67e75f4019303ce8ecf489af79896a2f76d`.
- Source archive SHA256:
  `98189f4dc0b97504a08c54b531638126ea9dace6305a6c4b371f922f324c6382`.
- Binary: 4,698,096 bytes; SHA256
  `5d515f50923499a5fe73ff18763a5528321e4ad712804e0ae9b553b9eb45cacb`.

The primary capture audit checks all 73 manifest payloads, all 32 command exit
codes, both exact PASS records, signed commit bytes and all 6,718 source files
against a fresh Git archive. Signer trust is supplied independently from the
capture: `SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

`raw/musl-accepted` retains original capture payloads except `source.tar` and
`owner-binary`; the original manifest still records these omissions. The full
capture remains at the path above. Outer wrapper logs are in `outer`, and local
validation/audit logs and scripts are in `raw/host`. `retained-files.sha256`
covers the retained raw files without normalizing tool-emitted whitespace.

After hardware-source freeze, one test-only refinement made the 2,048-operation
Tokio case register its pending drain before releasing the paused owner, and
check its watchdog before polling the drain. A timer wake therefore cannot
rescue a lost owner wake. Final CPU suites include this refinement. It changes
no production implementation, qualifier, proof or hardware admission input.

## Implemented Scope

- Cooperative drain closes admission across all cloned handles, observes the
  finite accepted workload and separates quiescence from native cleanup.
  Budget exhaustion, Stop, panic and terminal uncertainty retain custody.
- Shared reply-count admission follows the actual producer/future cell lifetime,
  including abandoned producers and caller-retained completed futures. The
  separately bounded drain slot remains available with full reply/channel capacity.
- Graph-local versions partition validated effects into exact covered segments,
  check optional producer expectations before reservation, retain input and
  storage-predecessor lineage, and commit only after successful native retirement.
  Complete-set checks precede mutation; historical reports cannot authorize
  a later execution.

See the [R65 contract](../../runtime-async-drain-versions-v1.md) for limits,
error precedence, conservative invalidation and verification boundaries.

## CPU And Proof Results

| Validation | Result |
| --- | --- |
| GNU all features/all targets, four crates | 1,979 passed; 5 ignored; 45 harnesses |
| musl all features/all targets, four crates | 1,979 passed; 5 ignored; 45 harnesses |
| GNU doctests | 64 passed; 0 ignored; 4 harnesses |
| musl doctests | 64 passed; 0 ignored; 4 harnesses |
| R26/R40/R60/R61/R62/R63/R65 runner tests | 68 passed |
| Clippy, all features/all targets, warnings denied | Passed |
| Workspace formatting and source/document diff checks | Passed |
| Production Cargo closure | 42 packages; 8 permitted build scripts |
| Authenticated Verus suite | 54 positive sources; 1,300 obligations; 624 expected-negative rejections |

The tested crates are runtime, runtime-model, KFD and completion. Thirty-one new
runtime/model tests cover admission races, destructor reentry, reply lifetime,
full-capacity lifecycle admission, raw predecessor progress, active graphs,
observer Drop, budget exhaustion, initially terminal contexts, poll/flush panic,
final-tick quiescence, partial overwrites, aliases, failure/cancellation, bounded
version references and failure-atomic begin/commit. The 2,048-operation test is
a one-owner CPU mock with a real Tokio executor, not native GPU depth evidence.

Eight new Verus obligations prove per-segment guard conditions, with eight
targeted negative mutations. Production uses the corresponding pure Rust
helpers; the model test makes 648 phase/index guard comparisons. Reply accounting reuses
R64's checked helpers. The pinned 190-file, 129,019,839-byte verifier closure and
pre/post source, runner, solver and negative-inventory gates passed.

These are guard/arithmetic proofs with reviewed Rust correspondence, not full
ledger/executor refinement or proofs of partitioning, mutex/CAS linearization,
thread scheduling, drain termination, native completion or machine-code effects.
Three swarm agents contributed implementation, proof integration, qualifier
construction and independent reviews; the primary integrated and validated them.

## MI300X Qualification

The R65 profile inherits the signed-source, private offline build, pinned nightly,
static-musl/BFD, full-symbol, topology, queue-census and owned-cleanup gates.
Its copy-only authorizer rejects all compute admission. No gate was relaxed.

Both processes executed the same four-stream/twelve-node/five-copy diamond
twice, resetting and fully reading back input/output/padding bytes for each run.
Each graph checked exactly 21 version records, 10 input rows, 13 terminal-current
versions and distinct execution occurrences. All graph submissions retired before
readback; only afterward did the owner perform idle drain and explicit cleanup.

Each process emitted exactly one line:

```text
PASS schema=fe2o3.runtime.r65-graph-versions-idle-drain.v1 bytes=1048832 owner_threads=1 streams=4 nodes=12 copies=5 executions=2 versions=21 version_inputs=10 current_versions=13 occurrences=distinct joins=host canaries=complete submissions=released drain=idle-quiescent admission=closed cleanup=complete
```

Selected GPU: index 1, UID `0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`,
KFD GPU 23018/node 3. Worker CPUs 0-47, memory NUMA node 0, observer CPU 95.

| Run | Root PID/PGID | Census Samples | Target Queue Observations | Maximum Gap |
| --- | --- | --- | --- | --- |
| 0 | 3665864 | 1,324 | 1,705 | 5,350 us |
| 1 | 3666022 | 1,322 | 1,697 | 7,303 us |

Both sealed 2ms censuses passed the unchanged 10,000us maximum-gap gate, with
zero foreign/terminal selected-device queues, reaped children and absent process
groups. Five topology records agree and all four boundary telemetry checks are
idle. Independent inspection of the actual ELF matches all 5,209 unique full
symbols and recorded program headers, with no undefined/dynamic symbols,
interpreter, dynamic dependencies, GNU minimum-stack lookup or Tokio bytes.
The retained Cargo metadata independently passes the production closure audit.

## Shared-Host Cleanup

One guarded attempt was needed. Its wrapper removed the exact private stage
`/dev/shm/fe2o3-r65-owner.1Cci4FZq`. Independent SSH checks confirmed absence of
that stage, wrapper/runner PIDs 3655843/3658984, recorded compiler processes
3659540/3664412 and both GPU child PIDs. GPU 1 reported 0% utilization and VRAM
allocation. GPU 0's foreign 44% allocation remained untouched. The executor
crate remained absent from the shared Cargo cache; caches and build outputs
were task-private. No resets, fault injection or all-GPU campaign occurred.

## Remaining A1/A2 Acceptance

A1 still needs general admitted generated kernels, mixed-duration/out-of-order
and thousands-in-flight native qualification, complete native memory/signal/
kernarg budgets and active-work drain/failure qualification. A2 still needs
compiler-admitted repeated kernel/copy graphs, cross-run mutation authority,
pools/executable residency and measured compute/copy overlap. Disjoint native
compute/SDMA coexistence requires reciprocal exact-storage checks in both
lower KFD and runtime code; it is not solely a compiler dependency.

#134/#214 admission/refinement work remains separate. The copy-only results and
abstract guard proofs do not fill those gaps. #182 and both milestones remain open.
