# Qualified Actual Begin Guards

Source: 343cc10f108c1c64d302d27f3d21b884ea28911b.

- Verus: two whole-root runs, each 444 verified and zero errors at default solver limits.
- Twenty-two controls: thirteen executable omissions/mutations, three contract sensitivities, four projections and two live-fixture sensitivities.
- CPU: 887 unit tests and 27 doctests passed; twelve tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- Two selected release benchmarks passed, producing 2,464 rows over 176 owner/scope/case combinations.
- No native GPU execution or HIP/HSA measurements were run.

Actual allocation lookup, stable/combined counts, caller-ordered unread scans and both Begin
wrappers now execute shared production bodies over the actual private owner declarations.
The paired harness independently executes actual and historical issued Begin. It requires only
represented pre-state, mapped inputs and the existing issued invariant. That invariant derives
index/addition safety; successful admission or desired post-state are not assumed.

Results preserve exact error precedence and rejection identity, nested raw Begin transitions,
and complete stable/producer owner frames. The old historical-guard-gated hybrid harness remains
as earlier evidence, but is not used to establish this new actual wrapper correspondence.

The synthetic live witness retains a Pending producer reservation/count while Begin updates
a distinct allocation, then checks raw replay and guard-first rejection. It does not establish
actual constructor/reader-acquisition reachability or exercise a live stable lease in Verus.
CPU fixtures independently cover stable and producer protection, full state, storage identities,
restoration, caller order and lookup errors before missing count storage.

The 444 obligations include inherited proofs. Production adapters, declarations and unchanged
raw Begin dependencies are source-bound. Paired execution and synthetic fixtures are proof-only.
Actual constructor/acquisition/release refinement, physical Vec capacity/address and allocator/
unwind semantics, universal represented-model existence and native producer authority remain open.
Native pending-consumer admission stays closed. Full HIP/HSA parity remains unproved.

## Instrumented CPU Comparison

Ratios are shared/frozen medians over seven alternating rounds. Lower is faster.
These are non-exclusive CPU regression observations with test-only indexed-access counters,
per-call timer and dispatch overhead inside the measurement; restoration is outside it.
They are not uninstrumented production latency, a GPU comparison, or an orders-of-magnitude claim.
The complete case roster is retained below, including regressions and short/error paths.

