# Qualified Ordering Results

Source: `cdf474b3b7bf7334a1e60b83c066016326be478a`.

- Verus: two whole-root runs, each 32 verified and zero errors; nine scoped executable controls rejected.
- CPU: 866 unit tests and 27 doctests passed; five ignored tests in the normal run. The three new benchmark tests were run explicitly.
- Format and all-target Clippy with warnings denied passed.
- No GPU, MI300X, HIP or HSA measurement was performed in this packet.

All ratios below are candidate median time / baseline median time; lower is faster.
Each median uses seven paired rounds. Raw nanosecond totals and iteration counts are retained.
The host was shared, without CPU affinity or frequency isolation. These measurements do not establish
a universal speedup or a statistically significant difference for near-unity ratios.

## Test-Only Sort Candidate

Production retains the standard sort. The proved heap candidate is not shipped.

| References | Ascending | Descending | Shuffled | Duplicates |
| --- | --- | --- | --- | --- |
| 8 | 0.947 | 1.442 | 2.327 | 2.746 |
| 64 | 0.973 | 9.964 | 1.825 | 1.576 |
| 512 | 1.186 | 20.205 | 2.180 | 3.414 |
| 4096 | 1.336 | 53.435 | 4.981 | 9.337 |

## Production Searches

The baseline uses standard binary search; the candidate is the shared, proved lower-bound search.

| Entries | Key Repeated | Key Mixed | Slot Repeated | Slot Mixed |
| --- | --- | --- | --- | --- |
| 0 | 0.770 | 0.849 | 0.796 | 0.962 |
| 1 | 1.622 | 1.418 | 1.313 | 1.625 |
| 2 | 1.370 | 1.281 | 1.901 | 1.432 |
| 8 | 0.831 | 1.055 | 1.347 | 1.101 |
| 64 | 0.633 | 0.747 | 0.903 | 0.956 |
| 512 | 0.526 | 0.584 | 0.833 | 0.834 |
| 4096 | 0.505 | 0.536 | 0.524 | 0.583 |

## Complete Enrollment

Both versions use standard sorting. The baseline is the frozen original batch body.
The test validates results, allocation contents, the free stack and caller output before timing.

| Batch | Fresh | Half Occupied | Early Replay | Late Replay | Capacity | Prefix Alias |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | 0.657 | 0.666 | 1.035 | 1.190 | 1.087 | 1.294 |
| 64 | 0.628 | 0.548 | 0.942 | 1.154 | 1.092 | 1.189 |
| 512 | 0.770 | 0.536 | 1.009 | 1.060 | 1.131 | 1.253 |

The remaining architectural gate is still actual-type lifecycle composition: enrollment
header/rollback/commit and production sorting, Begin, reader admission/release, public-wrapper
correspondence and physical storage/unwind contracts. Native pending-consumer admission remains closed.
