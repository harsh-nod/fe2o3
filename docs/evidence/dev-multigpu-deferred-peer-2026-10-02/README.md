# Deferred Compute Peer Continuation

This checkpoint removes the intermediate host join from
`native peer -> deferred compute -> native peer -> D2H` above
`43c70ad45bf14f67a4a04d691a0925e374453589`. The complete pipeline passes on two
MI300X GPUs in both device orders, driven only from the final readback stream.
This is bounded functional qualification, not general application authority,
formal refinement or HIP/HSA performance parity.

## Implementation

A downstream native peer copy can retain an exact pending deferred producer's
full `Write` output and explicit event. Its independently retained, accounted
launch payload and immutable admission identity survive queued, issued and
completed producer states, including public event release. The issued child
result is recorded separately from its live stream mapping. Successful native
completion and restored custody are required before transfer ownership moves.

The reservation exception applies only to that authenticated source. It does
not permit an unrelated deferred owner or destination, a partial output, an
aliasing `ReadWrite` binding or staged-copy fallback. Checked dependency ranks
precede recursive progress; failure, cancellation and uncertain ownership keep
their existing disposition rules. Polling and expired waits remain
observation-only. Context production code and all existing kernel authorities,
policies and artifacts are unchanged.

## Native Results

The new `gfx942-runtime-deferred-peer-chain-smoke` example reuses unchanged R57
V2 authority. Three setup launches finish first. One owner command then admits
the initial peer, destination's pending `C + B -> D` compute, return peer into a
distinct allocation and dependent readback. Public events are released after
dependent admission; all four original submissions remain pending before
explicit progress. Only the final readback stream is registered for progress.

| Case | Ordered GPU ordinals | Result |
| --- | --- | --- |
| Deferred pipeline | 6, 7 | Full bytes, four exact callbacks, native counter 0 -> 2 |
| Reversed pipeline | 7, 6 | Full bytes, four exact callbacks, native counter 0 -> 2 |
| Existing live batches | 6, 7 | Two changed batches in one Context; counter 0 -> 2 -> 4 |
| Existing queued control | 6, 7 | Round 0; full output and padding, six exact callbacks |

Each new pipeline independently checks all 262,144 output bytes against
`f32((i & 63) * 0.25 + (i & 31))`. The extent is exactly page-aligned, so there
are no padding bytes to qualify. Both peer destinations are checked against
their initial sentinels before admission; no expected computed output is
installed from the host. The output SHA-256 is
`275fe5e215ed3701e9f0d694119cf46d89846916ea29269dec0533d56e410925`.

Across four processes there are 14 successful launches (including six setup
launches), 10 peer copies, eight dependent readbacks and 26 exact pipeline
callbacks. Eight peer completions have directly observed native counters; the
two queued-control peers retain their explicit post-cutoff counter limitation.
Results release in reverse dependency order before explicit owned shutdown.

Fresh UID/BDF, activity, VRAM, counted process-to-device and host-memory checks
pass before and after every case. All three uploaded executables and the exact
owned scratch directory were removed; owned process absence and restored
baselines were checked. GPU 0's foreign work was not targeted. These point
observations are not an exclusive reservation or continuous attachment census.

## CPU And Source Checks

- Runtime: 2,096 passed, zero failures/filtering and 32 unchanged hardware ignores.
- All previous tests remain, with eight backend and six Context regressions added.
- Examples: 43 passed, including four new argument, oracle, extent and callback tests.
- Strict Clippy, no-default and hardware-feature checks, formatting and whitespace pass.
- All 32 exact source-control workflow commands pass. Ten guard files refresh only
  17 SHA literals and seven inventory counts; 76 proof files and their predicates
  and expected counts remain unchanged.
- The prior 1,925 KFD passes are authenticated reuse through the identical test
  executable, 344 unchanged source files and the accepted baseline archive, not
  a fresh KFD test run. No new kernel compiler rebuild or correctness proof is claimed.

The Context tests execute real mock copy/read/write operations before ordinary
completion observation. Backend tests use scripted native custody transitions.
Neither set substitutes for the separately recorded GPU arithmetic checks, and
the old component proofs do not formally refine this new continuation adapter.

## Evidence

`raw.tar.gz` contains `fe2o3-multigpu-deferred-peer-20261002/`, including the exact
CPU commands, environments and rosters, source snapshots, metadata proposal,
source-control receipts, native campaign, cleanup and final `qualification.json`.
The accepted selection is CPU `attempt-01`, metadata `proposal-01`, source
workflow `attempt-01-after` and `hardware-01`. The auditor checks all 5,867 source
files, exact added test names, binary bindings, independently computed outputs,
complete receipt rosters and final source/ELF continuity. `SHA256SUMS` binds this
README and the archive.
All 567 raw files were independently read back from the archive and matched
their original contents.

New native witness SHA-256:
`b410686a050ec1a344f5e525d70105790ae5f58103732dc666134f6a6f6d0a16`.
Runtime test executable SHA-256:
`29abe19fc84604f420125893f50572e97e9fd4fc3273c2218d0c0c1dd3fe6c2b`.
Auditor SHA-256:
`ea879ae3a5bbba4cb3368d4347521a7f9d0adf1d6dd356c5fec41ec658ccb418`.

## Remaining Work

The next application gate is genuine compiler/semantic-to-machine evidence for
an ordinary production kernel through the existing caller-authority constructor.
Another finite fixture or hash whitelist cannot supply that evidence. Arbitrary
graphs, native partial-failure isolation, postcopy source preservation, physical
overlap, eight-GPU execution and matched HIP/HSA performance remain unqualified.
This checkpoint does not rerun the earlier seven-GPU campaign or support
same-process device reopen, and does not close the overall multi-device milestone.
