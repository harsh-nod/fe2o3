# Qualified Actual-Type Begin

Source: `263af455bd9d8bd83de65b7cf67b86178b0a65de`.

- Verus: two whole-root runs, each 45 verified and zero errors at default solver limits.
- 33 scoped controls: 31 executable mutations and two contract sensitivities.
- CPU: 878 unit tests and 27 doctests passed; ten ignored tests in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- The separately selected release benchmark passed with 616 rows across 44 matched cases.

The public Begin method now forwards to the shared actual-type executor. Preflight and
full execution have unconditional raw-content contracts. Stage and commit preserve the
historical storage-ready domain, without uniqueness, canonicality or custody premises.
Concrete executions exercise aliases, dirty tails, raw epoch/lineage ordering, empty
malformed metadata and repeated destinations in the weaker helper domain.

Tests compare complete state, storage identity and exact indexed-access counts against
the frozen original methods from 535763f018e1bf2236d4e8abe7790daf3c9df242. Those methods and
all eight referenced helper bodies are authenticated directly against Git objects.

## Matched CPU Measurements

Each value is the median of seven rounds, in ns per operation. Lower shared/frozen
is better. Both variants use the same journal, warm reset outside timing, matching
iterations, alternating round order, black-box inputs/results and test-only counters.
Complete state/storage/counter equivalence and reset correctness are checked before
timing. The public production Begin method is the shared candidate.

This non-exclusive host measurement includes timer and test instrumentation overhead.
Mixed ratios are not a no-regression gate, an isolated attribution of overhead,
a native GPU benchmark or evidence of HIP/HSA performance parity.

| Roster | Capacity | Pattern | Shared ns | Frozen ns | Shared/frozen |
| ---: | ---: | --- | ---: | ---: | ---: |
| 0 | 1024 | normal | 128.49 | 184.04 | 0.698 |
| 0 | 1024 | dirty_tail | 182.63 | 136.44 | 1.339 |
| 1 | 1024 | normal | 114.15 | 158.39 | 0.721 |
| 1 | 1024 | permuted | 117.23 | 105.98 | 1.106 |
| 1 | 1024 | alias | 104.40 | 140.99 | 0.740 |
| 1 | 1024 | dirty_tail | 172.47 | 195.59 | 0.882 |
| 1 | 1024 | device_first | 154.76 | 83.86 | 1.845 |
| 1 | 1024 | device_last | 90.95 | 78.04 | 1.165 |
| 1 | 1024 | member_last | 98.80 | 88.10 | 1.122 |
| 1 | 1024 | scratch_last | 120.29 | 90.75 | 1.325 |
| 8 | 1024 | normal | 287.76 | 295.76 | 0.973 |
| 8 | 1024 | permuted | 294.06 | 284.68 | 1.033 |
| 8 | 1024 | alias | 282.79 | 255.24 | 1.108 |
| 8 | 1024 | dirty_tail | 258.05 | 238.51 | 1.082 |
| 8 | 1024 | device_first | 84.58 | 79.58 | 1.063 |
| 8 | 1024 | device_last | 116.24 | 125.87 | 0.923 |
| 8 | 1024 | member_last | 165.01 | 140.90 | 1.171 |
| 8 | 1024 | scratch_last | 157.83 | 165.03 | 0.956 |
| 64 | 1024 | normal | 1383.20 | 1379.71 | 1.003 |
| 64 | 1024 | permuted | 1172.73 | 1109.24 | 1.057 |
| 64 | 1024 | alias | 1040.11 | 980.59 | 1.061 |
| 64 | 1024 | dirty_tail | 1014.96 | 1022.98 | 0.992 |
| 64 | 1024 | device_first | 112.22 | 120.20 | 0.934 |
| 64 | 1024 | device_last | 321.34 | 327.88 | 0.980 |
| 64 | 1024 | member_last | 467.03 | 420.74 | 1.110 |
| 64 | 1024 | scratch_last | 406.58 | 418.33 | 0.972 |
| 512 | 1024 | normal | 7566.69 | 7321.07 | 1.034 |
| 512 | 1024 | permuted | 7919.01 | 7699.88 | 1.028 |
| 512 | 1024 | alias | 8027.96 | 8555.14 | 0.938 |
| 512 | 1024 | dirty_tail | 8031.02 | 8744.84 | 0.918 |
| 512 | 1024 | device_first | 664.10 | 773.52 | 0.859 |
| 512 | 1024 | device_last | 2661.54 | 2641.60 | 1.008 |
| 512 | 1024 | member_last | 3662.86 | 3324.38 | 1.102 |
| 512 | 1024 | scratch_last | 3191.78 | 2930.54 | 1.089 |
| 4096 | 8192 | normal | 69378.98 | 64799.77 | 1.071 |
| 4096 | 8192 | permuted | 83499.78 | 86087.06 | 0.970 |
| 4096 | 8192 | alias | 65638.28 | 68564.09 | 0.957 |
| 4096 | 8192 | dirty_tail | 71755.97 | 74333.83 | 0.965 |
| 4096 | 8192 | device_first | 4476.55 | 5258.84 | 0.851 |
| 4096 | 8192 | device_last | 20600.45 | 17943.07 | 1.148 |
| 4096 | 8192 | member_last | 28827.89 | 30693.85 | 0.939 |
| 4096 | 8192 | scratch_last | 26800.14 | 30206.87 | 0.887 |
| 8 | 65536 | normal | 192.03 | 177.35 | 1.083 |
| 8 | 65536 | permuted | 147.68 | 175.20 | 0.843 |

## Limits

This establishes normal-content execution, not physical Vec capacity/address preservation,
fallible allocation, unwind safety, compiled public-wrapper refinement, machine code
refinement, historical issued-custody correspondence or native producer authority.
Constructor refinement and actual reader admission/release remain separate work.
Native pending-consumer admission remains closed. Full HIP/HSA parity is not established.
