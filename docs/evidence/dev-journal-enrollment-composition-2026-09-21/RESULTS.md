# Qualified Actual-Type Enrollment Composition

Source: `2aefa408d79991ec4840871ae761930690756d57`.

- Verus: two whole-root runs, each 90 verified and zero errors; 28 scoped executable controls rejected.
- CPU: 874 unit tests and 27 doctests passed; nine ignored tests in the normal run.
- Both complete-enrollment release benchmarks were run explicitly.
- Format and all-target Clippy with warnings denied passed.
- No GPU, MI300X, HIP or HSA measurement was performed in this packet.

The actual-type shared batch function has no admission precondition. It proves the exact
header/replay/capacity/corruption decision, all-or-nothing rejection, canonical reverse-stack
output, exact initialized allocation contents and free-stack truncation, and unchanged other fields.
All sorted-key, all-Some, vacancy and selected-slot uniqueness premises are derived by callers.

These guarantees cover normal execution in the actual-type Verus model, not physical Vec
addresses/capacity, allocation failure or unwind. The public one-line forwarding wrapper
is source-bound and reviewed, not itself compiled into this root. Test counters are excluded.
Historical enrollment projection, reader/producer custody and status-preservation transport
still require explicit correspondence lemmas. Native pending-consumer admission remains closed.

## Measurement Scope

All ratios are current production median time / frozen baseline median time; lower is faster.
Values above 1 are measured overhead, not a passed no-regression or general performance-parity gate.
Each median uses seven alternating paired rounds; raw nanosecond totals and counts are retained.
Fixtures are reset outside timing and assert exact decisions, output, allocation contents and free stack.
Both paths use release unit-test builds with read instrumentation, not uninstrumented library latency.
Processes were pinned to logical CPU 31 on a shared host, with no exclusivity, sibling-core isolation
or frequency control. Owned solver and build work finished before timing. Near-unity differences
do not establish significance. Shuffle uses one fixed seed, and workload coverage remains scoped.

## Standard-Sort Baseline

Control: frozen iterator-based batch orchestration with the standard sort and current shared searches.
Candidate: actual production method with shared indexed orchestration and the verified adaptive sort.
Thus this comparison includes both orchestration and sort changes.

| Batch | Ascending | Descending | Shuffled | Duplicate Slot | Prefix Alias |
| --- | --- | --- | --- | --- | --- |
| 8 | 1.075 | 0.966 | 1.124 | 0.882 | 0.768 |
| 16 | 1.019 | 0.992 | 0.907 | 0.845 | 0.985 |
| 17 | 0.986 | 1.005 | 0.975 | 0.685 | 0.860 |
| 19 | 1.074 | 0.940 | 1.072 | 0.831 | 0.760 |
| 20 | 0.975 | 0.912 | 1.124 | 0.954 | 0.821 |
| 21 | 1.030 | 1.106 | 1.050 | 0.917 | 0.771 |
| 64 | 1.020 | 0.906 | 1.044 | 0.988 | 0.856 |
| 127 | 0.844 | 1.125 | 0.911 | 0.840 | 1.118 |
| 128 | 0.941 | 0.952 | 0.973 | 0.863 | 0.972 |
| 129 | 0.673 | 0.999 | 0.950 | 0.816 | 0.883 |
| 255 | 1.254 | 1.149 | 0.926 | 0.985 | 1.177 |
| 256 | 0.923 | 1.143 | 0.868 | 0.993 | 0.943 |
| 257 | 0.841 | 0.905 | 0.881 | 0.777 | 0.841 |
| 512 | 1.086 | 0.952 | 0.971 | 0.954 | 0.946 |
| 4096 | 1.060 | 0.987 | 1.180 | 0.952 | 0.971 |

## Matched Adaptive-Sort Baseline

Both paths use the same current shared adaptive sort and binary searches. The frozen baseline
retains the prior iterator-based orchestration; the candidate invokes the actual new production
method. This isolates orchestration and its resulting compiler choices, not GPU dispatch.

| Batch | Ascending | Descending | Shuffled | Duplicate Slot | Prefix Alias |
| --- | --- | --- | --- | --- | --- |
| 8 | 1.101 | 0.914 | 0.976 | 1.051 | 1.001 |
| 16 | 1.017 | 1.000 | 0.810 | 0.861 | 0.813 |
| 17 | 0.951 | 0.918 | 0.976 | 0.938 | 0.939 |
| 19 | 0.929 | 1.088 | 1.246 | 0.853 | 1.152 |
| 20 | 1.034 | 0.988 | 0.952 | 0.904 | 0.985 |
| 21 | 1.078 | 1.029 | 1.315 | 0.909 | 0.896 |
| 64 | 0.960 | 1.067 | 1.075 | 0.915 | 0.874 |
| 127 | 0.968 | 0.957 | 0.944 | 1.021 | 0.998 |
| 128 | 1.008 | 0.943 | 0.977 | 0.938 | 0.934 |
| 129 | 1.113 | 1.016 | 0.963 | 0.922 | 1.032 |
| 255 | 1.100 | 0.988 | 0.906 | 1.073 | 0.925 |
| 256 | 0.974 | 0.953 | 0.876 | 0.999 | 0.988 |
| 257 | 0.836 | 0.931 | 0.972 | 1.115 | 1.020 |
| 512 | 1.224 | 1.069 | 0.895 | 0.992 | 1.217 |
| 4096 | 0.888 | 1.112 | 1.049 | 0.992 | 1.094 |

## Occupied and Rejection Paths

The half-occupied fixture succeeds with unrelated occupied keys and a matching free partition.
Replay-first, replay-last and capacity fixtures are deliberate raw rejection paths, not valid
saturated-journal throughput. Duplicate slots and prefix aliases above are also rejection paths.
Early replay refers to arena position: the complete canonical header is still validated first.

| Batch | Half Occupied | Replay First | Replay Last | Capacity |
| --- | --- | --- | --- | --- |
| 8 | 0.877 | 0.873 | 0.935 | 0.807 |
| 16 | 0.951 | 1.171 | 0.888 | 0.804 |
| 17 | 0.994 | 1.093 | 0.866 | 0.844 |
| 19 | 0.994 | 1.042 | 1.020 | 0.963 |
| 20 | 1.005 | 1.126 | 0.943 | 0.831 |
| 21 | 0.964 | 1.178 | 0.859 | 0.917 |
| 64 | 1.026 | 1.070 | 0.779 | 0.861 |
| 127 | 0.956 | 1.040 | 0.764 | 0.891 |
| 128 | 1.049 | 1.229 | 0.820 | 0.842 |
| 129 | 1.032 | 1.175 | 0.924 | 0.812 |
| 255 | 1.012 | 1.016 | 1.121 | 0.951 |
| 256 | 0.962 | 1.147 | 1.119 | 0.903 |
| 257 | 1.190 | 1.029 | 0.980 | 0.950 |
| 512 | 0.950 | 1.214 | 1.274 | 0.922 |
| 4096 | 0.825 | 1.176 | 0.994 | 1.041 |

Gate 1 advances actual-type raw batch enrollment, including its caller-precondition derivation.
Construction, historical custody projection, Begin, reader admission/release, public-wrapper
correspondence and physical storage/unwind remain open. This is not full HIP/HSA parity.
