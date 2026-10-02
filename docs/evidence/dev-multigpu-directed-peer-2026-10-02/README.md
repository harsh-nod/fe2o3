# Native Directed Multi-GPU Copies

This checkpoint enables prequeued directed scalar copies to use the integrated
router's native XGMI transport above `e8cd1388bd96aa9ad49639c5e9f995c75c8c9d3b`.
Three-GPU chains and shared-source fanout pass in both device orders, with two
changed-data rounds on the same allocations. This is bounded functional
qualification, not general application authority, formal refinement or HIP/HSA
performance parity.

## Implementation

The existing native-peer opt-in now selects native transport for eligible
directed copies: initialized, equal full-buffer DeviceLocal extents with an
admitted route and available storage. Directed identities authenticate the
native phase, both endpoint reservations and original allocation custody.
Partial ranges and absent routes retain staged fallback. No launch authority,
kernel artifact, Context implementation or Worker protocol is widened.

An explicit directed progress quantum can advance one authenticated,
already-started native owner blocking either endpoint. Shared-source siblings
serialize without acquiring a success dependency on each other. Selection
checks at most two endpoint slots and preserves the existing bounded graph walk.
Cancellation cannot refund published native custody; uncertainty retains both
original owners and dependent roots.

Queued typed consumers retain exact peer-access permits before native
extraction. Consumer drain can drive its retained peer prefix while a transfer
occupies the child, without issuing child compute during that custody. A
completed earlier FIFO entry no longer stops traversal before the consumer's
gate. Poll, wait and expired drain remain observation-only. These new consumer
interactions have scripted CPU coverage, not a new GPU arithmetic witness.

## Native Results

All six final-source MI300X cases pass on 2026-10-02. The new
`gfx942-runtime-directed-peer-copy-smoke` uses the public production constructor
with a deny-all compute authority and no modules or launches.

| Case | Ordered GPU ordinals | Result |
| --- | --- | --- |
| Directed chain A -> B -> C | 5, 6, 7 | Two changed rounds; native counter 0 -> 2 -> 4 |
| Reversed directed chain | 7, 6, 5 | Two changed rounds; native counter 0 -> 2 -> 4 |
| Shared-source fanout A -> B, A -> C | 5, 6, 7 | Two changed rounds; native counter 0 -> 2 -> 4 |
| Reversed fanout | 7, 6, 5 | Two changed rounds; native counter 0 -> 2 -> 4 |
| Existing deferred compute continuation | 6, 7 | Full bytes; four exact pipeline callbacks |
| Existing live sharded compute batches | 6, 7 | Two changed batches; twelve exact callbacks |

Each new copy transfers 8,388,581 bytes through a three-packet plan with a
37-byte final packet. Both peer roots are admitted before progress and their
public events are released. Only the final directed peer is driven thereafter;
fanout first seeds its first peer with two bounded steps. Those Pending seed
observations alone do not independently attest publication or physical overlap.
Chains use no such seed. Read-only observers and expired drain cannot advance
the prequeued roots.

Across the four new cases, all 16 native copies, 48 full-buffer readbacks and
16 exact callbacks pass. Every initial destination sentinel, final destination
byte and postcopy source byte is checked. Two independently derived payload
digests are:

- Round 0: `13a604a93f5755c7ccb103573377ab7c5e2ae60c5cc6b8c89923b360bfbd0364`.
- Round 1: `a3517b4755609712963df7efa4c015e8a16597b07412ff178931b8a224775e7e`.

Including the two unchanged controls, the campaign completes 22 peer copies,
eight compute launches (three are setup), five dependent D2H readbacks and
32 pipeline callbacks. All 22 peer completions have observed native counters.
The compute controls retain their finite trusted-artifact authorities; postcopy
source preservation is newly checked only by the copy-only cases.

Fresh UID/BDF, VRAM/activity, counted process-to-device and host-memory gates
pass before and after every case. All three uploaded binaries and the exact
owned scratch directory were removed; owned process absence and restored
baselines were checked. GPU 0's foreign work was not targeted. These point
observations are not an exclusive reservation or continuous attachment census.

## CPU And Source Checks

- Runtime: 2,109 passed, zero failures/filtering and 32 unchanged hardware ignores.
- All previous tests remain; seven directed-copy and six typed-consumer tests are added.
- Examples: 48 passed, including five new CLI, byte-oracle, packet and authority tests.
- Strict Clippy, no-default/hardware-feature checks, formatting and whitespace pass.
- All 32 exact source-control workflow commands pass. Nine guard files refresh
  only 16 SHA literals and seven inventory counts. All 76 proof files, predicates
  and expected proof/mutant counts remain unchanged; no solver was run here.
- The prior 1,925 KFD passes are authenticated reuse through the identical test
  executable, 344 unchanged source files and the accepted baseline archive, not
  a fresh KFD test run or a new kernel rebuild.

The rejected first attempt exposed two synthetic-host fixture errors. Correct
scripted host ownership and exact materialized completion records repair those
tests without weakening the runtime's launch guard. The second attempt passes
the runtime suite but rejects the new witness's unsupported digest formatting;
the existing sibling example's byte-to-hex pattern fixes it. Both attempts and
earlier diagnostics remain recorded, not counted as qualification.

## Evidence

`raw.tar.gz` contains `fe2o3-multigpu-directed-peer-20261002/`: exact CPU commands,
environments and rosters, source snapshots, metadata proposals, source-control
receipts, native campaign, cleanup and final `qualification.json`. Accepted
selection: CPU `attempt-03`, metadata `proposal-02`, source workflow
`attempt-03-after` and `hardware-01`. The auditor validates all 5,871 source
files, exact added tests, binary bindings, independent full-payload digests and
complete command/cleanup rosters. All 1,423 archived files were independently
read back and matched to their original contents. `SHA256SUMS` binds this README
and the archive.

New native witness SHA-256:
`8c0e6d088dcb458ab8abe1244194dc7a663609d41947bd2844f67412ce7ac141`.
Runtime test executable SHA-256:
`e5bb8d432de8f5f920d8135fa2a4ca0093a50f4ae47f026b83a60fc4d4763e17`.
Auditor SHA-256:
`7ecea8b77ff54c9557b122df5e77341047d4a6f6e57c098dfe80f84e66752fd3`.

## Remaining Work

The native directed path here is prequeued, not native admission of a new root
after endpoint extraction. Pending directed D2H chaining remains outside this
increment; its witness reads only settled results. General application kernel
evidence, native partial-failure isolation, eight-GPU execution, physical
overlap, full executable refinement and matched HIP/HSA performance remain
open. Same-process device reopen remains unsupported. This checkpoint does not
rerun the earlier seven-GPU campaign or close the multi-device milestone.
