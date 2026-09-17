# Issue 271 Guarded Receipt Checkpoint

Reviewable WIP, not production activation or tutorial qualification. M5 exact
optimized-program verification and M7 production integration remain active.
M8 is unqualified; no acceptance milestone is newly complete. Do not merge
this historical tree wholesale over current main.

## Exact Source

- Host: XSJHARMENON01.
- Checkout: /home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913.
- Checkout HEAD: 10b190b8267c0c4011b4ef6bbb52680a3c44b391.
- Source: 5,075 files, manifest SHA256
  a809aed19dd8fd9685013b9b7ca307451a155958068a7b740e9eec4460a4adb6.
- Previous published WIP: 248540fcca6fcc4ccd9e0701954a0efaa9461741.

Snapshot construction verifies every source Git blob and preserves the source
checkout HEAD/index. This status is outside the source manifest. The batch
changes four source paths, including two new files and one formatting-only
legacy receipt file, plus this status.

## Implemented

The preceding guarded formal analysis now has a bounded inert V3 receipt format
with extraction policy 2. It preserves conditional domains, symbolic bounds,
whole-allocation alias regions, and ordered conflict endpoints. Explicit
predicates retain both Read and Write access kinds. Body order is not inferred
from numeric operation identifiers.

A closed current-format facade selects legacy encoding when exactly representable
and V3 only for actual guarded obligations. Legacy V1/V2 APIs, identities, exact
bytes and unsupported-representation refusals remain intact. Decoding checks
closed policy/version pairs, row order/uniqueness, exact subjects, ranges and
conflict joins. It does not create compiler ownership or artifact/runtime
authority. Bits32 can be inert metadata; live Complete analysis remains Bits64.

Byte, record and auxiliary-storage caps remain explicit. Sorting, lookup and
actual reserved capacities are bounded and charged. Sixteen new tests cover
round trips, genuine mixed accesses, aliases/conflicts, legacy compatibility,
malformed policies/rows, resource boundaries and inert foreign identifiers.

## Exact Qualification

On the final formatted source above, with source/helper guards passing:

- 820 kernel-IR tests passed across 41 suites, zero failed, one pre-existing
  ignored full-block-count stress test. This includes all 16 new codec tests.
  Logs: v257-clean-v360-guarded-receipt-final-kernel-ir.BQXA9U7e.
- Full-workspace formatting passed.
  Logs: v257-clean-v360-guarded-receipt-core-formatted-check.vpQlmhTN.
- 426 lowerer library tests and 896 backend library tests passed, zero failed
  or ignored. Logs: v257-clean-v361-guarded-receipt-final-consumers.WuLSJvMd.

Total: 2,142 passing tests on the exact final source, with one existing ignore.

The pre-format source 1c248503005f625993716e582703cb4fdba9e87740fd0d3550ccc3e83266c7fc
passed 820 kernel-IR, 426 lowerer and 896 backend tests: 2,142 total, zero
failures, one existing ignore. The final two-file formatting change preserved
the new V3 implementation and tests byte-for-byte. The final-source rerun above
is separate evidence; earlier results are not transferred across snapshots.

## Remaining Integration

Active lowerer evidence, semantic lineage and multiroot consumers still use
legacy receipt APIs. Their guarded-format/policy migration and the exact
core-proved versus ranked-pending access partition are separate work. No old
admission or artifact gate is bypassed by the new inert codec.

The guarded analysis and live source/output test foundations must be composed
explicitly with current main. Preserve its execution-KIR V15, semantic-MIR V29
and aggregate/RustCall ABI boundaries. Historical WIP tests do not qualify that
composition. No new main integration is claimed here.

Separate initializer WIP 17f786cca73585a6ca39554f3d25595a3d9eb828 retains helper
local-memory obligations but still has the genuine HelperEffectsUnavailable
positive failure. Real Rust shape observations and source/Load/latest-Store,
call and N/B/O relations are prerequisites to helper acceptance, not completed
by this codec.

Broader source rules, managed metadata custody, runtime/target binding,
production activation and legacy retirement remain open. M8 all-tutorial
compilation, simulation, target-matched hardware and website qualification
remain open. Approved Verus runtime qualification is deferred, not passed.
No new extractor, actual-collector, GPU or website qualification is claimed
for this source. Separate issue 272 WIP is not changed by this checkpoint.
