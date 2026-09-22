# Qualified Actual Stable-Reader Release

Source: 859a4c62483f23888867d31a74dcb5df84160acc.

- Verus: two whole-root runs, each 522 verified and zero errors at default solver limits.
- Twenty controls: fourteen executable mutations, one contract sensitivity, two projections and three witness sensitivities.
- CPU: 901 unit tests and 27 doctests passed; fourteen tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- One selected release benchmark passed, producing 728 rows over 52 case combinations.
- No native GPU execution or HIP/HSA measurements were run.

Actual full-reference lookup, early-return header, ordered release preflight and sequential
take/decrement/free-stack push now share production bodies. The public release slice API
supplies real Vec capacity only after evidence, empty, overflow and arena checks.
The paired harness independently executes actual and historical release with the same
explicit capacity observation. Admission success and desired post-state are not premises.

The raw actual contract requires matching reader/allocation storage lengths. Successful
preflight derives selected-slot uniqueness and sufficient counts, without assuming the
full reader invariant. Errors preserve the whole actual owner; commit preserves the journal
and incarnation, clears selected leases, decrements exact counts, and appends caller-ordered
slots. A malformed existing free prefix is preserved, not sanitized.

Paired custody uses represented reader-invariant pre-state. Concrete witnesses acquire
three live overlapping synchronous leases, release two while retaining one, reject short
capacity, duplicates, replay and stale references, then reacquire a released slot with a new
incarnation. Separate raw undercount and malformed-prefix witnesses cover outside-invariant
contents. Initial fixture storage is synthetic, not actual fallible constructor reachability.

CPU fixtures compare frozen and shared results, complete snapshots, retained custody,
lookup counts and storage identities. A counted expression checks real capacity observation
placement. Same contents with different physical capacities exercise distinct outcomes;
logical-only and physical-only headroom shortages are isolated. Same-owner restoration
touches only precomputed selected leases/counts and truncates the appended free-stack suffix.
Incarnation is asserted unchanged; setup and reset stay outside timed execution.

The 522 obligations include inherited proofs. Public/helper adapters, private scan
declaration and unchanged baseline dependencies are source-authenticated. The formal result
is normal-content behavior with an explicit capacity observation: physical capacity/address,
push nonallocation, allocator/unwind behavior and quiescence authenticity are not proved.
CPU checks of truncated reader storage demonstrate specific early errors, not universal
safety outside the storage-shape domain. Producer admission/release, remaining wrappers,
universal represented-model existence and native producer authority remain open.
Native pending-consumer admission stays closed. Full HIP/HSA parity remains unproved.

## Instrumented CPU Comparison

Ratios are shared/frozen medians over seven alternating rounds. Lower is faster.
These are non-exclusive CPU regression observations with test-only journal lookup counters,
per-call timer and dispatch overhead inside the measurement; restoration is outside it.
Counters do not include all lease/count/free-vector accesses. These are not uninstrumented
production latency, GPU comparisons or an orders-of-magnitude claim.
The complete case roster includes regressions and short/error paths.

