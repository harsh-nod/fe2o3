# Directed Native Peer Readback Qualification

Status: bounded CPU and ten-case MI300X qualification accepted by the independent
audit on 2026-10-02 UTC. Baseline:
`67d81278003d0edb3b7b7a8052b7e7f2849526f2`.

## Implementation

- A separate default-false backend opt-in admits exact pending directed peer
  outputs into same-device DeviceLocal-to-HostVisible readbacks. Context keeps
  writer identity, range, epoch, lineage, bounded ancestry and independent result
  retains; ordinary-only backends do not silently acquire this contract.
- Readback admission retains router metadata and staging, not child DMA custody.
  Explicit consumer progress services directed ancestors and authenticated
  already-started native resource blockers. A sibling is not a success dependency.
- Native peer -> compute -> native peer progress services the compute producer's
  exact retained peer prefix before testing child availability, then rechecks
  occupancy before child flush or observation.
- Native owners retain their existing paired terminal/panic handling. The added
  ordinary-copy blocker handoff applies only to staged legs. Cancellation does
  not cancel an independent producer or release uncertain native custody.

## Verification

Final CPU qualification passes 2,125 runtime tests with zero failures and 32
unchanged hardware ignores. All 50 example tests, eight focused groups, strict
Clippy, feature/format checks and all 32 source-control commands pass. There are
16 new runtime tests. The accepted selection is
`attempt-04`, metadata `proposal-04`, source workflow `attempt-04-after`, and
`hardware-01`. Rejected candidates are not final-source evidence.

The 1,925-test KFD qualification is authenticated historical reuse, not a fresh
run: its executable, complete test roster and all 344 source files match the
pinned prior evidence archive. The final audit checks all 5,874 source files.

The new CPU tests exercise actual Context/router code with scripted backend
completion or physical-owner fixtures. They do not demonstrate GPU arithmetic,
real driver faults or fault isolation. The hardware witness uses the public
copy-only constructor with deny-all compute authority and enables `--readback`
for three-device chains and fanout in both orders. Existing directed, deferred
compute and live-compute witnesses serve as current-source controls.

The two changed-content readback rounds reuse one Context and all six
allocations. Each peer copy is 8,388,581 bytes, including an odd 37-byte tail,
and uses three native packets. Round zero admits D2H before progress; round one
admits D2H after a bounded seed. The public interface exposes completion counts,
not publication state: live publication is explicitly **unobserved**. Exact
before/after-publication admission is checked by the CPU physical-owner fixture.

After admission and any reported seed, only the D2H stream is driven. All public
peer events are released before that tail drive. The witness checks every host
output byte, destination sentinels, postcopy source preservation, original result
callbacks, logical release and explicit native shutdown.

All ten live cases pass on GPUs 5/6/7: four new readback cases, four existing
directed-copy cases, one two-device deferred compute chain and one two-device
live-compute control. The new four cases total 16 native peers, eight D2H copies,
64 full verification reads and 24 callbacks. Including controls, the campaign
records 38 native peers, eight compute launches, 13 dependent D2H copies and
56 callbacks. It runs from the fresh 13:07 UTC admission snapshot through the
13:13 UTC final snapshot.

The final audit confirms no owned processes, removal of all three executables
and `/tmp/fe2o3-multigpu-directed-readback-20261002-jogVLM`, and restored selected
GPU use/VRAM and complete PID baselines. GPU 0 was not selected; no device reset
or foreign-process termination was used.

| Artifact | SHA-256 |
| --- | --- |
| Runtime test executable | `4b269b52c9861ed3f5425a97bac8ffb00310cdcfea7579ce55a25212ad533ef3` |
| Directed/readback witness | `7e1510768a415c92309dfba0234bdde165d1a00ffc57448f2dda1133e887f0a5` |
| Full source manifest | `5199bb179970d52e1a555b2fef56d651619c85c35d3a79a93c4f31230a2d1f77` |
| Independent auditor | `9aff386cbe6bdae852d7cfb24dd2b37aa766f3b1d3b1470509f3ac539b2d78e1` |

## Boundaries

This is not full HIP/HSA parity, arbitrary application-kernel authority, physical
overlap, eight-device qualification, matched performance or whole-adapter formal
refinement. New native peer/compute admission after endpoint extraction remains
separate work. Native ambiguity still fail-stops the router and retains uncertain
owners; third-child metadata is not evidence of continued same-Context operation.

The source guard refresh changes only identity hashes and exact roster counts.
All 76 proof files, proof predicates and expected proof/mutant counts remain
unchanged. No solver was run and historical proofs are not promoted to proof of
these new routing and custody adapters.

MI300X is shared. Qualification must use fresh UID/BDF/VRAM/PID gates, avoid
foreign processes and device resets, and remove only owned executables and the
exact owned scratch directory. Point-in-time gates are not an exclusive
reservation or a performance-isolation claim.

## Reproduction

The [raw bundle](raw.tar.gz), authenticated by [SHA256SUMS](SHA256SUMS), retains
controllers, commands, clean environments, source and
executable identities, test rosters, output/exit records, source-control audits,
hardware admission/cleanup receipts and rejected diagnostics. The read-only
auditor independently checks the full payload oracle, all final-source test
rosters, historical KFD executable/source identity and exact hardware case maps:

```sh
python3 -I -B audit.py attempt-04 proposal-04 attempt-04-after hardware-01
```

The auditor requires the source-freeze workspace with baseline HEAD, recorded
candidate files and retained artifacts. The archive is an evidence record, not a
portable self-contained build environment or a command that accepts a later HEAD.
