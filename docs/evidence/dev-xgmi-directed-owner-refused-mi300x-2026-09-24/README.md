# Native Directed Owner Admission Refusal

Status: failed native graph qualification. The native process executed, but no
passing workload receipt or complete-buffer result exists. This packet preserves
the original failure; successful host checks do not upgrade the graph result.

## Attempt And Outcome

The signed source is `1d1992d07a0de94a683c9f6a1d8ddcd184262ea5`, built in the
owned detached worktree recorded by `verify.py`. GNU/musl CPU qualification is
copied byte-exactly from that signed checkpoint; release musl example tests and
all three Python suites, including compiled invalid-CLI checks, also pass.

GPUs 5 and 6 each pass fresh preflight, fixed two-second settled postflight and
fixed twenty-second delayed postflight. These are six sequential point-in-time
observations, not an exclusive reservation. The native workload exits 1 with:

`BackendRejected(... Busy, "overlapping XGMI copies require dependency")`.

Root and left were admitted; right was rejected while reading the shared A1
source. The internal owner cleanup report conservatively retains two streams,
two events, two submissions, five allocations, two writers and two readers until
process exit. It is **not** a successful runtime cleanup report. Workload stdout
is empty, parsed results are empty, and native campaign acceptance remains false.

The external controller collects exact remote records before deleting only its
marker-owned directory. Separate cleanup and path/process absence checks pass.
All recorded process groups are absent. The private remote path was
`/home/harsh/fe2o3-xgmi-directed-owner-20260924.5ea83059de917b7d`.

## Diagnosis And Correction

The production allocation-owner gate in `kfd_backend.rs` requires every earlier
owner of either allocation to be an explicit dependency. For right A1 -> A3,
A1's active owners are root and left. Right correctly names root, but not its
independent sibling left. The gate therefore treats a read/read share as a
hazard. The prior Context and directed-driver CPU mocks did not execute this
shared native admission gate.

Admission alone must not be relaxed: the native batch moves each allocation
mapping into one request, so selecting both readers would attempt to take A1
twice. The next implementation must jointly qualify:

- Exact directed-scalar read/read provenance at the actual shared admission
  gate, preserving dependencies for writers and conservative legacy profiles.
- A bounded allocation-disjoint FIFO publication prefix, without inventing a
  sibling dependency or skipping the first colliding entry.
- Full-ready-set flush refusal before effects when that set cannot be published
  together, preserving capacity precedence and the nonwaiting contract.
- Exact aggregate admission that recognizes healthy reader sharing but refuses
  a non-disjoint complete roster without partial publication or terminalization.
- CPU regressions through these production helpers, then a separately signed
  fresh native campaign of the unchanged diamond workload.

This is an admission/custody integration gap, not evidence of slow copy engines
or a failed GPU instruction. No retry replaces this failure. Native fault,
formal-refinement, aggregate-memory, performance and A1/A2 acceptance remain open.

## Offline Replay

The packet retains payload bytes, including the exact ELF and observer archive,
all local/remote records, collection manifests and the signed CPU prerequisite.
`verify.py` authenticates its transitive helpers, source and CPU archive before
replaying exact commands, exit statuses, input inventories, result absence,
original errors, both endpoint identities and timing, and external cleanup.
Observer UTC nanoseconds must fall inside each enclosing remote command receipt;
an older valid transcript cannot satisfy this freshness check after rehashing.

The verifier's successful exit means **this refusal packet is consistent**,
not that the native graph passed. Its result explicitly returns
`native_graph_qualified=false` and `owner_disposition=retained-until-process-exit`.
Mutation tests cover altered or promoted failures, forged results, source/payload
changes, stale/busy endpoints, shortened delays, missing cleanup and skipped
tests. Eleven additional CPU-content mutations bypass only the test's signed-copy
layer to exercise deeper command/type/order/result/source checks independently;
the production verifier always authenticates the signed copy first.