| Roster | Capacity | Shape | Fault | Shared ns | Frozen ns | Shared/frozen |
| ---: | ---: | --- | --- | ---: | ---: | ---: |
| 1 | 1024 | grouped | none | 70.94 | 90.33 | 0.785 |
| 1 | 1024 | grouped | late_reference | 56.07 | 58.17 | 0.964 |
| 1 | 1024 | grouped | late_count | 70.95 | 85.34 | 0.831 |
| 1 | 1024 | grouped | evidence | 46.60 | 44.14 | 1.056 |
| 1 | 1024 | grouped | capacity | 54.00 | 48.04 | 1.124 |
| 1 | 1024 | distinct | none | 66.97 | 83.35 | 0.803 |
| 1 | 1024 | distinct | late_reference | 49.62 | 45.64 | 1.087 |
| 1 | 1024 | distinct | late_count | 62.24 | 80.06 | 0.777 |
| 1 | 1024 | distinct | evidence | 43.88 | 45.48 | 0.965 |
| 1 | 1024 | distinct | capacity | 44.24 | 45.19 | 0.979 |
| 8 | 1024 | grouped | none | 208.24 | 330.05 | 0.631 |
| 8 | 1024 | grouped | late_reference | 173.25 | 265.69 | 0.652 |
| 8 | 1024 | grouped | late_count | 194.33 | 305.65 | 0.636 |
| 8 | 1024 | grouped | evidence | 47.52 | 43.46 | 1.094 |
| 8 | 1024 | grouped | capacity | 48.77 | 48.05 | 1.015 |
| 8 | 1024 | distinct | none | 238.19 | 343.59 | 0.693 |
| 8 | 1024 | distinct | late_reference | 214.59 | 372.86 | 0.576 |
| 8 | 1024 | distinct | late_count | 189.87 | 307.61 | 0.617 |
| 8 | 1024 | distinct | evidence | 43.66 | 42.35 | 1.031 |
| 8 | 1024 | distinct | capacity | 43.47 | 43.16 | 1.007 |
| 64 | 1024 | grouped | none | 1668.42 | 2377.81 | 0.702 |
| 64 | 1024 | grouped | late_reference | 1358.39 | 2117.11 | 0.642 |
| 64 | 1024 | grouped | late_count | 1115.68 | 2011.77 | 0.555 |
| 64 | 1024 | grouped | evidence | 43.32 | 42.22 | 1.026 |
| 64 | 1024 | grouped | capacity | 46.25 | 45.62 | 1.014 |
| 64 | 1024 | distinct | none | 1692.38 | 2504.23 | 0.676 |
| 64 | 1024 | distinct | late_reference | 1407.09 | 2148.69 | 0.655 |
| 64 | 1024 | distinct | late_count | 1307.98 | 2013.87 | 0.649 |
| 64 | 1024 | distinct | evidence | 46.47 | 44.62 | 1.042 |
| 64 | 1024 | distinct | capacity | 47.34 | 46.26 | 1.023 |
| 512 | 1024 | grouped | none | 10819.61 | 17876.90 | 0.605 |
| 512 | 1024 | grouped | late_reference | 11392.70 | 17318.60 | 0.658 |
| 512 | 1024 | grouped | late_count | 10181.29 | 17812.61 | 0.572 |
| 512 | 1024 | grouped | evidence | 44.22 | 42.74 | 1.035 |
| 512 | 1024 | grouped | capacity | 47.58 | 46.04 | 1.033 |
| 512 | 1024 | distinct | none | 12718.12 | 18505.35 | 0.687 |
| 512 | 1024 | distinct | late_reference | 10918.25 | 17178.61 | 0.636 |
| 512 | 1024 | distinct | late_count | 10548.77 | 16540.23 | 0.638 |
| 512 | 1024 | distinct | evidence | 49.41 | 47.50 | 1.040 |
| 512 | 1024 | distinct | capacity | 47.88 | 43.36 | 1.104 |
| 4096 | 4098 | grouped | none | 101669.69 | 145618.81 | 0.698 |
| 4096 | 4098 | grouped | late_reference | 86777.30 | 139179.91 | 0.623 |
| 4096 | 4098 | grouped | late_count | 83886.88 | 132437.23 | 0.633 |
| 4096 | 4098 | grouped | evidence | 59.72 | 75.53 | 0.791 |
| 4096 | 4098 | grouped | capacity | 68.50 | 62.14 | 1.102 |
| 4096 | 4098 | distinct | none | 99352.06 | 155507.42 | 0.639 |
| 4096 | 4098 | distinct | late_reference | 87344.73 | 140731.84 | 0.621 |
| 4096 | 4098 | distinct | late_count | 76194.36 | 137244.11 | 0.555 |
| 4096 | 4098 | distinct | evidence | 69.39 | 62.70 | 1.107 |
| 4096 | 4098 | distinct | capacity | 80.17 | 73.30 | 1.094 |
| 8 | 65536 | grouped | none | 204.89 | 337.87 | 0.606 |
| 8 | 65536 | distinct | none | 251.24 | 327.02 | 0.768 |
