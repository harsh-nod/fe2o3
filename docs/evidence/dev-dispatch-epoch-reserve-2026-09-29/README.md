# Epoch Reservation And Heap-Backed Templates

Qualified on 2026-09-29 UTC at signed source
`e9cd6fa17265345f3547e818675a59f1e7de280f`.
Implementation parent: `a773f33653527a21cb6be80881df6fbe6e012d5f`.
This is host reservation and stack-safety qualification, not an A1/A2 exit or
HIP/HSA parity claim. Three native agents reviewed custody, tests and proof
boundaries read-only; primary owned integration and qualification.

## Implementation

The actual capacity, preflight and epoch-reservation bodies are shared with
Verus. The proof establishes exact ordered refusal with unchanged logical owner
state, or the first reusable Vacant slot with exactly the queue binding, global
generation and selected slot updated. Exhausted vacant slots are skipped;
all-occupied capacity refusal precedes global generation exhaustion. Roster
validation and the u16 slot-index boundary precede every mutation. Arbitrary
neighboring slots, payload and metadata are framed without adding constructor
invariants to raw states. Reserve/cancel/reserve and exhausted-prefix witnesses
exercise the actual bodies. Scanning remains linear and allocation-free.

CPU testing exposed a stack overflow from returning the template array by value
at N=8193. Resource-derived templates now remain boxed through ordinary,
dependency-source and persistent live paths into the completion binder. The
production and CPU traits carry the box without an array-unbox/rebox roundtrip.
Small-array convenience APIs remain; this is not blanket stack safety for every
API. Allocation precedes epoch reservation; allocator abort/unwind behavior is
not included in the normal-return claims.

## Results

| Check | Result |
| --- | --- |
| Signed reservation campaign | 36 stages; all 28 executable negatives rejected logically |
| Reservation proof | 23 overlapping obligations, `--no-cheating`, four-file relocated closure |
| Signed cancellation regression | 30 stages; all 22 executable negatives rejected logically |
| Cancellation proof | 16 obligations, `--no-cheating`, two-file relocated closure |
| Source continuity | 6,248 unchanged hashes across both campaigns and CPU qualification |
| KFD CPU | 1,448 passed; 320 construction tests excluded |
| Runtime CPU | 1,828 passed; 30 hardware tests ignored |
| Doctests | 124 passed |
| Static checks | Strict Clippy, production build and workspace/included formatting passed |
| Affected positive proofs | Completion preparation 55, source rollback 51, source failure 21 |
| Independent audit | Signed blobs, relocation/mutant bytes and selectors checked; all 108 recorded process groups absent |

Nine new CPU groups cover accounted owners at Default64/Qualification1024,
4,608 independent raw-state oracle cases, index/generation boundaries, genuine
N=3 resources and permuted templates, ordered competing refusals, neighboring
custody and corrected retries. Reservation observes zero allocations in the
scoped test, not a latency result. Explicit 2 MiB threads cover N=8192/8193
template admission and two repeated N=8192 dependency-source no-effect retries
on both lanes, including actual event release and bound/epoch cancellation.

The complete old cancellation mutation suite was rerun. The three other affected
roots were rerun as positive proofs only, not their historical mutation suites.
Preparation and rollback retain their two disclosed standard-library contracts;
source failure retains `--no-cheating`.

The stopped signed reservation campaign remains rejected: the truncation mutant
produced a cast-range recommendation in addition to the intended postcondition
failure. Modulo 65536 now expresses the same truncation without that diagnostic;
the strict classifier was not relaxed. The complete campaign was rerun. The
independent auditor's result/manifest filename collision was corrected by deriving
manifest names from exact failing-stage entries, retaining all count/byte checks.

## Trust And Limits

Reservation refines the retained table's logical payload projection, not its
accounting adapter, constructor provenance or native authority. Resource-derived
template construction has CPU coverage but is not yet fully source-refined.
The maximum-size successful template fixture repeats genuine retained metadata;
it does not establish maximum-size public preparation reachability. The
dependency-source stack test uses CPU no-effect receipts, not native publication.

Mapping authentication, native publication/receipt truth, protected Worker
execution, outer terminalization/unwind and hardware memory ordering remain
outside these claims. No GPU or matched HIP/HSA performance work was run.
A0-A7 exit statuses and accepted lane checkpoints are unchanged.

## Evidence And Reproduction

`raw.tar.gz` is 4,366,597 bytes and contains 787 manifested files plus the inner
manifest: successful and stopped campaigns, development logs, CPU/static/proof
regressions, signed-source manifests, process records, independent audit and
`COMMANDS.md`. The inner `SHA256SUMS` checks extracted bytes; the outer manifest
checks this README and archive. A fresh extraction passed all hashes and exact
tree comparison before publication. Only manifested restore files were removed,
then empty directories; the owned restore path is absent. No MI300X artifacts or
processes were created.

Next: refine complete resource-derived `bind_templates` and Ordinary refusal
classification, then compose that handoff with completion binding. Native
authority, production execution and all broader milestone exit gates remain open.
