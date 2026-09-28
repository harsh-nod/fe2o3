# Shared Pipeline Lifecycle And Epoch Chain

Development metadata refinement, CPU qualification and one native ordered-pipeline
smoke. This is not full runtime refinement, HIP/HSA parity or a speedup result.

## Source And Results

Signed source: `6d55282a7beeb44f11c25725bb287ecab2fab383`.
Tree: `6f89ea5d101625538d5798f6f781a3614483d8df`.
Implementation parent: `19de214db7da92ec5a565efc49d7c7114b025d68`;
the final source commit changes only one solver-control mutation.

| Check | Result |
| --- | --- |
| Original, relocated and closing Verus runs | Each 44 verified, 0 errors |
| New executable lifecycle solver mutations | 12 intended logical failures |
| Signed inputs, source continuity, pinned verifier closure | Passed |
| Chain calibration and inherited classifier/publication calibration | Passed |
| Focused pipeline CPU tests | 17 passed |
| Full runtime library suite | 1806 passed, 0 failed, 28 ignored |
| Strict runtime all-feature/all-target Clippy | Passed |
| Runtime no-default-feature compilation and workspace formatting | Passed |
| MI300X ordinary pipeline | 64 launches; exact output; contiguous completion; cleanup complete |

`qualification2` exits zero. The full CPU suite took 198.84 seconds. Its counts
overlap the focused suite; neither duration is a GPU benchmark. The three earlier
telemetry tests reporting `InspectSocket(PermissionDenied)` pass with the updated
unrestricted environment, without test changes or waivers. Ignored tests remain
unqualified. The KFD library suite, other model suites and doctests were not rerun.

## Architecture And Proof

The runtime now borrows its actual slot table and scalar heads through private
lifecycle wrappers. Complete first-epoch lookup, promotion and quarantine bodies
are compiled by both Rust and Verus. Public APIs, capacities and native authority
boundaries are unchanged. Promotion remains an early-terminating O(N) scan with
O(1) extra storage; quarantine remains a full O(N) scan. CPU controls observe zero
allocations in normal paths. No measured latency or throughput improvement is
claimed for this extraction.

Raw contracts retain arbitrary representable metadata, exact opaque Active
payloads without Copy/Clone, and all unaffected slots/generations/heads. Promotion
selects the first physical epoch match regardless of phase or identity corruption.
Its minimal no-panic premise is positive occupancy when a promotion exists; the
existing live-zero path removes the selected owner before panicking. A CPU test
preserves that destructive prefix. The inspected production caller prevalidates
occupancy, but full caller refinement remains separate.

The stronger, phase-independent `chain` predicate establishes unique exact owners,
occupancy, bounded capacity and a complete confirmed interval [L,H), where
H=next.unwrap_or(MAX+1), L=frontier.unwrap_or(H), and a staged owner occupies H
outside that interval. Shared transitions preserve this invariant through stage,
confirm, withdraw, promotion and quarantine. Chain implies the frontier predicate
and, for a Publishing staged owner, the settlement predicate. Raw corruption
acceptance is not hidden behind this stronger premise.

The 44 obligations include 26 inherited publication obligations and 18 additions.
The no-precondition opaque-payload witness publishes MAX-1, stages/withdraws/retries
MAX without an epoch gap, blocks promotion while staged, confirms exhaustion,
quarantines both owners, promotes them in order with exact payloads, and rejects
fresh admission while returning the original owner. A general staged-quarantine
witness proves idempotence and refusal of confirmation, withdrawal and promotion.
Quarantine does not erase the staged marker. Raw promotion of an unstaged
Quarantined owner remains possible; outer terminal gating is not a metadata theorem.

The 12 mutants target wrong epoch/index, omitted last slot, positive-live promotion
while staged, missing occupancy decrement, reused frontier, phase/owner substitution,
generation reset, and incomplete/corrupt quarantine. Every accepted negative was
inspected and failed the intended postcondition, assertion or loop invariant.
These are solver mutations of executable lifecycle bodies, not Rust executions or
mutations of the chain predicate. The prior 18 publication mutants were not rerun.

