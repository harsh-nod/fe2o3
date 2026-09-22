# Qualified Actual Producer-Read Queries

Source: b5157a6e57fa57b9e99dcac8e5822d8455d55b42.

- Verus: two whole-root runs, each 546 verified and zero errors at default solver limits.
- Twenty-nine controls: twenty-five executable mutations, one raw-domain contract sensitivity, one projection and two witness sensitivities.
- CPU: 908 unit tests and 27 doctests passed; fifteen tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- One selected release benchmark passed, producing 1,456 rows over 104 case combinations.
- No native GPU execution or HIP/HSA measurements were run.

Actual writer lookup and producer status, validation, retained inspection, lookup and
public status now share production bodies. Raw contracts are unconditional: no custody,
count/storage shape, capacity or successful-admission premise is introduced.
Four paired harnesses independently execute actual and historical queries, preserving
exact results and error precedence on represented owners and mapped inputs.

Successful public status eliminates a redundant traversal: three versus six journal
accesses for Pending/Unknown, and one versus two for Success/NoEffect. Other operations
and rejected queries retain their original counts. These counters measure journal
allocation/backlink/writer access, not every reservation or vector operation.

Synthetic raw witnesses run all four query operations for Pending, Unknown, Success
and NoEffect while irrelevant free/count/incarnation storage is malformed. Pending
and Unknown writer metadata may contain invalid heads and maximal counts; read-only
lookup does not validate retained chains. Resolved witnesses use either an out-of-range
old producer slot or the same slot occupied by an unrelated Reserved writer. A stale
reservation reference rejects before a corrupt stored device; the exact reference
exposes that device error. These are not acquisition/settlement reachability proofs.

CPU fixtures use actual lifecycle setup and retain a second reservation, compare
complete snapshots, storage identities, exact results and journal access counts
against frozen pre-change execution. They isolate error order, range boundaries,
full identities, nonzero prior lineage, resolved writer reuse and device-generation
mismatch. Writer leaf tests preserve Reserved/Pending/Unknown and exact counts,
including zero and maximal identifiers/counts, without chain-admission semantics.

The 546 obligations include inherited proofs. Historical query declarations are an
exact authenticated projection into one type universe, not a duplicate custody root.
Reviewed public/private adapters and unchanged baseline dependencies are source-bound.
Production's typed journal receiver uses immutable Deref; proof execution explicitly
projects the journal field. That unchanged trait adapter is authenticated, not newly
proved as a trait implementation. Physical storage, allocator/unwind behavior,
construction and lifecycle reachability, producer admission/release and remaining
wrappers remain open. Native pending-consumer admission stays closed; Gate 1 and full
HIP/HSA parity remain unproved.

## Instrumented CPU Comparison

Ratios are shared/frozen medians over seven alternating rounds. Lower is faster.
These are non-exclusive CPU regression observations with test-only journal counters,
per-call timer, result mapping and dispatch overhead inside the measurement. Counter
reset, assertions and storage checks are outside timing. The same immutable owner
needs no structural restoration. Instrumentation may affect compiler optimization;
these are not uninstrumented production latency, GPU comparisons or an
orders-of-magnitude claim. All short/error paths and regressions are retained.

