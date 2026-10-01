# Native24 Offline Analysis

Source: `a26dbebb57f948439a2e813c7a17c1142014ba1e`. Original execution session: `1309`, terminal `0`.

This is descriptive evidence from one nonexclusive point-idle MI300X campaign, not performance acceptance, engine equivalence, or formal refinement.

## Ordinary Comparison

Mean of two invocation p50s, microseconds per batch. Each copy is 1 MiB; depth is copies per batch.

| Depth | Direction | KFD | HSA | HIP | KFD/HSA delta | KFD/HIP delta |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | forward | 40.345 | 33.581 | 39.298 | +20.14% | +2.66% |
| 1 | reverse | 40.486 | 33.730 | 38.934 | +20.03% | +3.99% |
| 16 | forward | 673.973 | 531.910 | 513.152 | +26.71% | +31.34% |
| 16 | reverse | 674.063 | 532.295 | 516.763 | +26.63% | +30.44% |
| 32 | forward | 1169.533 | 1066.870 | 991.663 | +9.62% | +17.94% |
| 32 | reverse | 1168.306 | 1071.661 | 995.117 | +9.02% | +17.40% |

## Instrumented Host Distributions

Every cell has twenty samples (ten from each of two invocations). Entries are median / nearest-rank p95 in microseconds. Rows are independent distributions and must not be added. Only `operational_checks_ns` and other explicitly named derived fields are per-sample sums.

| Depth | Direction | Elapsed | Submit total | Wait total | Four checks | Roster validation | Scan | Scan thread CPU | Retirement |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | forward | 42.694 / 43.636 | 13.470 / 13.780 | 28.994 / 30.045 | 23.991 / 24.426 | 0.096 / 0.140 | 16.525 / 17.265 | 15.182 / 15.894 | 0.070 / 0.090 |
| 1 | reverse | 43.050 / 43.405 | 13.515 / 13.691 | 29.504 / 29.734 | 24.002 / 24.178 | 0.091 / 0.110 | 17.165 / 17.376 | 15.793 / 15.995 | 0.070 / 0.080 |
| 16 | forward | 683.837 / 686.977 | 16.084 / 16.855 | 667.533 / 670.413 | 24.646 / 25.329 | 0.210 / 0.520 | 653.989 / 656.261 | 73.956 / 77.547 | 0.446 / 0.621 |
| 16 | reverse | 683.562 / 687.828 | 16.079 / 16.685 | 667.237 / 671.664 | 24.702 / 25.216 | 0.205 / 0.400 | 653.528 / 657.723 | 73.582 / 77.013 | 0.461 / 0.681 |
| 32 | forward | 1182.658 / 1191.070 | 18.673 / 22.794 | 1163.630 / 1172.192 | 24.622 / 25.046 | 0.370 / 0.460 | 1149.398 / 1158.492 | 118.980 / 126.120 | 0.681 / 0.911 |
| 32 | reverse | 1182.572 / 1189.248 | 18.728 / 19.900 | 1162.763 / 1167.955 | 24.407 / 25.348 | 0.341 / 0.471 | 1148.626 / 1153.584 | 117.341 / 122.284 | 0.716 / 1.122 |

Complete min/median/p95/max/mean and missing counts for every phase, observation offset, CPU field, and counter are in `analysis.json`. Spin/yield/sleep and context-switch histograms are retained there, as are both invocation identities.

Missing observations across 120 profiled samples: 0 field-values.

## Limits

- Instrumentation changes timing and readiness; profiled data is not an ordinary HIP/HSA comparison.
- Scan wall time includes completion polling, yielding and sleeping while GPU progress overlaps it; it is not pure host overhead.
- Requested sleep is not actual sleep duration. No wall-minus-CPU or requested-sleep value is called avoidable latency.
- Completion offsets are host observations, not device timestamps. HSA/HIP engine identities are unobserved.
- Operational reset/VRAM checks and the wait policy remain unchanged. Scope entry and finish are outside ordinary timed samples.
- Fresh device checks are point-in-time observations, not an exclusive reservation or proof of absent interference.
- Native success and byte replay do not establish universal driver correctness, full HIP/HSA parity, Context-facade coverage, or A7 acceptance.

## Reproduction

Run the adjacent Python controller with `python3 -I -B`, `--prepared` pointing to the retained attempt-1 directory, and `--output` pointing to a new empty result directory. It audits raw hashes, source binding, exact archive members, recorded closed groups and independent replay without launching commands, querying GPUs, or probing old PIDs.
