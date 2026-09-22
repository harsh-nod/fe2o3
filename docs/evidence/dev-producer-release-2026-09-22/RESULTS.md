# Qualified Actual Producer-Read Release

Source: d645f152bb4af57404a957f5d77fb42b74894090.

- Verus: two whole-root runs, each 674 verified and zero errors at default solver limits.
- Twenty-four controls: nineteen executable mutations, one conditional-domain contract sensitivity, one projection and three witness sensitivities.
- CPU: 923 unit tests and 27 doctests passed; seventeen tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- One selected release benchmark passed, producing 1,120 rows over 80 cases.
- No native GPU execution or HIP/HSA measurements were run.

Production and proof share release ordering, header, per-reference preflight and
take/decrement/free-append commit, including the public adapter. Actual and historical
release execute independently under represented custody. Exact results and complete
owner state correspond; errors preserve the owner and successful release preserves
custody. Pending, Unknown, Success and NoEffect are all releasable; this is not a
claim that unresolved producer inputs may be consumed.

The raw execution domain requires count storage to cover allocation storage only
after the header succeeds. It assumes no desired result, valid free partition,
selected-slot uniqueness, producer epoch, combined budget or issuer-validity predicate.
Lookup identity and strict four-field order derive selected-slot uniqueness.
Coverage is sufficient, not the minimum bound along every reached execution path.
The existing malformed free prefix is preserved verbatim on successful release.
The entire stable owner and producer next-incarnation remain unchanged.

Four synthetic fixture/witness functions cover paired live acquisition followed by
late rejection, capacity shortage, successful subsets and stale replay; all four
raw statuses with producer ID zero, malformed free prefixes and epoch zero; and
header failures with missing count storage. A separate acquisition fixture exports
the concrete live state to keep solver work bounded. These are not proofs of
fallible Rust construction or actual Begin reachability.

CPU fixtures start from the actual lifecycle and retain a producer reservation and
a stable lease. They compare frozen results, full snapshots, input/storage identity
and exact journal accesses. Additional checks cover a mixed-status roster, equal
ranges with distinct incarnations, full-identity error precedence, equal logical
contents with unequal physical headroom, and early errors with missing counts.
The latter header-success malformed-storage fixtures exceed the conservative formal
domain and are concrete CPU checks only. A counted capacity expression is observed
zero times for early header errors and once for capacity rejection or header success.

The 674 obligations include inherited proofs. Twenty-three historical declarations
are projected exactly into one logical type universe. The frozen original method,
unchanged lookup helpers and reviewed public/private capacity adapters are authenticated.
The logical proof consumes an explicit capacity observation. Actual Vec capacity,
physical storage/nonallocation, allocator/unwind and quiescence authenticity remain
separate boundaries. Outer stable wrappers and construction/reachability remain open.
Native pending-consumer admission stays closed; Gate 1 and full HIP/HSA parity
remain unproved.

## Instrumented CPU Comparison

Ratios are shared/frozen medians over seven alternating rounds. Lower is faster.
These are non-exclusive CPU regression observations with test-only journal counters
and per-call timer/dispatch overhead. O(k) same-owner restoration and counter reset
are outside timing. Full post-state and storage are checked before resetting each
variant batch. Restoration does not reacquire reads from unresolved/resolved producers.
Successful release and late count errors use 3k accesses for Pending/Unknown and k
for Success/NoEffect; late stale references use one fewer item's weight. Header
errors use zero. Counters omit other reservation/vector operations.
Instrumentation may affect optimization. These are not production latencies, GPU
comparisons or an orders-of-magnitude claim. Regressions are retained.

