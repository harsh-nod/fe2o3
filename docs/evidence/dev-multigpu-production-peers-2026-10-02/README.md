# Production Multi-GPU Peer Checkpoint

Development qualification on 2026-10-02 UTC, based on
`7256bcfafc54cd5997507773debf94c4677b2682` plus the archived source patch.
This is a working two-device correctness checkpoint, not full A3, HIP/HSA
parity, performance acceptance or whole-adapter formal verification.

## Changes

- Default-feature production and semantic native-peer constructors validate the
  complete device roster and every ordered route before enabling PUBLIC storage.
  Caller kernel authorities, private defaults and staged fallback remain unchanged.
- Initial copy-only primary queues are admitted without a warm-up dispatch. The
  new branch requires pristine submission history and empty ownership ledgers;
  established auxiliary and detached-dispatch rules are unchanged.
- Completed deferred-compute results have exact private completion receipts and
  independent result custody. Pending deferred chains remain unsupported. CPU
  tests begin at an admitted child handoff; they do not qualify GPU-produced bytes.
- Ring witnesses perform two changed-content 268,433,409-byte copies, each using
  65 packets through one 64-slot native queue, with full-byte verification.
  Large H2D setup explicitly progresses its 63+2-packet directional windows.

## Final Results

`attempt-08` is the immutable build/check/focused/full-runtime selection.
The full runtime suite passes 2,001 tests, with 32 existing hardware ignores and
zero filtering or failures. The two example suites pass nine and three tests.
Strict combined Clippy, no-default checks, formatting and whitespace checks pass.
All 32 source-control commands pass in `source-ci/attempt-04-after`; proposal-04
changes only 18 hash literals and seven inventory counts across 11 guard files.
The 1,025 source and 76 historical proof files remain bound to that proposal.
All 1,925 KFD tests pass in five fresh disjoint shards (82, 81, 81, 81 and
1,600 tests). Their exact roster union covers every discovered test, including
all seven cold-session regressions, with no failures or ignores. Historical KFD
results are not reused.

`hardware-final-02` passes all seven selected cases on MI300X GPUs 6/7:

| Case | Checked result |
| --- | --- |
| Production native, 6 -> 7 | Two 8,388,581-byte copies, no kernel authority |
| Production staged control, 6 -> 7 | Same byte checks, zero native peer completions |
| Semantic native, 7 -> 6 | Same native copies with empty semantic profiles |
| Semantic staged control, 7 -> 6 | Same byte checks, zero native peer completions |
| Queued consumer plus ring reuse, 6 -> 7 | Four exact R57 launches, three logical peer copies |
| Reverse ring reuse, 7 -> 6 | Four exact R57 launches, three logical peer copies |
| Queued consumer plus packetized and ring copies, 6 -> 7 | Four exact R57 launches, five logical peer copies |

Every ring case verifies both changed-content rounds, including the one-byte
tail, unchanged source and complementary destination sentinel. The reported
readback totals count full-buffer verifications, not individual chunk reads.
Packet counts are bound to the executed plan, not raw ring telemetry. A queue
is retained across each complete copy, not cached across both copies.

Executable SHA-256, checked locally and before/after remote execution:

- Compute witness: `56dfec541f6e625ebcddc497a49bb94726925962a98f4ee9c30decda20e22091`
- Production witness: `330df566f03d63f75ef94c54dbeb8719c0539a22a3fb1486114f09b690f6847f`

Each case used a fresh UID/BDF, process-attachment, memory and idle-state check.
These are point observations, not an exclusive reservation. Exact owned files
and scratch directory were removed; no owned process remained and GPU6/7 memory
and the process roster returned to baseline. No foreign job was stopped.

## Proof Scope

`cold-proof-attempt-01` passes 14 stages: six controller controls, complete pinned
Verus closure checks before/after, two positive runs of two executable contracts,
and nine rejected logical one-condition mutations. The actual shared predicate
proves exact initial-state conditions and preservation of established acceptance.
The 11 input files are source-bound. This does not prove the authenticity of
native observations, ownership/model restoration, Linux currentness or DMA.

The unchanged packet-count/subrange arithmetic proof is explicitly reused from
the checksum-pinned [packetized checkpoint](../dev-multigpu-packetized-2026-10-02/README.md).
All ten proof/support/source pins and the actual prior archive members are checked.
No prior whole-runtime or hardware result is substituted for this final campaign.

## Rejected Attempts

Raw evidence preserves the early compile/lint failures and two scripted fixture
failures, with their source snapshots and corrected eager-preparation assumptions.
The original native run and owned diagnostics exposed cold queue rejection.
`hardware-final-01` passed four public-constructor controls but stopped at the
large-upload helper, which had assumed one directional window. All remote scratch
from rejected attempts was explicitly cleaned.

Build attempt-07 compiled successfully but its source-continuity controller
rejected an overlapping example edit with exit 125. Final attempt-08 binds the
unchanged library test ELFs to the complete frozen tree; the rejected outer run
is not counted as qualification. All final checks and source controls were rerun.

## Reproduction

`raw.tar.gz` contains immutable controllers, commands, environment records,
source manifests, exact test rosters and output, native receipts, failure logs,
the new proof campaign, review notes, and the source patch. `SHA256SUMS` protects
the archive. The final `qualification.json` is produced only after raw-log,
source/ELF, exact-roster, proof and cleanup checks succeed.

Original audit invocation in the raw directory and recorded workspace:

```sh
python3 -I -B audit.py attempt-08 attempt-08 attempt-04-after hardware-final-02 \
  --proposal proposal-04 --kfd-shards .
```

The auditor creates a fresh qualification.json and refuses to overwrite an
existing receipt. The archived invocation is not an in-place rerun command.
It deliberately requires the recorded workspace paths, current source,
prior pinned archive and verifier closure. It is not a portable standalone replay.
The archive's source patch is checked by reverse application before packaging.

Remaining priorities: genuine 2..8-device sharding, outstanding group drain with
bounded output capture, isolated partial failures, physical-overlap measurement
and matched HIP/HSA scaling. See the [current tracker](../../runtime-a1-a2-swarm-current.md).
