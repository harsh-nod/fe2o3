# Qualified Adaptive Sorting Results

Source: `5fa5889098d90c335c7c4ba179be9821f40cadd8`.

- Verus: two whole-root runs, each 57 verified and zero errors; 15 scoped executable controls rejected.
- CPU: 870 unit tests and 27 doctests passed; seven ignored tests in the normal run. Both new benchmark tests were run explicitly.
- Format and all-target Clippy with warnings denied passed.
- No GPU, MI300X, HIP or HSA measurement was performed in this packet.

Production retains the standard sort. The adaptive candidate is test-only, not a production replacement.

All ratios are candidate median time / baseline median time; lower is faster.
Each median uses seven alternating paired rounds. Raw nanosecond totals and iteration counts are retained.
The host was shared, without CPU affinity or frequency isolation. Near-unity ratios do not establish
statistically significant differences, and these fixtures do not establish universal performance parity.

## Direct Sorting

The baseline is standard unstable sorting by slot; the candidate uses the shared, proved adaptive body.
Each invocation resets the fixture outside the timed interval and consumes the result.
Sizes include both the insertion and partition dispatch boundaries.

| References | Ascending | Descending | Shuffled | Duplicates |
| --- | --- | --- | --- | --- |
| 8 | 1.056 | 0.886 | 0.986 | 0.978 |
| 16 | 1.021 | 0.933 | 1.164 | 1.134 |
| 17 | 0.883 | 0.280 | 0.906 | 1.497 |
| 64 | 0.696 | 1.172 | 0.836 | 0.627 |
| 255 | 0.713 | 1.223 | 0.853 | 0.595 |
| 256 | 0.528 | 0.997 | 0.826 | 0.566 |
| 257 | 0.646 | 1.100 | 0.796 | 0.598 |
| 512 | 0.581 | 1.406 | 0.934 | 0.665 |
| 4096 | 0.631 | 1.455 | 1.213 | 0.761 |

## Complete Enrollment Sort Selection

Both controls use the same frozen batch body and current shared searches; only sorting differs.
This measures the copied selector body, not a promoted production entry point.
Fixtures validate results, allocation contents, free stack and caller output against production.
Duplicates and prefix aliases are rejected states, not successful enrollment throughput.
Other enrollment validation work can dilute sorting regressions; this table does not override direct measurements.

| Batch | Ascending | Descending | Shuffled | Duplicate Slot | Prefix Alias |
| --- | --- | --- | --- | --- | --- |
| 8 | 1.022 | 1.027 | 0.860 | 0.977 | 1.053 |
| 16 | 1.029 | 0.894 | 0.947 | 0.926 | 0.995 |
| 17 | 0.845 | 0.925 | 0.982 | 1.090 | 0.998 |
| 64 | 0.963 | 1.083 | 0.955 | 1.040 | 0.932 |
| 255 | 0.996 | 0.922 | 1.064 | 0.976 | 0.958 |
| 256 | 1.038 | 1.143 | 0.915 | 1.220 | 0.962 |
| 257 | 1.016 | 1.129 | 1.051 | 0.951 | 0.972 |
| 512 | 1.007 | 1.012 | 0.929 | 0.947 | 1.152 |
| 4096 | 0.961 | 1.106 | 1.159 | 0.942 | 1.055 |

The remaining architectural gate is actual-type lifecycle composition: construction, full enrollment
header/rollback/commit and production sorting, Begin, reader admission/release, public-wrapper
correspondence and physical storage/unwind contracts. Native pending-consumer admission remains closed.
