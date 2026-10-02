# Native Multi-Packet Compute/XGMI Qualification

Date: 2026-10-02 UTC. Base: `1327f19b1b0c9481ac5d2cd62c27a6a50ed609aa`.
`raw.tar.gz` and `SHA256SUMS` retain the implementation patch, source identities,
exact commands, terminal receipts, rejected attempts and verification output.
This is an A3 increment, not full HIP/HSA parity or whole-runtime formal closure.

## Implementation

Initialized PUBLIC persistent DeviceLocal owners can now copy equal full logical
extents in multiple native SDMA packets. The immutable allocation-free plan
admits at most 4,096 packets of at most 4,194,272 bytes each. The runtime's separate
256 MiB allocation limit is unchanged. The two original allocation owners and
one directional queue remain rooted across the complete logical transfer, with
one packet ticket in flight. Mappings are established once and restored once.

Explicit progress samples one completion or publishes the next packet. Ordinary
poll never publishes. Intermediate completions do not return allocation custody,
release child reservations, increment the native logical-copy counter or hand
inputs to a deferred consumer. Both model foundations are retaken around active
steps. Uncertain errors/unwinds retain the owners and poison the affected route;
no uncertain prefix falls back to host staging. Existing private, partial,
uninitialized and otherwise ineligible allocations retain staged behavior.

The new optional `--packetized-copy` witness adds auxiliary 4,194,273-byte and
8,388,581-byte copies after the four exact R57 launches. They exercise two and
three packets with one-byte and 37-byte tails. Absolute-offset-derived bytes and
complementary sentinels make omitted tails and first-packet replay observable.
Neither those buffers nor the transfer plan expand kernel launch authority.

## Hardware

Primary-session SSH worked. Earlier worker-namespace DNS failures are retained
but are not evidence that the host was unavailable. The authoritative initial
PID-to-GPU query showed another workload on GPU 0 only. Every candidate run
rechecked both selected unique IDs, zero utilization, unchanged memory use and
the unchanged process roster immediately before launch. This is a shared-host
correctness campaign, not an exclusive reservation or continuous FD census.

| Device | Unique ID | PCI BDF |
| --- | --- | --- |
| GPU 6 | `0x10a254ce4987e716` | `0000:C6:00.0` |
| GPU 7 | `0x53691ef168a0147d` | `0000:E5:00.0` |

Candidate witness SHA-256:
`3bdcfc078ccd29eb73d1913ea3a5c591e2ad756dbef4efedb48011df604e90c6`.
It matches local build output and remote initial/final hashes.

| Candidate mode | Direction | Native logical copies | Full readbacks | Result |
| --- | --- | --- | --- | --- |
| Default explicit drain | 6 -> 7 | 1 | 13 | Pass |
| Queued consumer | 6 -> 7 | 1 | 13 | Pass |
| Packetized copies | 6 -> 7 | 3 | 21 | Pass |
| Queued consumer plus packetized copies | 6 -> 7 | 3 | 21 | Pass |
| Reverse packetized copies | 7 -> 6 | 3 | 21 | Pass |

Every mode executes four exact R57 launches and the original 262,144-byte peer
copy. The packetized modes add two large copies totaling 12,582,854 bytes and
five SDMA packets; the completion counter counts logical copies, not packets.
Combined mode queues the original small-buffer consumer before its copy
progresses, then separately drains the large auxiliary copies. It does **not**
qualify compute consumption of large packetized outputs or physical overlap.

Two earlier hardware runs of the accepted baseline executable are separately
retained, not counted as candidate runs. Both default and queued-consumer modes
passed with baseline SHA-256
`13f1c30355e837934096231ca01cdaf20cc35c1717fab892ec67b3f3f46b3ff8`.

All seven processes exited zero after explicit logical and native cleanup.
The final GPU memory figures and process roster match the pre-run observation.
Only the two uploaded executables were removed; `rmdir` of their exact owned
directory returned zero at 04:05:27 UTC. No remote build tree, workload, core
dump or scratch directory was left behind. Hardware receipts are under
`hardware-primary`; an independent agent audited their hashes and mode fields.

## CPU Checks

Accepted compilation/tooling is `attempt-03`. Cargo's structured artifact
records select the actual KFD and runtime test executables; hashes are checked
before/after execution and focused rosters must be nonempty. Each controller
executes its own immutable per-attempt script snapshot.

| Check | Result |
| --- | --- |
| Full KFD library, four disjoint shards | 1,918 passed; no failures or ignores; exact union coverage |
| Full runtime library | 1,986 passed; 32 existing hardware ignores; no failures or filtering |
| KFD compute/XGMI | 55 passed |
| KFD packetized composed subset | 5 passed |
| Runtime compute/XGMI | 38 passed |
| Runtime packetized subset | 6 passed |
| Runtime deferred compute subset | 7 passed |
| Witness CLI/pattern tests | 5 passed |
| Strict combined KFD/runtime all-feature Clippy | Pass |
| No-default runtime library | Pass; existing unused `Route::Native` warning |
| Witness build, targeted formatting, whitespace | Pass |