Three new CPU groups compare independent pre-extraction references and full owner/
neighbor/head/generation snapshots at 64/1024 capacities, including all five phases,
malformed identities, duplicate frontiers, MAX, staged refusal and high-slot
quarantine. Their populated payloads use scripted execution, not native receipts.

## Native Smoke

The signed source was exported with `git archive` into a fresh MI300X task directory.
Local and remote archive hashes match. An offline, locked debug build used the
pinned nightly-2026-04-03 toolchain and two build jobs with a dedicated target.
GPU1 unique ID `0xab83d2ffef0d3cdf` appeared idle immediately before execution;
GPU0's ComfyUI work was left untouched. This was shared-host correctness testing,
not an exclusive performance reservation.

The existing `gfx942-runtime-r60-ordinary-pipeline` example passed its full
64-publication prefix, contiguous completion identity, byte-exact output, tail
resident reuse, zero tail user-data materializations and normal resource shutdown checks.
Its output hash is `79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
Execution is ordered wait-for-prior, not concurrent kernels. The profile, binary
hash, build log and GPU snapshots are archived. ELF NEEDED entries name only
libgcc, libc and the loader; this observation is not the complete production
dependency/symbol audit. No native populated-pipeline quarantine, AUX fault,
multi-device, protected Worker, HIP/HSA baseline or performance result is claimed.

The task-owned remote source/build directory was removed after copying results;
absence was checked at 2026-09-28T16:07:14Z. Account-shared caches and other users'
processes were not cleaned or stopped.

## Remaining Work

The proof still mirrors metadata types and treats the complete Active payload
opaquely. It does not mechanically authenticate native receipts or prove the
HostMetadataTable adapters, actual Pending handoff, resource-retain settlement,
accepted recipe/lane/storage identity, observer ordering or AUX restoration.

Next compose the actual outer lane result with indexed publication attempt,
metadata settlement and installation of the exact native batch. Published or
Retryable storage alone is insufficient after an outer error/unwind. Keep the
current pre-confirm timestamp mutation explicit. Then prove the actual successor
link check (stream, predecessor, retain count, structurally equal recipe, shape
and receipt-derived phase) and its callback-free interval before promotion.
Native fault controls and broader Context/Worker/compiler/hardware refinements
remain required. A0-A7 and accepted Native R125, Admission R118B and Resources
R116/V3 checkpoints are unchanged.

The refreshed GitHub API record shows issue #182 OPEN, updated
2026-09-27T10:25:24Z. Both source remotes received the qualified branch; publication
of the later documentation commit is recorded separately.

## Evidence And Reproduction

`receipts.tar.xz` contains commands, statuses, timestamps, signed proof snapshots,
mutation diagnostics, CPU logs and native receipts. Verify its adjacent SHA256
file. The large native source tar is omitted from this archive; its hash, exact
commit and export command are retained for reconstruction from the pushed source.

Preflight frontend/proof failures, the witness resource-limit failure, and the
first failed campaign remain in the archive. That campaign's staged mutant also
introduced underflow, producing a relative vstd diagnostic rejected by the strict
path classifier. The revised mutant isolates positive-live staged admission;
the production proof and classifier were not weakened. The first full suite was
interrupted by the environment transition and is not counted as a completed run.
Final local qualification and the native build/smoke ran serially with no live
implementation edits. No earlier frozen evidence packet was changed.

This remains a standalone development campaign, not the global Verus release gate.
From a signed clean checkout, use a new absolute output outside the repository:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/test-compute-pipeline-chain.py
python3 -I -B crates/fe2o3-runtime-model/verus/check-compute-pipeline-chain.py \
  --verus "$VERUS" --output "$OUTPUT"
```
