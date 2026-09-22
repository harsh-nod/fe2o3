# Qualified Production Sorting Results

Source: `dd7d445ff269ebf836490c61ca01de1b7f33c413`.

- Verus: two whole-root runs, each 59 verified and zero errors; 19 scoped executable controls rejected.
- CPU: 870 unit tests and 27 doctests passed; eight ignored tests in the normal run. Three benchmark/diagnostic tests were run explicitly.
- Format and all-target Clippy with warnings denied passed.
- No GPU, MI300X, HIP or HSA measurement was performed in this packet.

Production now invokes the shared, verified adaptive sort. This accepts a verification/performance
tradeoff, not the earlier no-regression gate or general performance parity. Ratios above 1 below
are measured overhead, including valid shuffled enrollment; they must not be concealed by wins elsewhere.

All ratios are candidate median time / baseline median time; lower is faster.
Each median uses seven alternating paired rounds. Raw nanosecond totals and iteration counts are retained.
Benchmark processes were pinned to logical CPU 31 on a shared host, without exclusivity,
sibling-core isolation or frequency control. Near-unity ratios do not establish significance.
Our release build and solver campaigns completed before these timing runs.

## Direct Sorting

The baseline is standard unstable sorting by slot; the candidate uses the shared production body.
Each invocation resets the fixture outside the timed interval and consumes the result.
Sizes cover insertion, ninther and partition dispatch boundaries. Shuffle uses one fixed seed.

| References | Ascending | Descending | Shuffled | Duplicates |
| --- | --- | --- | --- | --- |
| 8 | 1.133 | 0.975 | 1.002 | 0.928 |
| 16 | 1.318 | 1.051 | 0.973 | 1.000 |
| 17 | 0.993 | 0.250 | 1.060 | 1.080 |
| 19 | 0.951 | 0.216 | 0.941 | 0.960 |
| 20 | 1.001 | 0.216 | 1.000 | 1.081 |
| 21 | 1.121 | 0.955 | 0.893 | 0.863 |
| 64 | 1.040 | 0.960 | 0.847 | 0.667 |
| 127 | 1.077 | 0.935 | 0.806 | 0.516 |
| 128 | 0.815 | 0.904 | 0.774 | 0.536 |
| 129 | 0.843 | 0.922 | 0.651 | 0.601 |
| 255 | 1.147 | 0.946 | 0.798 | 0.567 |
| 256 | 1.282 | 0.897 | 0.742 | 0.523 |
| 257 | 1.126 | 0.988 | 0.785 | 0.585 |
| 512 | 0.861 | 0.829 | 0.785 | 0.690 |
| 4096 | 0.747 | 0.794 | 0.948 | 0.519 |

## Actual Production Enrollment

The candidate is the actual production method, not a copied candidate selector. The control is
the frozen batch body using the standard sort and the same current shared searches.
Both are release unit-test builds with test-only read instrumentation, not standalone uninstrumented latency.
Fixtures validate results, allocation contents, free stack and caller output before timing.
Allocation arenas start empty; occupied/replay workloads and multiple shuffle seeds are not timed here.
Duplicates and prefix aliases are rejected states, not successful enrollment throughput.
Other validation work can dilute sorting regressions; this table does not override direct measurements.

| Batch | Ascending | Descending | Shuffled | Duplicate Slot | Prefix Alias |
| --- | --- | --- | --- | --- | --- |
| 8 | 0.956 | 1.044 | 1.038 | 0.844 | 0.946 |
| 16 | 1.022 | 0.860 | 0.855 | 1.232 | 1.206 |
| 17 | 1.039 | 1.042 | 0.969 | 1.277 | 0.993 |
| 19 | 1.076 | 0.783 | 0.989 | 1.066 | 1.043 |
| 20 | 1.058 | 0.811 | 0.909 | 0.867 | 1.040 |
| 21 | 0.698 | 1.025 | 0.909 | 0.917 | 0.713 |
| 64 | 1.113 | 1.072 | 0.907 | 1.132 | 1.009 |
| 127 | 1.017 | 0.914 | 1.073 | 1.005 | 0.961 |
| 128 | 0.940 | 1.076 | 0.923 | 1.003 | 1.250 |
| 129 | 1.045 | 0.909 | 1.016 | 1.006 | 0.977 |
| 255 | 1.111 | 1.086 | 0.976 | 0.900 | 0.987 |
| 256 | 0.964 | 0.964 | 1.092 | 0.958 | 0.927 |
| 257 | 1.035 | 0.945 | 0.870 | 1.100 | 1.087 |
| 512 | 0.976 | 0.870 | 0.973 | 1.021 | 0.956 |
| 4096 | 1.102 | 0.968 | 1.171 | 0.967 | 0.921 |