| Capacity | State/fault | Operation | Shared ns | Frozen ns | Shared/frozen |
| ---: | --- | --- | ---: | ---: | ---: |
| 8 | pending | status | 74.54 | 77.79 | 0.958 |
| 8 | pending | validate | 74.54 | 74.74 | 0.997 |
| 8 | pending | lookup | 97.48 | 97.69 | 0.998 |
| 8 | pending | query | 83.09 | 128.75 | 0.645 |
| 8 | unknown | status | 78.32 | 77.14 | 1.015 |
| 8 | unknown | validate | 82.71 | 83.82 | 0.987 |
| 8 | unknown | lookup | 107.59 | 110.85 | 0.971 |
| 8 | unknown | query | 83.48 | 136.02 | 0.614 |
| 8 | success | status | 74.82 | 82.26 | 0.910 |
| 8 | success | validate | 68.23 | 69.92 | 0.976 |
| 8 | success | lookup | 79.95 | 109.91 | 0.727 |
| 8 | success | query | 74.14 | 94.00 | 0.789 |
| 8 | no_effect | status | 65.10 | 66.65 | 0.977 |
| 8 | no_effect | validate | 64.28 | 69.74 | 0.922 |
| 8 | no_effect | lookup | 81.63 | 88.41 | 0.923 |
| 8 | no_effect | query | 66.87 | 97.30 | 0.687 |
| 8 | success_reused | status | 64.73 | 62.53 | 1.035 |
| 8 | success_reused | validate | 67.16 | 63.27 | 1.061 |
| 8 | success_reused | lookup | 80.91 | 86.82 | 0.932 |
| 8 | success_reused | query | 72.35 | 96.34 | 0.751 |
| 8 | no_effect_reused | status | 69.81 | 73.02 | 0.956 |
| 8 | no_effect_reused | validate | 72.34 | 71.66 | 1.010 |
| 8 | no_effect_reused | lookup | 87.80 | 96.58 | 0.909 |
| 8 | no_effect_reused | query | 76.68 | 112.87 | 0.679 |
| 8 | allocation | status | 63.28 | 59.78 | 1.059 |
| 8 | allocation | validate | 60.33 | 59.99 | 1.006 |
| 8 | allocation | lookup | 64.03 | 64.70 | 0.990 |
| 8 | allocation | query | 65.85 | 75.79 | 0.869 |
| 8 | backlink | status | 61.41 | 68.02 | 0.903 |
| 8 | backlink | validate | 58.07 | 59.05 | 0.983 |
| 8 | backlink | lookup | 72.24 | 70.22 | 1.029 |
| 8 | backlink | query | 56.99 | 66.82 | 0.853 |
| 8 | device | status | 69.02 | 74.33 | 0.929 |
| 8 | device | validate | 71.41 | 71.94 | 0.993 |
| 8 | device | lookup | 75.34 | 79.85 | 0.943 |
| 8 | device | query | 80.26 | 95.27 | 0.842 |
| 8 | writer | status | 87.86 | 88.06 | 0.998 |
| 8 | writer | validate | 76.77 | 81.75 | 0.939 |
| 8 | writer | lookup | 87.83 | 93.14 | 0.943 |
| 8 | writer | query | 84.40 | 94.12 | 0.897 |
| 8 | reference | status | 77.04 | 79.35 | 0.971 |
| 8 | reference | validate | 85.02 | 82.29 | 1.033 |
| 8 | reference | lookup | 62.46 | 54.35 | 1.149 |
| 8 | reference | query | 56.58 | 58.34 | 0.970 |
| 8 | pending_lineage | status | 83.49 | 79.64 | 1.048 |
| 8 | pending_lineage | validate | 84.04 | 82.64 | 1.017 |
| 8 | pending_lineage | lookup | 100.61 | 98.33 | 1.023 |
| 8 | pending_lineage | query | 90.76 | 104.56 | 0.868 |
| 8 | resolved_lineage | status | 69.70 | 73.51 | 0.948 |
| 8 | resolved_lineage | validate | 67.49 | 72.20 | 0.935 |
| 8 | resolved_lineage | lookup | 82.49 | 81.60 | 1.011 |
| 8 | resolved_lineage | query | 72.91 | 92.46 | 0.789 |
| 65536 | pending | status | 85.78 | 86.32 | 0.994 |
| 65536 | pending | validate | 83.56 | 81.61 | 1.024 |
| 65536 | pending | lookup | 100.67 | 111.19 | 0.905 |
| 65536 | pending | query | 89.64 | 148.39 | 0.604 |
| 65536 | unknown | status | 82.97 | 81.10 | 1.023 |
| 65536 | unknown | validate | 89.87 | 85.24 | 1.054 |
| 65536 | unknown | lookup | 103.59 | 98.63 | 1.050 |
| 65536 | unknown | query | 93.45 | 137.99 | 0.677 |
| 65536 | success | status | 66.07 | 69.54 | 0.950 |
| 65536 | success | validate | 66.67 | 72.17 | 0.924 |
| 65536 | success | lookup | 93.00 | 93.20 | 0.998 |
| 65536 | success | query | 82.33 | 111.23 | 0.740 |
| 65536 | no_effect | status | 71.85 | 68.34 | 1.051 |
| 65536 | no_effect | validate | 71.83 | 64.43 | 1.115 |
| 65536 | no_effect | lookup | 91.31 | 86.05 | 1.061 |
| 65536 | no_effect | query | 77.10 | 115.11 | 0.670 |
| 65536 | success_reused | status | 69.04 | 71.08 | 0.971 |
| 65536 | success_reused | validate | 65.67 | 70.95 | 0.926 |
| 65536 | success_reused | lookup | 80.72 | 90.96 | 0.887 |
| 65536 | success_reused | query | 71.62 | 105.74 | 0.677 |
| 65536 | no_effect_reused | status | 61.87 | 64.07 | 0.966 |
| 65536 | no_effect_reused | validate | 71.20 | 69.80 | 1.020 |
| 65536 | no_effect_reused | lookup | 81.13 | 87.32 | 0.929 |
| 65536 | no_effect_reused | query | 64.68 | 96.65 | 0.669 |
| 65536 | allocation | status | 53.72 | 53.16 | 1.010 |
| 65536 | allocation | validate | 61.15 | 63.23 | 0.967 |
| 65536 | allocation | lookup | 71.93 | 68.66 | 1.048 |
| 65536 | allocation | query | 67.25 | 66.03 | 1.019 |
| 65536 | backlink | status | 56.61 | 57.00 | 0.993 |
| 65536 | backlink | validate | 54.23 | 56.47 | 0.960 |
| 65536 | backlink | lookup | 59.73 | 67.69 | 0.882 |
| 65536 | backlink | query | 63.63 | 74.60 | 0.853 |
| 65536 | device | status | 77.22 | 77.48 | 0.997 |
| 65536 | device | validate | 69.45 | 71.81 | 0.967 |
| 65536 | device | lookup | 71.85 | 82.00 | 0.876 |
| 65536 | device | query | 71.46 | 77.54 | 0.922 |
| 65536 | writer | status | 73.71 | 72.88 | 1.011 |
| 65536 | writer | validate | 72.89 | 76.01 | 0.959 |
| 65536 | writer | lookup | 104.45 | 103.27 | 1.011 |
| 65536 | writer | query | 93.93 | 106.43 | 0.883 |
| 65536 | reference | status | 87.19 | 73.74 | 1.182 |
| 65536 | reference | validate | 76.43 | 79.29 | 0.964 |
| 65536 | reference | lookup | 57.87 | 50.38 | 1.149 |
| 65536 | reference | query | 49.94 | 54.55 | 0.915 |
| 65536 | pending_lineage | status | 81.23 | 80.27 | 1.012 |
| 65536 | pending_lineage | validate | 80.71 | 82.18 | 0.982 |
| 65536 | pending_lineage | lookup | 89.94 | 92.92 | 0.968 |
| 65536 | pending_lineage | query | 86.48 | 98.40 | 0.879 |
| 65536 | resolved_lineage | status | 70.95 | 69.62 | 1.019 |
| 65536 | resolved_lineage | validate | 67.66 | 74.57 | 0.907 |
| 65536 | resolved_lineage | lookup | 76.57 | 85.28 | 0.898 |
| 65536 | resolved_lineage | query | 73.94 | 81.03 | 0.913 |