| Owner | Scope | Roster | Capacity | Pattern | Shared ns | Frozen ns | Shared/frozen |
| --- | --- | ---: | ---: | --- | ---: | ---: | ---: |
| producer | guard | 0 | 1024 | normal | 62.21 | 65.98 | 0.943 |
| producer | begin | 0 | 1024 | normal | 88.98 | 85.78 | 1.037 |
| producer | guard | 0 | 1024 | invalid_writer | 68.93 | 69.10 | 0.998 |
| producer | begin | 0 | 1024 | invalid_writer | 86.42 | 87.03 | 0.993 |
| producer | guard | 1 | 1024 | normal | 75.55 | 75.90 | 0.995 |
| producer | begin | 1 | 1024 | normal | 134.82 | 130.35 | 1.034 |
| producer | guard | 1 | 1024 | invalid_first | 82.32 | 75.66 | 1.088 |
| producer | begin | 1 | 1024 | invalid_first | 73.14 | 97.54 | 0.750 |
| producer | guard | 1 | 1024 | invalid_last | 73.88 | 62.11 | 1.189 |
| producer | begin | 1 | 1024 | invalid_last | 63.42 | 68.16 | 0.930 |
| producer | guard | 1 | 1024 | busy_first | 88.49 | 72.63 | 1.218 |
| producer | begin | 1 | 1024 | busy_first | 75.50 | 71.84 | 1.051 |
| producer | guard | 1 | 1024 | busy_last | 74.52 | 74.93 | 0.994 |
| producer | begin | 1 | 1024 | busy_last | 76.92 | 69.39 | 1.108 |
| producer | guard | 1 | 1024 | pending_last | 90.85 | 86.18 | 1.054 |
| producer | begin | 1 | 1024 | pending_last | 117.12 | 115.61 | 1.013 |
| producer | guard | 1 | 1024 | unrelated | 78.87 | 76.16 | 1.036 |
| producer | begin | 1 | 1024 | unrelated | 123.64 | 121.06 | 1.021 |
| producer | guard | 1 | 1024 | invalid_writer | 69.87 | 88.36 | 0.791 |
| producer | begin | 1 | 1024 | invalid_writer | 110.28 | 83.05 | 1.328 |
| producer | guard | 8 | 1024 | normal | 146.39 | 143.10 | 1.023 |
| producer | begin | 8 | 1024 | normal | 488.70 | 438.39 | 1.115 |
| producer | guard | 8 | 1024 | invalid_first | 61.32 | 65.97 | 0.929 |
| producer | begin | 8 | 1024 | invalid_first | 54.19 | 52.10 | 1.040 |
| producer | guard | 8 | 1024 | invalid_last | 83.15 | 81.53 | 1.020 |
| producer | begin | 8 | 1024 | invalid_last | 88.07 | 81.12 | 1.086 |
| producer | guard | 8 | 1024 | busy_first | 52.12 | 45.43 | 1.147 |
| producer | begin | 8 | 1024 | busy_first | 47.39 | 51.49 | 0.920 |
| producer | guard | 8 | 1024 | busy_last | 80.40 | 90.30 | 0.890 |
| producer | begin | 8 | 1024 | busy_last | 75.78 | 95.81 | 0.791 |
| producer | guard | 8 | 1024 | pending_last | 84.74 | 93.59 | 0.905 |
| producer | begin | 8 | 1024 | pending_last | 230.34 | 237.57 | 0.970 |
| producer | guard | 8 | 1024 | unrelated | 81.86 | 83.72 | 0.978 |
| producer | begin | 8 | 1024 | unrelated | 251.36 | 258.20 | 0.974 |
| producer | guard | 8 | 1024 | invalid_writer | 93.87 | 87.26 | 1.076 |
| producer | begin | 8 | 1024 | invalid_writer | 156.43 | 161.31 | 0.970 |
| producer | guard | 64 | 1024 | normal | 666.57 | 653.53 | 1.020 |
| producer | begin | 64 | 1024 | normal | 2698.79 | 2853.06 | 0.946 |
| producer | guard | 64 | 1024 | invalid_first | 64.53 | 74.72 | 0.864 |
| producer | begin | 64 | 1024 | invalid_first | 74.46 | 67.77 | 1.099 |
| producer | guard | 64 | 1024 | invalid_last | 642.47 | 645.89 | 0.995 |
| producer | begin | 64 | 1024 | invalid_last | 616.03 | 632.42 | 0.974 |
| producer | guard | 64 | 1024 | busy_first | 88.03 | 80.07 | 1.099 |
| producer | begin | 64 | 1024 | busy_first | 105.47 | 118.05 | 0.893 |
| producer | guard | 64 | 1024 | busy_last | 881.47 | 873.90 | 1.009 |
| producer | begin | 64 | 1024 | busy_last | 822.90 | 867.74 | 0.948 |
| producer | guard | 64 | 1024 | pending_last | 866.74 | 1020.12 | 0.850 |
| producer | begin | 64 | 1024 | pending_last | 2114.62 | 2346.70 | 0.901 |
| producer | guard | 64 | 1024 | unrelated | 797.83 | 829.22 | 0.962 |
| producer | begin | 64 | 1024 | unrelated | 3258.05 | 3702.79 | 0.880 |
| producer | guard | 64 | 1024 | invalid_writer | 751.81 | 752.40 | 0.999 |
| producer | begin | 64 | 1024 | invalid_writer | 1599.90 | 1601.21 | 0.999 |
| producer | guard | 512 | 1024 | normal | 6045.89 | 6607.70 | 0.915 |
| producer | begin | 512 | 1024 | normal | 31937.88 | 39207.07 | 0.815 |
| producer | guard | 512 | 1024 | invalid_first | 84.97 | 85.56 | 0.993 |
| producer | begin | 512 | 1024 | invalid_first | 99.48 | 92.27 | 1.078 |
| producer | guard | 512 | 1024 | invalid_last | 7049.80 | 6192.36 | 1.138 |
| producer | begin | 512 | 1024 | invalid_last | 9425.83 | 6444.63 | 1.463 |
| producer | guard | 512 | 1024 | busy_first | 101.25 | 105.51 | 0.960 |
| producer | begin | 512 | 1024 | busy_first | 103.21 | 120.63 | 0.856 |
| producer | guard | 512 | 1024 | busy_last | 6689.16 | 6201.02 | 1.079 |
| producer | begin | 512 | 1024 | busy_last | 7491.28 | 6428.70 | 1.165 |
| producer | guard | 512 | 1024 | pending_last | 5741.66 | 5815.61 | 0.987 |
| producer | begin | 512 | 1024 | pending_last | 14691.28 | 18773.56 | 0.783 |
| producer | guard | 512 | 1024 | unrelated | 5679.72 | 7512.92 | 0.756 |
| producer | begin | 512 | 1024 | unrelated | 29644.07 | 33362.86 | 0.889 |
| producer | guard | 512 | 1024 | invalid_writer | 5864.33 | 5696.12 | 1.030 |
| producer | begin | 512 | 1024 | invalid_writer | 16511.82 | 15093.45 | 1.094 |
| producer | guard | 4096 | 8192 | normal | 72046.77 | 91467.16 | 0.788 |
| producer | begin | 4096 | 8192 | normal | 315087.30 | 299833.53 | 1.051 |
| producer | guard | 4096 | 8192 | invalid_first | 221.68 | 192.65 | 1.151 |
| producer | begin | 4096 | 8192 | invalid_first | 243.79 | 211.39 | 1.153 |
| producer | guard | 4096 | 8192 | invalid_last | 61968.87 | 57000.12 | 1.087 |
| producer | begin | 4096 | 8192 | invalid_last | 56729.47 | 48570.16 | 1.168 |
| producer | guard | 4096 | 8192 | busy_first | 228.93 | 321.72 | 0.712 |
| producer | begin | 4096 | 8192 | busy_first | 308.72 | 361.13 | 0.855 |
| producer | guard | 4096 | 8192 | busy_last | 62235.63 | 59146.42 | 1.052 |
| producer | begin | 4096 | 8192 | busy_last | 57174.59 | 61615.89 | 0.928 |
| producer | guard | 4096 | 8192 | pending_last | 50419.47 | 56758.38 | 0.888 |
| producer | begin | 4096 | 8192 | pending_last | 124710.73 | 140992.00 | 0.885 |
| producer | guard | 4096 | 8192 | unrelated | 45221.96 | 44155.62 | 1.024 |
| producer | begin | 4096 | 8192 | unrelated | 199743.95 | 183268.44 | 1.090 |
| producer | guard | 4096 | 8192 | invalid_writer | 36595.54 | 38920.90 | 0.940 |
| producer | begin | 4096 | 8192 | invalid_writer | 98417.62 | 81739.51 | 1.204 |
| producer | guard | 8 | 65536 | normal | 139.71 | 157.42 | 0.888 |
| producer | begin | 8 | 65536 | normal | 456.00 | 505.53 | 0.902 |
| producer | guard | 8 | 65536 | unrelated | 144.59 | 167.95 | 0.861 |
| producer | begin | 8 | 65536 | unrelated | 576.74 | 662.58 | 0.870 |
| stable | guard | 0 | 1024 | normal | 114.96 | 62.50 | 1.839 |
| stable | begin | 0 | 1024 | normal | 78.39 | 96.01 | 0.816 |
| stable | guard | 0 | 1024 | invalid_writer | 66.90 | 70.74 | 0.946 |
| stable | begin | 0 | 1024 | invalid_writer | 79.45 | 126.83 | 0.626 |
| stable | guard | 1 | 1024 | normal | 80.28 | 72.94 | 1.101 |
| stable | begin | 1 | 1024 | normal | 131.98 | 167.62 | 0.787 |
| stable | guard | 1 | 1024 | invalid_first | 80.24 | 75.07 | 1.069 |
| stable | begin | 1 | 1024 | invalid_first | 66.74 | 81.43 | 0.820 |
| stable | guard | 1 | 1024 | invalid_last | 67.49 | 73.24 | 0.922 |
| stable | begin | 1 | 1024 | invalid_last | 70.00 | 89.81 | 0.779 |
| stable | guard | 1 | 1024 | busy_first | 78.25 | 81.34 | 0.962 |
| stable | begin | 1 | 1024 | busy_first | 72.32 | 87.94 | 0.822 |
| stable | guard | 1 | 1024 | busy_last | 96.53 | 84.65 | 1.140 |
| stable | begin | 1 | 1024 | busy_last | 70.21 | 79.56 | 0.882 |
| stable | guard | 1 | 1024 | pending_last | 118.51 | 90.04 | 1.316 |
| stable | begin | 1 | 1024 | pending_last | 117.45 | 101.28 | 1.160 |
| stable | guard | 1 | 1024 | unrelated | 80.69 | 70.95 | 1.137 |
| stable | begin | 1 | 1024 | unrelated | 107.69 | 104.16 | 1.034 |
| stable | guard | 1 | 1024 | invalid_writer | 83.34 | 78.04 | 1.068 |
| stable | begin | 1 | 1024 | invalid_writer | 99.49 | 90.63 | 1.098 |
| stable | guard | 8 | 1024 | normal | 112.09 | 108.89 | 1.029 |
| stable | begin | 8 | 1024 | normal | 296.38 | 338.26 | 0.876 |
| stable | guard | 8 | 1024 | invalid_first | 73.53 | 81.65 | 0.900 |
| stable | begin | 8 | 1024 | invalid_first | 77.09 | 69.59 | 1.108 |
| stable | guard | 8 | 1024 | invalid_last | 120.14 | 123.67 | 0.971 |
| stable | begin | 8 | 1024 | invalid_last | 107.28 | 112.50 | 0.954 |
| stable | guard | 8 | 1024 | busy_first | 90.22 | 78.26 | 1.153 |
| stable | begin | 8 | 1024 | busy_first | 80.35 | 77.79 | 1.033 |
| stable | guard | 8 | 1024 | busy_last | 155.90 | 152.74 | 1.021 |
| stable | begin | 8 | 1024 | busy_last | 157.00 | 137.20 | 1.144 |
| stable | guard | 8 | 1024 | pending_last | 136.31 | 151.37 | 0.901 |
| stable | begin | 8 | 1024 | pending_last | 209.83 | 235.51 | 0.891 |
| stable | guard | 8 | 1024 | unrelated | 134.02 | 133.80 | 1.002 |
| stable | begin | 8 | 1024 | unrelated | 319.15 | 324.03 | 0.985 |
| stable | guard | 8 | 1024 | invalid_writer | 118.81 | 125.95 | 0.943 |
| stable | begin | 8 | 1024 | invalid_writer | 142.13 | 141.25 | 1.006 |
| stable | guard | 64 | 1024 | normal | 615.10 | 766.57 | 0.802 |
| stable | begin | 64 | 1024 | normal | 2167.78 | 1776.23 | 1.220 |
| stable | guard | 64 | 1024 | invalid_first | 98.85 | 80.10 | 1.234 |
| stable | begin | 64 | 1024 | invalid_first | 85.54 | 82.48 | 1.037 |
| stable | guard | 64 | 1024 | invalid_last | 640.97 | 715.66 | 0.896 |
| stable | begin | 64 | 1024 | invalid_last | 626.70 | 617.56 | 1.015 |
| stable | guard | 64 | 1024 | busy_first | 75.26 | 83.89 | 0.897 |
| stable | begin | 64 | 1024 | busy_first | 77.70 | 75.41 | 1.030 |
| stable | guard | 64 | 1024 | busy_last | 416.67 | 455.62 | 0.915 |
| stable | begin | 64 | 1024 | busy_last | 520.38 | 521.75 | 0.997 |
| stable | guard | 64 | 1024 | pending_last | 457.43 | 382.60 | 1.196 |
| stable | begin | 64 | 1024 | pending_last | 788.10 | 673.85 | 1.170 |
| stable | guard | 64 | 1024 | unrelated | 507.03 | 477.17 | 1.063 |
| stable | begin | 64 | 1024 | unrelated | 2073.51 | 2527.63 | 0.820 |
| stable | guard | 64 | 1024 | invalid_writer | 707.25 | 734.83 | 0.962 |
| stable | begin | 64 | 1024 | invalid_writer | 721.46 | 675.38 | 1.068 |
| stable | guard | 512 | 1024 | normal | 3300.61 | 3552.31 | 0.929 |
| stable | begin | 512 | 1024 | normal | 16117.59 | 20425.46 | 0.789 |
| stable | guard | 512 | 1024 | invalid_first | 93.97 | 76.76 | 1.224 |
| stable | begin | 512 | 1024 | invalid_first | 76.66 | 75.21 | 1.019 |
| stable | guard | 512 | 1024 | invalid_last | 4985.03 | 4838.51 | 1.030 |
| stable | begin | 512 | 1024 | invalid_last | 4598.18 | 5209.69 | 0.883 |
| stable | guard | 512 | 1024 | busy_first | 84.36 | 85.51 | 0.987 |
| stable | begin | 512 | 1024 | busy_first | 85.94 | 83.23 | 1.033 |
| stable | guard | 512 | 1024 | busy_last | 5272.64 | 5857.32 | 0.900 |
| stable | begin | 512 | 1024 | busy_last | 4434.08 | 5061.07 | 0.876 |
| stable | guard | 512 | 1024 | pending_last | 4666.45 | 5806.15 | 0.804 |
| stable | begin | 512 | 1024 | pending_last | 8217.43 | 9017.30 | 0.911 |
| stable | guard | 512 | 1024 | unrelated | 4126.09 | 4397.50 | 0.938 |
| stable | begin | 512 | 1024 | unrelated | 17625.94 | 19657.43 | 0.897 |
| stable | guard | 512 | 1024 | invalid_writer | 6383.04 | 6234.88 | 1.024 |
| stable | begin | 512 | 1024 | invalid_writer | 5615.22 | 5851.45 | 0.960 |
| stable | guard | 4096 | 8192 | normal | 41979.50 | 42101.07 | 0.997 |
| stable | begin | 4096 | 8192 | normal | 149517.83 | 186020.69 | 0.804 |
| stable | guard | 4096 | 8192 | invalid_first | 204.35 | 238.79 | 0.856 |
| stable | begin | 4096 | 8192 | invalid_first | 233.52 | 225.15 | 1.037 |
| stable | guard | 4096 | 8192 | invalid_last | 47342.17 | 44052.70 | 1.075 |
| stable | begin | 4096 | 8192 | invalid_last | 43406.54 | 40193.78 | 1.080 |
| stable | guard | 4096 | 8192 | busy_first | 278.80 | 219.81 | 1.268 |
| stable | begin | 4096 | 8192 | busy_first | 257.16 | 235.70 | 1.091 |
| stable | guard | 4096 | 8192 | busy_last | 45504.77 | 44614.78 | 1.020 |
| stable | begin | 4096 | 8192 | busy_last | 40609.36 | 36817.33 | 1.103 |
| stable | guard | 4096 | 8192 | pending_last | 40137.27 | 45523.57 | 0.882 |
| stable | begin | 4096 | 8192 | pending_last | 61537.58 | 63346.45 | 0.971 |
| stable | guard | 4096 | 8192 | unrelated | 39314.79 | 40900.67 | 0.961 |
| stable | begin | 4096 | 8192 | unrelated | 178531.84 | 167664.98 | 1.065 |
| stable | guard | 4096 | 8192 | invalid_writer | 46491.01 | 38017.87 | 1.223 |
| stable | begin | 4096 | 8192 | invalid_writer | 37108.91 | 34667.57 | 1.070 |
| stable | guard | 8 | 65536 | normal | 149.40 | 145.89 | 1.024 |
| stable | begin | 8 | 65536 | normal | 346.49 | 343.61 | 1.008 |
| stable | guard | 8 | 65536 | unrelated | 149.42 | 177.71 | 0.841 |
| stable | begin | 8 | 65536 | unrelated | 306.37 | 408.26 | 0.750 |
