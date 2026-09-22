# Qualified Producer-Owner Stable Wrappers

Source: 6a87055ee303865f023c12af9c6847f5d85431dd.

- Verus: two whole-root runs, each 699 verified and zero errors at default solver limits.
- Eighteen controls: thirteen executable mutations, one conditional-budget contract sensitivity, one projection and three witness sensitivities.
- CPU: 930 unit tests and 27 doctests passed; eighteen tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- One selected release-mode benchmark passed, producing 896 rows over 64 cases.
- No native GPU execution or HIP/HSA measurements were run.

Production and proof share the producer owner's stable acquisition header/delegation
and direct stable release delegation. Actual and historical outer wrappers execute
independently. Exact results, output, stable state and unchanged producer reservations,
free slots, counts and incarnation correspond. Synchronous consumers remain admissible.
Acquisition checks context, consumer ID, roster and output before the combined budget;
that budget precedes the existing stable capacity/epoch check and repeated inner header.
Release reads neither producer budget, counts nor incarnation.

The raw acquire domain requires safe budget arithmetic only after its prefix succeeds,
and stable count storage only after the complete outer header succeeds. Release retains
the existing unconditional stable count-storage premise. No producer count-storage,
valid partition, custody, selected uniqueness or desired-result premise is required
for these raw executions. Historical correspondence adds represented producer custody.

Eight synthetic fixture/witness functions demonstrate nonempty retained NoEffect producer
custody with successful synchronous stable acquisition, late rejection, capacity shortage,
subset release and stale replay; shared-budget precedence; malformed prefixes; irrelevant
producer counts/epoch/budget; and raw aliased stable destinations. They do not prove
fallible Rust construction or settlement reachability. Eleven historical declarations
are projected unchanged into the existing logical type universe.

CPU fixtures use the actual lifecycle with held stable and producer references, both
consumer kinds, all four producer statuses, same resolved allocation custody, exact
state/output/storage comparison and journal access counts. The legal shared-budget
test demonstrates a rejection absent from the stable leaf. The benchmark budget fault
is separately a malformed raw-budget case, not a valid arena population.

Frozen outer methods come from 30aac7720c4a2eef34da3e3682ec5b74784ed61d and invoke
unchanged current stable/capacity leaves. No older leaf baseline contaminates this
wrapper comparison. The stable owner's only source edit is a test-only opaque-helper
export; no mutable production access is added.

Capacity remains an explicit observation forwarded by the proof adapter and remains
delayed inside the runtime stable leaf. Physical storage/nonallocation, allocator
failure/unwind, constructor/settlement reachability, quiescence authenticity and remaining
owner lifecycle wrappers are separate boundaries. Gate 1 remains open and native
pending-consumer admission remains closed; full HIP/HSA parity is not established.

## Instrumented CPU Comparison

Ratios are shared/frozen medians over seven alternating rounds; lower is faster.
These non-exclusive CPU observations include test-only journal counters and per-call
timer/dispatch overhead. O(k) restoration, setup, assertions and counter reset are
outside timing. Full post-state and buffer/storage identities are checked before
each variant batch is reset. Success and late count/extent/slot failures use k
journal accesses, stale final references k-1, and header failures zero.
These are not production latencies or GPU comparisons. Regressions are retained.

