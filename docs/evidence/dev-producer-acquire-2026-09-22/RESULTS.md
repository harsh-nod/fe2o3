# Qualified Actual Producer-Read Acquisition

Source: edae8293a6ff03f930892351828740c47ce2f71a.

- Verus: two whole-root runs, each 625 verified and zero errors at default solver limits.
- Thirty-three controls: twenty-eight executable mutations, one conditional-domain contract sensitivity, one projection and three witness sensitivities.
- CPU: 914 unit tests and 27 doctests passed; sixteen tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- One selected release benchmark passed, producing 1,008 rows over 72 cases.
- No native GPU execution or HIP/HSA measurements were run.

Production and proof share stable/producer retained counts, combined budget, both
capacity validators, producer header, ordered preflight and sequential commit into
the public mutable slice. Actual and historical acquisition execute independently
under represented producer custody. Their exact results, complete owner state and
output correspond; rejection is atomic and successful admission preserves custody.
A paired harness also executes actual and historical producer capacity validation.

Raw actual safety has a path-sensitive domain. Budget arithmetic is required only
after the header prefix passes; count storage covers journal allocation storage
only after the full header passes. Roster length must fit u64. Getter arithmetic
has its own subtraction/overflow bounds. These contracts assume neither a desired
success result, selected-slot uniqueness nor a producer issuer-validity predicate.
Count coverage is sufficient, not the minimum bound along every reached path.

Six synthetic fixture/witness functions cover represented live acquisition, late
rejection and occupied-output replay; raw duplicate destinations with sequential
overwrite; matching Pending producer ID zero; malformed-budget/count prefix errors;
and unequal raw arena lengths with budget-before-epoch precedence and overflow.
The literal fixture mirrors a logical constructor plus synthetic Pending metadata.
It does not prove fallible Rust construction or actual Begin reachability.
Alias and unequal-arena witnesses do not claim valid custody for malformed states.

CPU fixtures start from the actual lifecycle and retain one producer reservation
and one stable lease. They compare frozen execution, exact results, full snapshots,
output, storage identity and journal access counts. Missing-count fixtures with
early allocation/device errors exceed the formal contract's conservative
header-success count-storage scope; those are concrete CPU checks only.

The 625 obligations include inherited proofs. Forty-three historical declarations
are extracted exactly into one logical type universe. Frozen methods, unchanged
dependencies and reviewed adapters are authenticated. The generation adapter uses
two unchanged immutable Deref implementations in Rust and an explicit field path
in proof. This is a source-bound adapter, not a new proof of trait dispatch.
Producer release, outer stable wrappers, physical storage, allocator/unwind,
construction/reachability and producer-quiescence authenticity remain open.
Native pending-consumer admission stays closed; Gate 1 and full HIP/HSA parity
remain unproved.

## Instrumented CPU Comparison

Ratios are shared/frozen medians over seven alternating rounds. Lower is faster.
These are non-exclusive CPU regression observations with test-only journal counters
and per-call timer/dispatch overhead. The same owner is restored by an O(k) reset
outside timing, including counters; assertions and storage comparisons are also
outside timing. Reset preserves the retained entries and requires no release path.
All cases retain equal variant-specific access counts: zero for output/epoch
header errors, 3k-1 for a late device error, 3k otherwise. These counters measure
journal allocation/backlink/writer access, not every reservation/vector operation.
Instrumentation may affect optimization. These are not uninstrumented production
latencies, GPU comparisons or an orders-of-magnitude claim. Regressions are retained.

