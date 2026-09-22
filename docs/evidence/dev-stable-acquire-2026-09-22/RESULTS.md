# Qualified Actual Stable-Reader Acquisition

Source: 7fa83cd8ef5eef9c33ed6bfc18802c870f4f7964.

- Verus: two whole-root runs, each 486 verified and zero errors at default solver limits.
- Nineteen controls: thirteen executable mutations, two contract sensitivities, two projections and two fixture sensitivities.
- CPU: 893 unit tests and 27 doctests passed; thirteen tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- One selected release benchmark passed, producing 588 rows over 42 case combinations.
- No native GPU execution or HIP/HSA measurements were run.

Actual capacity/read validation, header/output checks, ordered preflight and sequential commit
now execute shared production bodies over the private stable owner and the public mutable
output slice. The paired harness independently executes actual and historical acquisition.
Its premises are represented pre-state, mapped inputs, the existing reader invariant and
the machine length bound; no successful admission or desired post-state is assumed.

The raw actual contract requires count-storage shape, but does not require unique free slots.
It preserves sequential alias overwrites, exact error precedence, whole-owner rejection
identity and unchanged output on error. The paired historical invariant supplies uniqueness
for custody preservation. Historical rejection is a contents frame, not whole Vec identity.

Concrete witnesses acquire a live lease and then an overlapping same-allocation batch with
a synchronous consumer, preserving earlier custody; late-member and occupied-output failures
preserve actual identity and mapped contents. A separate malformed alias witness checks two
returned incarnations and the final overwritten slot without claiming valid custody.

CPU fixtures compare frozen and shared results, complete snapshots, retained leases, lookup
counts, owner/output storage identities and same-owner reset. Restoration touches only
precomputed lease/count slots, the popped free-stack suffix, incarnation and output.

The 486 obligations include inherited proofs. Public adapters, private scan declaration,
helper adapters and the handwritten generation getter are source-bound and inspected.
Synthetic fixture construction is not actual fallible constructor reachability. Producer
admission/release, stable release, other wrappers, physical Vec capacity/address and allocator/
unwind semantics, universal represented-model existence and native producer authority remain open.
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
| 1 | 1024 | grouped | none | 154.42 | 156.68 | 0.986 |
| 1 | 1024 | grouped | late_extent | 134.80 | 146.90 | 0.918 |
| 1 | 1024 | grouped | late_output | 98.16 | 102.96 | 0.953 |
| 1 | 1024 | grouped | late_slot | 116.53 | 115.49 | 1.009 |
| 1 | 1024 | distinct | none | 159.39 | 134.39 | 1.186 |
| 1 | 1024 | distinct | late_extent | 122.23 | 122.60 | 0.997 |
| 1 | 1024 | distinct | late_output | 94.48 | 101.10 | 0.935 |
| 1 | 1024 | distinct | late_slot | 135.27 | 135.14 | 1.001 |
| 8 | 1024 | grouped | none | 318.34 | 335.54 | 0.949 |
| 8 | 1024 | grouped | late_extent | 334.68 | 322.25 | 1.039 |
| 8 | 1024 | grouped | late_output | 102.36 | 105.88 | 0.967 |
| 8 | 1024 | grouped | late_slot | 359.42 | 352.08 | 1.021 |
| 8 | 1024 | distinct | none | 429.84 | 438.59 | 0.980 |
| 8 | 1024 | distinct | late_extent | 387.99 | 393.15 | 0.987 |
| 8 | 1024 | distinct | late_output | 99.57 | 103.45 | 0.962 |
| 8 | 1024 | distinct | late_slot | 356.97 | 457.98 | 0.779 |
| 64 | 1024 | grouped | none | 3332.09 | 3373.85 | 0.988 |
| 64 | 1024 | grouped | late_extent | 2263.58 | 2473.28 | 0.915 |
| 64 | 1024 | grouped | late_output | 247.41 | 213.38 | 1.159 |
| 64 | 1024 | grouped | late_slot | 2733.51 | 2140.85 | 1.277 |
| 64 | 1024 | distinct | none | 2702.12 | 2477.74 | 1.091 |
| 64 | 1024 | distinct | late_extent | 1709.54 | 1646.17 | 1.038 |
| 64 | 1024 | distinct | late_output | 217.89 | 200.86 | 1.085 |
| 64 | 1024 | distinct | late_slot | 1566.61 | 1688.31 | 0.928 |
| 512 | 1024 | grouped | none | 23772.93 | 23660.89 | 1.005 |
| 512 | 1024 | grouped | late_extent | 20060.19 | 19357.84 | 1.036 |
| 512 | 1024 | grouped | late_output | 1172.50 | 1015.23 | 1.155 |
| 512 | 1024 | grouped | late_slot | 21709.10 | 22059.06 | 0.984 |
| 512 | 1024 | distinct | none | 24981.81 | 25881.78 | 0.965 |
| 512 | 1024 | distinct | late_extent | 17773.62 | 18368.87 | 0.968 |
| 512 | 1024 | distinct | late_output | 1097.66 | 969.47 | 1.132 |
| 512 | 1024 | distinct | late_slot | 18044.21 | 16698.89 | 1.081 |
| 4096 | 4098 | grouped | none | 164766.27 | 149665.88 | 1.101 |
| 4096 | 4098 | grouped | late_extent | 160218.02 | 137868.11 | 1.162 |
| 4096 | 4098 | grouped | late_output | 7526.31 | 7433.69 | 1.012 |
| 4096 | 4098 | grouped | late_slot | 137690.70 | 135931.80 | 1.013 |
| 4096 | 4098 | distinct | none | 152495.81 | 214644.89 | 0.710 |
| 4096 | 4098 | distinct | late_extent | 138133.86 | 138451.75 | 0.998 |
| 4096 | 4098 | distinct | late_output | 7819.06 | 7482.78 | 1.045 |
| 4096 | 4098 | distinct | late_slot | 144174.05 | 131431.31 | 1.097 |
| 8 | 65536 | grouped | none | 431.14 | 480.31 | 0.898 |
| 8 | 65536 | distinct | none | 546.96 | 691.77 | 0.791 |