| Operation | Roster | Capacity | Shape | Consumer | Fault | Shared ns | Frozen ns | Shared/frozen |
| --- | ---: | ---: | --- | --- | --- | ---: | ---: | ---: |
| acquire | 1 | 1024 | grouped | submission | none | 71.89 | 76.04 | 0.945 |
| acquire | 8 | 1024 | grouped | submission | none | 308.15 | 308.48 | 0.999 |
| acquire | 64 | 1024 | grouped | submission | none | 1802.64 | 2104.15 | 0.857 |
| acquire | 512 | 1024 | grouped | submission | none | 15662.27 | 16588.77 | 0.944 |
| acquire | 4096 | 4098 | grouped | submission | none | 97597.83 | 88387.64 | 1.104 |
| acquire | 8 | 65536 | grouped | submission | none | 231.75 | 226.78 | 1.022 |
| acquire | 1 | 1024 | grouped | synchronous | none | 78.61 | 89.39 | 0.879 |
| acquire | 8 | 1024 | grouped | synchronous | none | 229.62 | 250.40 | 0.917 |
| acquire | 64 | 1024 | grouped | synchronous | none | 1556.75 | 1867.52 | 0.834 |
| acquire | 512 | 1024 | grouped | synchronous | none | 16906.68 | 17900.18 | 0.944 |
| acquire | 4096 | 4098 | grouped | synchronous | none | 116395.80 | 104396.80 | 1.115 |
| acquire | 8 | 65536 | grouped | synchronous | none | 207.58 | 235.10 | 0.883 |
| acquire | 8 | 1024 | grouped | submission | budget | 56.44 | 55.19 | 1.023 |
| acquire | 8 | 1024 | grouped | submission | output | 48.12 | 48.48 | 0.993 |
| acquire | 8 | 1024 | grouped | submission | extent | 181.12 | 155.20 | 1.167 |
| acquire | 8 | 1024 | grouped | submission | slot | 143.39 | 168.15 | 0.853 |
| acquire | 1 | 1024 | distinct | submission | none | 63.57 | 64.17 | 0.991 |
| acquire | 8 | 1024 | distinct | submission | none | 177.76 | 200.07 | 0.889 |
| acquire | 64 | 1024 | distinct | submission | none | 1323.05 | 1601.83 | 0.826 |
| acquire | 512 | 1024 | distinct | submission | none | 8734.91 | 9756.18 | 0.895 |
| acquire | 4096 | 4098 | distinct | submission | none | 109749.25 | 90229.69 | 1.216 |
| acquire | 8 | 65536 | distinct | submission | none | 208.81 | 192.56 | 1.084 |
| acquire | 1 | 1024 | distinct | synchronous | none | 56.63 | 64.98 | 0.872 |
| acquire | 8 | 1024 | distinct | synchronous | none | 166.48 | 183.58 | 0.907 |
| acquire | 64 | 1024 | distinct | synchronous | none | 1339.96 | 1276.66 | 1.050 |
| acquire | 512 | 1024 | distinct | synchronous | none | 10754.39 | 10104.74 | 1.064 |
| acquire | 4096 | 4098 | distinct | synchronous | none | 103794.59 | 114036.64 | 0.910 |
| acquire | 8 | 65536 | distinct | synchronous | none | 229.20 | 204.39 | 1.121 |
| acquire | 8 | 1024 | distinct | submission | budget | 43.90 | 46.21 | 0.950 |
| acquire | 8 | 1024 | distinct | submission | output | 45.97 | 45.96 | 1.000 |
| acquire | 8 | 1024 | distinct | submission | extent | 134.38 | 130.13 | 1.033 |
| acquire | 8 | 1024 | distinct | submission | slot | 133.13 | 144.78 | 0.920 |
| release | 1 | 1024 | grouped | submission | none | 52.56 | 66.76 | 0.787 |
| release | 8 | 1024 | grouped | submission | none | 225.06 | 197.05 | 1.142 |
| release | 64 | 1024 | grouped | submission | none | 1600.39 | 1669.59 | 0.959 |
| release | 512 | 1024 | grouped | submission | none | 11299.00 | 10534.46 | 1.073 |
| release | 4096 | 4098 | grouped | submission | none | 103867.16 | 107116.16 | 0.970 |
| release | 8 | 65536 | grouped | submission | none | 238.76 | 221.50 | 1.078 |
| release | 1 | 1024 | grouped | synchronous | none | 59.81 | 63.56 | 0.941 |
| release | 8 | 1024 | grouped | synchronous | none | 235.88 | 264.65 | 0.891 |
| release | 64 | 1024 | grouped | synchronous | none | 1611.80 | 1498.62 | 1.076 |
| release | 512 | 1024 | grouped | synchronous | none | 12550.71 | 13922.39 | 0.901 |
| release | 4096 | 4098 | grouped | synchronous | none | 125441.36 | 115532.20 | 1.086 |
| release | 8 | 65536 | grouped | synchronous | none | 278.81 | 232.75 | 1.198 |
| release | 8 | 1024 | grouped | submission | reference | 213.48 | 193.14 | 1.105 |
| release | 8 | 1024 | grouped | submission | count | 259.67 | 243.80 | 1.065 |
| release | 8 | 1024 | grouped | submission | evidence | 51.70 | 53.13 | 0.973 |
| release | 8 | 1024 | grouped | submission | capacity | 50.43 | 46.15 | 1.093 |
| release | 1 | 1024 | distinct | submission | none | 71.47 | 73.04 | 0.978 |
| release | 8 | 1024 | distinct | submission | none | 232.14 | 203.66 | 1.140 |
| release | 64 | 1024 | distinct | submission | none | 1355.22 | 1370.16 | 0.989 |
| release | 512 | 1024 | distinct | submission | none | 11495.96 | 10936.43 | 1.051 |
| release | 4096 | 4098 | distinct | submission | none | 100033.92 | 111392.05 | 0.898 |
| release | 8 | 65536 | distinct | submission | none | 205.10 | 234.89 | 0.873 |
| release | 1 | 1024 | distinct | synchronous | none | 60.35 | 66.66 | 0.905 |
| release | 8 | 1024 | distinct | synchronous | none | 201.41 | 225.04 | 0.895 |
| release | 64 | 1024 | distinct | synchronous | none | 1277.84 | 1393.66 | 0.917 |
| release | 512 | 1024 | distinct | synchronous | none | 12686.85 | 12640.41 | 1.004 |
| release | 4096 | 4098 | distinct | synchronous | none | 120577.14 | 124233.05 | 0.971 |
| release | 8 | 65536 | distinct | synchronous | none | 262.84 | 244.13 | 1.077 |
| release | 8 | 1024 | distinct | submission | reference | 211.53 | 225.76 | 0.937 |
| release | 8 | 1024 | distinct | submission | count | 210.68 | 181.87 | 1.158 |
| release | 8 | 1024 | distinct | submission | evidence | 44.08 | 44.90 | 0.982 |
| release | 8 | 1024 | distinct | submission | capacity | 46.97 | 46.45 | 1.011 |
