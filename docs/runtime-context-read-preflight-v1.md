# V4-J2 Reader Contents And Ordered Preflight

Development proof packet, not full reader lifecycle verification. Accepted
checkpoints remain Native R125 CPU/test, Admission R118B C1-C3 and Resources
R116/V3. A1/A2 and #182 remain open.

The executable Verus model in
`crates/fe2o3-runtime-model/verus/context_read_preflight_v1.rs` imports the exact,
unchanged V4-J1 journal types and constructor. Its 103 obligations include 69
inherited J1 obligations and 34 new reader obligations. The
[qualification receipt](evidence/dev-v4j2-reader-preflight-2026-09-17/README.md)
separates these solver results from Rust tests and native acceptance.

## Proved Boundary

- Constructor admission checks reader capacity before the base journal's
  generation/capacity checks. On successful storage, the concrete contents have
  the exact inner journal, vacant leases, descending free stack, zero counts and
  initial incarnation 1.
- Exact allocation lookup checks slot, key and context before validating the
  pending-member backlink. Read validation then checks device, extent, nonempty
  checked range, pending writer, attempt epoch and content lineage in that order.
- Lease lookup checks the complete slot/incarnation/consumer identity before
  validating its stored request against the current allocation.
- Acquire preflight checks consumer, roster/output shape, clean output, capacity
  and incarnation headroom, then scans requests in caller order. Each request is
  validated before canonical order, grouped reader-count headroom and the selected
  free suffix slot. The first failing member determines the error.
- Release preflight checks evidence identity, nonempty roster and free storage
  headroom before scanning exact references, current requests, canonical order
  and grouped counts. Identical ranges acquired separately may be released in
  increasing incarnation order, including a canonical subset of live leases.
- The unread guard performs exact allocation lookup before testing its count.
  Mutable preflight wrappers frame every proof-side journal and reader contents field;
  acquisition also frames the complete output, on both success and rejection.

These are executable Vec/slice scans against explicit ordered decision specs,
not unordered validity predicates. The admission scans are linear in their
rosters and perform no explicit Vec allocation; physical allocation behavior is
outside the contents proof.

## Premises And Nonclaims

The batch/guard functions require only that reader-count and allocation vectors
have equal lengths for direct indexing. Acquisition additionally requires the
request count to fit u64, as it does on the supported 32/64-bit targets. They do
not assume clean output, valid or sorted requests, current references, matching
evidence, sufficient capacity, correct counts, unique free slots or a healthy
reader invariant. Malformed pending backlinks, occupied/out-of-range free slots,
zero incarnations and incorrect counts remain available rejection cases.

The release capacity parameter represents the production `free_reads.capacity()`
observation; authenticating that observation and physical storage is separate.
Quiescence evidence remains an inert identity premise, not native authentication.
Both writer kinds are accepted. Epoch and lineage may differ, and lineage zero
is legal; neither case proves initialized input.

Preflight success does not establish safe commit. Duplicate free slots, incorrect
counts, duplicate base allocation identities or orphan members can violate the
healthy invariant without being rejected by these local guards. Still open:

- Exact acquisition/release commit effects, incarnation freshness and invariant
  preservation, including partition uniqueness and reader-count conservation.
- Base allocation/member invariant reachability and composition with writer
  mutation, settlement, retirement and Unknown disposal.
- Mechanical production Rust correspondence, storage allocation/capacity/pointer
  preservation, panics/unwind and whole-call atomicity.
- Context ordinary/generated ownership domains, authenticated native completion,
  Worker V3/machine refinement, concurrency/fault qualification and performance.

## Qualification

`check-read-preflight.py` admits only the exact initial pinned J1 import,
independently audits that dependency and applies the unchanged forbidden-source
policy to the remaining source. Every solver case gets its own exact dependency
copy, with source/tool checks before and after execution. Pinned Python utilities
are compiled from the verified source bytes, bypassing stale bytecode caches.

Twenty-one reversible executable-body mutations retain the original contracts.
Acceptance requires normal exit 1, exactly 102 verified obligations and one
intended postcondition error, with exact source spans in the mutated function.
Compile errors, wrong-module errors, unrelated failures, timeouts and extra
diagnostics fail qualification. Positive runs before and after require 103/0.
Parser, import-policy, stale-bytecode and process-cleanup self-tests are included.
The global `verify-verus.sh` runs the reader campaign without weakening its
existing source policy or changing historical J1 source/checker pins.
The runner's structural auditor independently checks the reader campaign wiring
and six new adverse integration fixtures; the 686 legacy negative files remain
unchanged. The preliminary integration failure and repaired qualification are
preserved as separate cohorts in the receipt.

Rust witnesses strengthen three existing error matrices to assert exact enums
and add five tests for constructor contents and simultaneous-fault ordering,
grouped counts, selected slots, evidence and physical headroom. Existing ABA,
multi-range and 4,000-step partition traces remain executable corroboration,
not proofs of commits. Malformed base pending backlinks are modeled by the proof
but are not constructed through the safe Rust API in these tests.
