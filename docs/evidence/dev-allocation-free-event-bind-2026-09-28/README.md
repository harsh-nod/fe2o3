# Allocation-Free Event Binding and Publication Prefix

Development evidence only. A1/A2 and issue #182 remain incomplete; accepted
lane checkpoints are unchanged. No GPU execution, HIP/HSA comparison or latency
measurement was performed. No remote hardware was used.

## Qualified Source

- Implementation, proof, CPU tests and initial checker:
  `dfcb5f8c932949773331494e15012fa918a689ac`.
- Final checker calibration and qualified source:
  `a9d3f4ff531a074eefb1f0bc954c5f1783a254e4`.

The batch event binder previously reserved an O(N) scratch Vec after native
publication. It now completes a read-only preflight and then updates the
existing event Vec and ledger entries in place. This removes that explicit
allocation and its post-publication allocation-refusal branch. Work remains
O(N); this is not a measured speedup. The later lane-event collection still
allocates, so the complete source-publication path is not allocation-free.

Production and Verus share the actual publication validation, phase marking,
occurrence construction, checked packet arithmetic, batch binder, forwarding
adapter and inline post-native success join. Common event definitions and
authentication were extracted without changing their existing contracts.

## Contract and Boundary

The binder preserves validation order: Ready, Published retention, roster
length, then each row's already-bound check, active-ledger authentication,
checked packet range and exact positional occurrence. No write occurs before
all rows pass. Refusal returns the original logical roster and leaves the
logical owner unchanged. Success changes only the selected ledger/event packet
IDs, preserving event identities, order, slot state, pins, readers, counters
and neighbors. Distinct row IDs are derived from validation, not assumed.

The actual inline success join proves three normal-return outcomes:

1. Publication marking refuses: unchanged logical owner and normalized Terminal
   completion error.
2. Marking succeeds but binding refuses: the Published slot prefix remains,
   ledger state is unchanged and the actual binding error becomes Terminal.
3. Both succeed: exact returned batch/roster and ledger packet-ID updates.

No healthy-owner, Ready, valid-roster or prior event-issuance premise is added
to this join. Marking retains its existing acceptance semantics, including
not requiring Ready or a valid packet range. The join consumes its error-path
tokens; it does not promise returned custody for those errors.

This is host-state correspondence for the shared executable bodies. It does
not prove native receipt authenticity, pre-native binding/issuance provenance,
the outer dispatch publication/terminalization wrapper, native execution,
destructors, panic/unwind or physical allocation identity. Call counts are not
part of this theorem. Zero allocation is a scoped CPU observation plus source
analysis, not an allocator theorem. Standard Verus/vstd/Z3/compiler trust
remains; the new root adds no custom external-body contract.

## Qualification

All six final campaigns bind the same signed source, **6225 unchanged source
hashes**, and the pinned **190-file / 129019839-byte** Verus release closure.
Each has original, exact relocated and closing whole-crate positives.

| Campaign | Stages | Whole-crate obligations | Executable negatives | Trust profile |
| --- | ---: | ---: | ---: | --- |
| Event binding | 36/36 | 43/0 errors | 28 | --no-cheating |
| Inline publication success join | 14/14 | 43/0 errors | 6 | --no-cheating |
| Bound cancellation regression | 31/31 | 26/0 errors | 23 | --no-cheating |
| Batch release regression | 32/32 | 29/0 errors | 24 | Two explicit reservation contracts |
| Source rollback regression | 18/18 | 51/0 errors | 10 | Same two contracts |
| Rollback adapters regression | 11/11 | 51/0 errors | 3 | Same two contracts |

Total: **142 passing stages and 94 executable negatives**. Obligation counts
overlap across roots and include helpers and derived checks; they are not
distinct runtime operations. Shared schema/core changes required the four
older campaigns to be rerun. Historical dispatch-epoch and native-source-failure
mutants are not included in these totals.

The new checker's six calibration groups check exact source graphs, production
wiring, mutation sites and strict diagnostics, including bounded-index and
closure postcondition errors only for their named functions. The exact
nine-file proof closure contains no additional trust input. Read-only review
independently checked the final source maps, relocated bodies, mutation deltas,
signatures, tool closures and trust-profile distinction.

- Final KFD regression: **1424 passed, zero failures**, with **320 construction
  tests explicitly excluded**. This is not a full KFD-suite run.
- Full runtime library: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests**: 42 KFD, 53 runtime (9 plus 44), 29 runtime-model.
- Strict all-feature/all-target Clippy, no-default-feature production checks,
  workspace/included-body formatting and diff checks pass.
- All **159 recorded process groups**, including the initially stopped
  campaign, were reaped and independently confirmed absent.

Three new CPU groups exercise N=3 binding. Success tests use literal low and
high packet-ID oracles, spare Vec capacity, mixed session/epoch identities,
same-slot aliases, readers and unrelated neighbors. They observe zero binder
allocations, unchanged Vec pointer/capacity/order, exact snapshots and ordinary
completion/release/recycle cleanup. Another group preserves existing acceptance
of coherent zero logical identities and hostile pin counts.

Fifteen hostile refusal cases test precedence, late-row failures, duplicate and
reordered IDs, missing/stale ledger rows and packet underflow/overflow. They
check zero allocations, unchanged complete owner snapshots and the exact
supplied Vec. These cases deliberately submit duplicated/altered test tokens,
then restore injected corruption and retry with retained genuine originals.
They are not genuine returned-token lifecycle tests.

## Retained Refusal

The initial signed campaign stopped at its first whole-crate positive. Verus
returned status 0 and 43/0, but the publication-controller classifier rejected
two bitvector informational notes. The final checker inherits the unchanged,
SHA-authenticated bound-cancellation controller's already-qualified exact-note
handling. Additional hostile-note calibration was added; no ancestor controller,
production body, proof or resource limit changed in that correction. The initial
run remains rejected evidence. All eight proof-development stdout/stderr pairs
are retained, including frontend and proof refusals.

## Remaining Work

Next is the actual pre-native event issuer and its fresh-ID, positional-row,
pin and neighbor provenance, together with moving the later lane-event output
reservation before native publication. That reservation change needs an
explicit early-error precedence contract. Issuance refusals must frame logical
state without pretending successful earlier reservations preserve allocation
identity. Available-slot pin provenance remains a separate caller invariant.

Native authority, outer dispatch settlement and terminalization, Context/Worker
composition, target scheduling, physical overlap, high-depth reuse, multi-device
and distributed faults, device-language and machine-code atomic/collective
refinement, and matched HIP/HSA measurements remain open. A1/A2 exit criteria
are not promoted. Issue #182 was checked on September 28 Los Angeles time
(September 29 UTC) and remained OPEN, last updated 2026-09-27T10:25:24Z.

## Archive

`raw.tar.gz` is **9913611 bytes**, containing **1586 manifested files plus the
inner SHA256SUMS manifest**. It retains signed relocated/mutated inputs,
development and stopped-run logs, final proof and CPU/static process records,
reproduction commands and the independent final audit.

An independent restore passed every hash and recursive comparison. Its owned
temporary directory was removed and absence checked. No remote machine or other
users' files were touched. The outer SHA256SUMS seals this report and archive. See
`COMMANDS.md`, `qualify.py`, `audit-final.py` and `final-audit.json` in the archive.
