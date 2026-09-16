# Context Version Journal Issuance Proof

This V4-J1 development packet integrates the previously external V15 candidate
into `crates/fe2o3-runtime-model/verus/context_version_journal_issuance_v1.rs`.
It strengthens registration and abort to operate on the complete proof-side
`JournalContentsV1`, with postconditions over actual before/after fields.
See the [qualification receipt](evidence/dev-v4j1-issuance-2026-09-16/README.md)
for source identities, solver results, negative campaigns and limitations.

The accepted Resources checkpoint remains R116/V3. This packet does not add a
journal to production `RuntimeContextV1`, admit generated kernels, authenticate
completion evidence or close A1/A2, issue #182 or HIP/HSA parity.

## Verified Boundary

The model uses complete writer keys (Context generation, local ID and kind),
exact slot references, Reserved/Pending/Unknown entries and separate allocation
and writer capacities. Registration history is ghost state: its watermark is
the maximum successfully registered local ID, not the maximum live ID or the
external Context allocator's complete history.

| Operation | Contents-Level Contract |
| --- | --- |
| Constructor | Context validation precedes capacity classification; admitted successful output has five exact scalars and seven initialized vectors, including descending free stacks. |
| Register | Exact ordered error classification, unchanged contents on rejection, selected free-slot removal, Reserved insertion, checked count increment and watermark advancement. |
| Lookup | Exact in-range Reserved key and Context identity; older valid references do not require the current watermark. |
| Abort | Exact lookup before logical/observed-physical headroom and count-underflow checks; clear/push/decrement on success, with unchanged watermark/history. |
| Traces | Conditional partition/count/history preservation, older-reference framing and stale-reference non-revival through abort/reuse. |

Registration and abort executable contracts do not require a global issuance
invariant. Their specific arithmetic and reference rejection cases therefore
remain observable on malformed prestates. This does not mean every corrupted
count is detected: a nonzero, in-range but inconsistent count can pass local
guards. Global invariant preservation requires an invariant prestate.

The new whole-journal wrappers prove that the other three scalar fields and
five vector contents remain unchanged. Their projection reads actual post-call
fields instead of inserting `pre.other` by construction. The lower execution
relations cover the changed writer/free vectors, watermark and Reserved count.
Mixed-phase witnesses are inhabited prestates, not proofs that issuance alone
can reach Pending/Unknown or that V2/V3 membership is consistent.

## Qualification

The canonical proof is source-pinned and part of the ordinary runtime-model
proof runner, source audit and success transcript. A separately pinned mutation
campaign generates 21 independent whole-module candidates from those exact
bytes, changing only one selected executable body while retaining specifications
and all other proof code. It covers eight constructor errors, five registration
errors, six abort errors and two whole-journal framing errors.

The campaign checks complete-crate counts and structured diagnostics. A mutation
must exit normally with exactly its declared verification result and one
postcondition error at the exact intended source span. Compilation errors,
timeouts, signals, extra diagnostics and unrelated proof failures are rejected.
Unchanged positives bracket the campaign. The existing 686 expected-negative
files retain their original separate contract.

Three Rust tests cover canonical constructor contents/free order, fresh-vacant
registration count overflow/bound rejection, and genuine-Reserved abort
underflow with valid return headroom. Rejection checks compare complete snapshots
and all seven original storage pointer/capacity pairs, with exact two/one indexed
accesses. These are executable tests, not a proof of Rust correspondence or cost.

## Remaining Correspondence

- Constructor loops prove contents but do not model Rust `try_reserve_exact`,
  `resize_with`, iterator extension, `StorageAllocationFailed` or partial-allocation
  cleanup. Result equality does not prove initialization happens after preflight.
- Storage capacities are explicit ghost labels; abort receives an external
  capacity observation. No proof authenticates actual `Vec` capacity/address,
  allocation behavior, nonallocation, ownership or whole-call unwind.
- The proof and Rust model are reviewed against each other and bound in the
  receipt, but their correspondence is not mechanically verified.
- V2/V3 membership and settlement proof composition remain outstanding.
- V5/V6 still need genuine Context identities, provisional registration before
  effects, allocation retirement, move-only tickets and settlement before
  callbacks. Generic backend errors remain insufficient NoEffect authority.
- All-writer coverage, Unknown recovery, input leases, aggregate retained-memory
  bounds, native execution and matched performance qualification remain separate.