| Roster | Capacity | Shape | Fault | Shared ns | Frozen ns | Shared/frozen |
| ---: | ---: | --- | --- | ---: | ---: | ---: |
| 2 | 1024 | grouped | none | 138.16 | 142.60 | 0.969 |
| 2 | 1024 | grouped | device | 108.08 | 106.13 | 1.018 |
| 2 | 1024 | grouped | count | 125.55 | 123.29 | 1.018 |
| 2 | 1024 | grouped | free | 128.12 | 127.18 | 1.007 |
| 2 | 1024 | grouped | output | 49.29 | 45.52 | 1.083 |
| 2 | 1024 | grouped | epoch | 51.35 | 49.21 | 1.044 |
| 2 | 1024 | grouped | alias | 137.72 | 135.70 | 1.015 |
| 2 | 1024 | distinct | none | 134.90 | 140.35 | 0.961 |
| 2 | 1024 | distinct | device | 102.91 | 99.49 | 1.034 |
| 2 | 1024 | distinct | count | 101.76 | 108.42 | 0.939 |
| 2 | 1024 | distinct | free | 103.20 | 97.37 | 1.060 |
| 2 | 1024 | distinct | output | 42.97 | 41.28 | 1.041 |
| 2 | 1024 | distinct | epoch | 49.19 | 45.06 | 1.092 |
| 2 | 1024 | distinct | alias | 139.51 | 144.84 | 0.963 |
| 8 | 1024 | grouped | none | 418.96 | 415.71 | 1.008 |
| 8 | 1024 | grouped | device | 330.64 | 316.22 | 1.046 |
| 8 | 1024 | grouped | count | 345.98 | 337.66 | 1.025 |
| 8 | 1024 | grouped | free | 367.90 | 365.71 | 1.006 |
| 8 | 1024 | grouped | output | 56.12 | 51.18 | 1.097 |
| 8 | 1024 | grouped | epoch | 54.83 | 53.93 | 1.017 |
| 8 | 1024 | grouped | alias | 435.41 | 407.64 | 1.068 |
| 8 | 1024 | distinct | none | 423.95 | 453.70 | 0.934 |
| 8 | 1024 | distinct | device | 346.14 | 334.67 | 1.034 |
| 8 | 1024 | distinct | count | 339.79 | 338.75 | 1.003 |
| 8 | 1024 | distinct | free | 330.99 | 327.48 | 1.011 |
| 8 | 1024 | distinct | output | 53.33 | 53.95 | 0.989 |
| 8 | 1024 | distinct | epoch | 58.75 | 55.40 | 1.060 |
| 8 | 1024 | distinct | alias | 415.87 | 434.11 | 0.958 |
| 64 | 1024 | grouped | none | 2860.27 | 3002.64 | 0.953 |
| 64 | 1024 | grouped | device | 2496.41 | 2405.78 | 1.038 |
| 64 | 1024 | grouped | count | 2444.00 | 2454.38 | 0.996 |
| 64 | 1024 | grouped | free | 2605.39 | 2531.34 | 1.029 |
| 64 | 1024 | grouped | output | 105.01 | 93.40 | 1.124 |
| 64 | 1024 | grouped | epoch | 90.38 | 101.52 | 0.890 |
| 64 | 1024 | grouped | alias | 2826.08 | 3033.19 | 0.932 |
| 64 | 1024 | distinct | none | 2619.60 | 2937.55 | 0.892 |
| 64 | 1024 | distinct | device | 2522.23 | 2425.65 | 1.040 |
| 64 | 1024 | distinct | count | 2450.05 | 2516.99 | 0.973 |
| 64 | 1024 | distinct | free | 2462.02 | 2455.93 | 1.002 |
| 64 | 1024 | distinct | output | 93.99 | 91.50 | 1.027 |
| 64 | 1024 | distinct | epoch | 66.90 | 72.38 | 0.924 |
| 64 | 1024 | distinct | alias | 2949.92 | 2960.79 | 0.996 |
| 512 | 1024 | grouped | none | 25868.48 | 25025.35 | 1.034 |
| 512 | 1024 | grouped | device | 15950.76 | 15424.20 | 1.034 |
| 512 | 1024 | grouped | count | 19131.16 | 18484.07 | 1.035 |
| 512 | 1024 | grouped | free | 18928.71 | 18378.68 | 1.030 |
| 512 | 1024 | grouped | output | 452.22 | 565.63 | 0.799 |
| 512 | 1024 | grouped | epoch | 395.61 | 449.41 | 0.880 |
| 512 | 1024 | grouped | alias | 22292.68 | 22851.07 | 0.976 |
| 512 | 1024 | distinct | none | 22619.03 | 23205.45 | 0.975 |
| 512 | 1024 | distinct | device | 17112.96 | 17719.34 | 0.966 |
| 512 | 1024 | distinct | count | 19344.57 | 19151.43 | 1.010 |
| 512 | 1024 | distinct | free | 18352.64 | 19088.60 | 0.961 |
| 512 | 1024 | distinct | output | 414.94 | 435.44 | 0.953 |
| 512 | 1024 | distinct | epoch | 289.60 | 298.32 | 0.971 |
| 512 | 1024 | distinct | alias | 22420.27 | 23544.36 | 0.952 |
| 4096 | 4098 | grouped | none | 183985.89 | 183378.55 | 1.003 |
| 4096 | 4098 | grouped | device | 150797.75 | 151645.31 | 0.994 |
| 4096 | 4098 | grouped | count | 156429.86 | 150079.00 | 1.042 |
| 4096 | 4098 | grouped | free | 138861.55 | 147554.00 | 0.941 |
| 4096 | 4098 | grouped | output | 3343.69 | 3378.61 | 0.990 |
| 4096 | 4098 | grouped | epoch | 3470.16 | 3385.38 | 1.025 |
| 4096 | 4098 | grouped | alias | 185527.78 | 192530.97 | 0.964 |
| 4096 | 4098 | distinct | none | 194204.72 | 196996.97 | 0.986 |
| 4096 | 4098 | distinct | device | 160203.66 | 159515.19 | 1.004 |
| 4096 | 4098 | distinct | count | 160227.61 | 143754.53 | 1.115 |
| 4096 | 4098 | distinct | free | 157630.98 | 158795.19 | 0.993 |
| 4096 | 4098 | distinct | output | 3531.12 | 3285.86 | 1.075 |
| 4096 | 4098 | distinct | epoch | 3098.28 | 3178.64 | 0.975 |
| 4096 | 4098 | distinct | alias | 181962.64 | 197173.69 | 0.923 |
| 8 | 65536 | grouped | none | 401.45 | 392.60 | 1.023 |
| 8 | 65536 | distinct | none | 396.13 | 418.45 | 0.947 |