Filters overlap. Fourteen new library test functions cover three pure planner
tests, five composed lower tests and six runtime tests; the witness adds two
tests. Lower composed coverage includes full/full/tail packet offsets, ring-tail
wrap and ticket generations, observer-only polling, premature finish rejection,
later publication failures, closing observation faults and both model retakes
at intermediate/final completions. Runtime tests cover exact progress generation,
expired/resumed drain, cancellation and release exclusion, fallback eligibility,
terminal ownership and final-only deferred consumer handoff. Scripted counters
never claim a native GPU completion.

Initial prechecks are retained: attempt-01 found a missing intermediate queue
re-export; attempt-02 found private/nonexistent fields in a new test. The export
was added, and the test uses existing ledger/query APIs without widening
production visibility. Neither failed attempt is accepted qualification.

The primary intentionally stopped the slow serial KFD full run using a pidfd
after checking its exact executable identity. Its exit 143 and controller exit
one remain recorded, not relabeled as passes. Qualification instead requires
four exact disjoint rosters over the unchanged compiled executable: three
108-test primary-construction shards and the 1,594-test remainder. The full
runtime result is independently terminal despite the original controller's
aggregate nonzero status. `serial-stop.md` records the reason; shard receipts
and `cpu-qualification.json` establish the replacement coverage separately.

All four shards passed with their process groups closed, immutable executable
hashes unchanged and empty stderr. The independent aggregate checks exact
per-test pass names, disjointness and equality with the original 1,918-test
roster. It separately counts all 1,986 runtime passes and 32 existing ignores.
Construction shards finish in 448.27, 510.07 and 528.27 seconds, each below the
1,200-second bound; the remainder finishes in 17.32 seconds. These are CPU test
durations, not GPU performance measurements.

## Arithmetic Proof

`arithmetic-proof/proof-attempt-05` passes the complete 13-stage campaign:

- Five classifier/source controls.
- Full pinned 190-file Verus release closure checked before and after.
- Two full positive runs, each reporting five verified and zero errors.
- Eight distinct shared-body mutants rejected by the intended postcondition,
  joined to the exact contract and macro expansion, not parser/tool failures.

The production Rust and proof include the same executable packet-count and
subrange macros. The proof establishes count bounds, nonempty bounded packets,
valid offsets, adjacent coverage and exact final tails. The source closure also
captures the wrapper, the existing packet/segment constant providers, runner,
controls and pinned support. All ten inputs match before/after. There are no
new axioms or external proof bodies. Wrapper forwarding and constants are
source-audited; wrapper-state refinement is not proved.

The earlier four development attempts are preserved and rejected: parser cast
precedence, an initially incorrect positive-summary count, a strict negative
classifier rejecting the compiler abort summary, and an overlap mutation that
failed an arithmetic obligation instead of the selected postcondition. The
final overlap mutation preserves that arithmetic obligation while making packet
ranges overlap. Production arithmetic/contracts were not weakened to accept a
mutant. Every final owned process group was absent at closing inspection.

This proof does not establish ordered-cursor/whole-adapter refinement, allocation
ownership, Linux currentness, DMA behavior, Context reconciliation, concurrency,
hardware correctness or performance. Those are separate trust/qualification
boundaries; hardware tests do not turn them into machine-code proofs.

## Source Controls

All 32 existing source-control workflow commands pass in
`source-ci/attempt-01-after`, with unchanged source inputs and closed process
groups. Final `proposal-04` metadata audits check 1,021 source inputs and 76
unchanged guarded executable-proof files. Changes are exactly 20 SHA literals
and seven source-inventory counts across 12 files. Contracts, predicates,
expected solver totals and mutant totals remain unchanged.

Cadence's two whole-file pins change only after an independent import/registration
delta review. The runtime roster grows from 340 to 341 files, or 345 to 346 with
the five schemas. Earlier proposals and the rejected post-apply audit following
a documentation edit are retained. The final proposal was regenerated from the
frozen tree; no stale source receipt was relabeled as current.

## Remaining Gates

Actual 65-packet ring reuse, further device pairs, repeated-copy qualification,
production-authority native-peer opt-in, completed deferred-result reuse,
all-device sharding, partial-failure campaigns and matched HIP/HSA measurements
remain open. Peer queues/mappings are not cached across logical copies. The
native runtime path remains qualification-only and reserves whole child backends
during transfers. The unchanged 76 historical proof files do not prove this new
adapter, and the previous ordinary Context completion-leaf proof gap remains.
A3 and the broader runtime parity milestones are not complete.