## Isolated Cyclic Partition

Baseline: the frozen simple unconditional-swap partition from 5fa588909. Both controls use
the same fixed shuffled fixture and pivot. This isolates partition work, not complete sorting.
The cyclic body uses direct slice-to-slice record copies; timing alone does not establish
a particular compiler lowering or a machine-code performance theorem.

| References | Cyclic / Simple |
| --- | --- |
| 256 | 0.859 |
| 4096 | 0.750 |
| 65536 | 0.745 |

## Untimed Work Diagnostic

These counters come from a copied recursive diagnostic, not the production entry point,
an instruction counter or a complexity proof. Cyclic partition changes descendant order,
so these cases also test the resulting endpoint/ninther choices across three fixed seeds.

| References | Seed | Cyclic | Ninther | Partitioned Volume | Calls | Heap Fallbacks | Max Depth |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 4096 | 1 | false | false | 38967 | 332 | 0 | 14 |
| 4096 | 1 | false | true | 38283 | 348 | 0 | 14 |
| 4096 | 1 | true | false | 41238 | 357 | 0 | 17 |
| 4096 | 1 | true | true | 37781 | 336 | 0 | 14 |
| 4096 | 3735928559 | false | false | 40109 | 338 | 0 | 15 |
| 4096 | 3735928559 | false | true | 38727 | 359 | 0 | 15 |
| 4096 | 3735928559 | true | false | 43176 | 349 | 0 | 18 |
| 4096 | 3735928559 | true | true | 39844 | 353 | 0 | 16 |
| 4096 | 11400714819323198485 | false | false | 40125 | 351 | 0 | 17 |
| 4096 | 11400714819323198485 | false | true | 37915 | 352 | 0 | 15 |
| 4096 | 11400714819323198485 | true | false | 40635 | 358 | 0 | 14 |
| 4096 | 11400714819323198485 | true | true | 38695 | 341 | 0 | 14 |
| 65536 | 1 | false | false | 1010084 | 5528 | 0 | 25 |
| 65536 | 1 | false | true | 898320 | 5525 | 0 | 21 |
| 65536 | 1 | true | false | 1006478 | 5575 | 0 | 25 |
| 65536 | 1 | true | true | 884047 | 5506 | 0 | 21 |
| 65536 | 3735928559 | false | false | 979707 | 5509 | 0 | 24 |
| 65536 | 3735928559 | false | true | 908106 | 5573 | 0 | 24 |
| 65536 | 3735928559 | true | false | 974606 | 5590 | 0 | 25 |
| 65536 | 3735928559 | true | true | 892970 | 5530 | 0 | 20 |
| 65536 | 11400714819323198485 | false | false | 973236 | 5522 | 0 | 23 |
| 65536 | 11400714819323198485 | false | true | 878918 | 5453 | 0 | 23 |
| 65536 | 11400714819323198485 | true | false | 1026168 | 5557 | 0 | 28 |
| 65536 | 11400714819323198485 | true | true | 901684 | 5522 | 0 | 22 |

Gate 1 still requires actual-type construction and full enrollment header/rollback/refill/commit
composition, caller-precondition derivation, Begin, reader admission/release, public-wrapper
correspondence and physical storage/unwind contracts. Native pending-consumer admission remains closed.