| Roster | Capacity | Shape | State | Fault | Shared ns | Frozen ns | Shared/frozen |
| ---: | ---: | --- | --- | --- | ---: | ---: | ---: |
| 1 | 1024 | grouped | pending | none | 82.83 | 76.31 | 1.085 |
| 1 | 1024 | grouped | unknown | none | 78.28 | 78.93 | 0.992 |
| 1 | 1024 | grouped | success | none | 73.86 | 71.52 | 1.033 |
| 1 | 1024 | grouped | no_effect | none | 79.86 | 79.68 | 1.002 |
| 1 | 1024 | distinct | pending | none | 97.86 | 90.06 | 1.087 |
| 1 | 1024 | distinct | unknown | none | 96.82 | 88.22 | 1.097 |
| 1 | 1024 | distinct | success | none | 72.36 | 70.65 | 1.024 |
| 1 | 1024 | distinct | no_effect | none | 73.72 | 78.37 | 0.941 |
| 8 | 1024 | grouped | pending | none | 387.64 | 333.72 | 1.162 |
| 8 | 1024 | grouped | unknown | none | 325.84 | 328.75 | 0.991 |
| 8 | 1024 | grouped | success | none | 270.82 | 238.82 | 1.134 |
| 8 | 1024 | grouped | no_effect | none | 273.49 | 272.66 | 1.003 |
| 8 | 1024 | distinct | pending | none | 355.48 | 383.69 | 0.926 |
| 8 | 1024 | distinct | unknown | none | 395.33 | 400.22 | 0.988 |
| 8 | 1024 | distinct | success | none | 277.96 | 237.40 | 1.171 |
| 8 | 1024 | distinct | no_effect | none | 331.02 | 280.51 | 1.180 |
| 64 | 1024 | grouped | pending | none | 2440.77 | 2777.62 | 0.879 |
| 64 | 1024 | grouped | unknown | none | 2746.87 | 2371.00 | 1.159 |
| 64 | 1024 | grouped | success | none | 1530.93 | 1711.69 | 0.894 |
| 64 | 1024 | grouped | no_effect | none | 1785.87 | 1759.92 | 1.015 |
| 64 | 1024 | distinct | pending | none | 2960.03 | 3149.44 | 0.940 |
| 64 | 1024 | distinct | unknown | none | 2428.96 | 2685.11 | 0.905 |
| 64 | 1024 | distinct | success | none | 1454.60 | 1492.65 | 0.975 |
| 64 | 1024 | distinct | no_effect | none | 1544.05 | 1616.21 | 0.955 |
| 512 | 1024 | grouped | pending | none | 18570.35 | 18672.26 | 0.995 |
| 512 | 1024 | grouped | unknown | none | 20870.00 | 21350.44 | 0.977 |
| 512 | 1024 | grouped | success | none | 12286.53 | 13705.32 | 0.896 |
| 512 | 1024 | grouped | no_effect | none | 13419.80 | 13783.44 | 0.974 |
| 512 | 1024 | distinct | pending | none | 21284.29 | 24178.30 | 0.880 |
| 512 | 1024 | distinct | unknown | none | 18421.22 | 19855.96 | 0.928 |
| 512 | 1024 | distinct | success | none | 14270.52 | 12984.15 | 1.099 |
| 512 | 1024 | distinct | no_effect | none | 14245.49 | 13715.07 | 1.039 |
| 4096 | 4098 | grouped | pending | none | 179134.17 | 191504.89 | 0.935 |
| 4096 | 4098 | grouped | unknown | none | 168879.03 | 182085.30 | 0.927 |
| 4096 | 4098 | grouped | success | none | 123717.47 | 120381.08 | 1.028 |
| 4096 | 4098 | grouped | no_effect | none | 102684.06 | 128822.22 | 0.797 |
| 4096 | 4098 | distinct | pending | none | 201618.84 | 204278.98 | 0.987 |
| 4096 | 4098 | distinct | unknown | none | 182259.45 | 185271.00 | 0.984 |
| 4096 | 4098 | distinct | success | none | 120770.39 | 121460.95 | 0.994 |
| 4096 | 4098 | distinct | no_effect | none | 116988.45 | 122585.00 | 0.954 |
| 8 | 1024 | grouped | pending | reference | 393.32 | 352.18 | 1.117 |
| 8 | 1024 | grouped | pending | count | 366.78 | 392.01 | 0.936 |
| 8 | 1024 | grouped | pending | evidence | 43.84 | 44.69 | 0.981 |
| 8 | 1024 | grouped | pending | capacity | 43.02 | 46.73 | 0.921 |
| 8 | 65536 | grouped | pending | none | 422.97 | 406.13 | 1.041 |
| 8 | 1024 | grouped | unknown | reference | 345.42 | 317.68 | 1.087 |
| 8 | 1024 | grouped | unknown | count | 354.32 | 362.12 | 0.978 |
| 8 | 1024 | grouped | unknown | evidence | 42.17 | 41.59 | 1.014 |
| 8 | 1024 | grouped | unknown | capacity | 46.21 | 47.71 | 0.968 |
| 8 | 65536 | grouped | unknown | none | 351.36 | 416.06 | 0.844 |
| 8 | 1024 | grouped | success | reference | 185.53 | 183.51 | 1.011 |
| 8 | 1024 | grouped | success | count | 205.97 | 205.99 | 1.000 |
| 8 | 1024 | grouped | success | evidence | 44.04 | 46.72 | 0.943 |
| 8 | 1024 | grouped | success | capacity | 44.96 | 45.33 | 0.992 |
| 8 | 65536 | grouped | success | none | 270.28 | 292.82 | 0.923 |
| 8 | 1024 | grouped | no_effect | reference | 234.26 | 231.41 | 1.012 |
| 8 | 1024 | grouped | no_effect | count | 251.68 | 271.39 | 0.927 |
| 8 | 1024 | grouped | no_effect | evidence | 43.82 | 50.15 | 0.874 |
| 8 | 1024 | grouped | no_effect | capacity | 47.50 | 44.82 | 1.060 |
| 8 | 65536 | grouped | no_effect | none | 234.96 | 231.93 | 1.013 |
| 8 | 1024 | distinct | pending | reference | 329.29 | 330.17 | 0.997 |
| 8 | 1024 | distinct | pending | count | 303.30 | 305.14 | 0.994 |
| 8 | 1024 | distinct | pending | evidence | 44.11 | 45.72 | 0.965 |
| 8 | 1024 | distinct | pending | capacity | 41.94 | 41.90 | 1.001 |
| 8 | 65536 | distinct | pending | none | 391.81 | 377.20 | 1.039 |
| 8 | 1024 | distinct | unknown | reference | 306.28 | 319.97 | 0.957 |
| 8 | 1024 | distinct | unknown | count | 370.42 | 371.21 | 0.998 |
| 8 | 1024 | distinct | unknown | evidence | 45.32 | 45.12 | 1.004 |
| 8 | 1024 | distinct | unknown | capacity | 54.51 | 46.05 | 1.184 |
| 8 | 65536 | distinct | unknown | none | 318.88 | 329.64 | 0.967 |
| 8 | 1024 | distinct | success | reference | 200.87 | 206.60 | 0.972 |
| 8 | 1024 | distinct | success | count | 235.62 | 236.49 | 0.996 |
| 8 | 1024 | distinct | success | evidence | 47.42 | 46.32 | 1.024 |
| 8 | 1024 | distinct | success | capacity | 44.49 | 45.95 | 0.968 |
| 8 | 65536 | distinct | success | none | 253.82 | 230.82 | 1.100 |
| 8 | 1024 | distinct | no_effect | reference | 186.47 | 233.71 | 0.798 |
| 8 | 1024 | distinct | no_effect | count | 228.08 | 233.23 | 0.978 |
| 8 | 1024 | distinct | no_effect | evidence | 41.35 | 43.28 | 0.955 |
| 8 | 1024 | distinct | no_effect | capacity | 45.81 | 47.16 | 0.971 |
| 8 | 65536 | distinct | no_effect | none | 234.23 | 230.05 | 1.018 |
