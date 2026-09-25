# Supplemental Typed-Launch Regression

These are ordinary CPU regression results, separate from the signed Verus
campaign. They do not qualify native failures, protected Worker execution,
performance, or Context executable refinement.

The only runtime source delta is the retained `source.patch` against signed
`33b4606f4134045757ce6f6d340e829ba79e2314`. It adds a test, not production
behavior. The tested file is
`crates/fe2o3-runtime/src/context/tests/producer_launch_tests.rs`, with SHA-256
`4a899881427ecace149e4338c9153487e53f47871880652dcc55a22ca14835f8`.
The checksum was rechecked after all runs. `rustc.txt` records the toolchain.

## Coverage

The new regression covers both producer positions and writerless/writable
consumers. All physical work succeeds before a producer's result observation is
discarded. Public producer events are released before progress. Context retains
the consumer's physical success but reports conservative
`QuiescentWithoutResult`, releases both reader classes and dependency retains,
preserves Unknown writers, and does not duplicate callbacks or backend actions
on repeated terminal polling. An unreconciled sibling can still complete, and
ordinary cleanup succeeds. Independent source review found no semantic mismatch.

## Results

| Run | Result |
| --- | --- |
| Fully qualified new test | 1 passed, covering four combinations |
| GNU runtime, all features | 1,404 passed, 22 hardware-only ignored |
| musl runtime, all features | 1,404 passed, 22 hardware-only ignored |
| Runtime doctests | 46 passed: four merged and 42 separate |
| Strict all-target Clippy | Passed |
| Workspace formatting check | Passed |

Cargo runs used `--locked --offline -p fe2o3-runtime --all-features`, four
build jobs, disabled incremental compilation, and the exact owned target
`/dev/shm/fe2o3-quiescent-20260925-7LwOPS1e`. Unit suites used `--lib`; the musl
suite added `--target x86_64-unknown-linux-musl`; doctests used `--doc`; Clippy
used `--all-targets -- -D warnings`. The first focused filter matched zero tests
and is retained but excluded; the corrected fully qualified filter ran the test.

All log bytes were copied and compared before removing that owned build cache
and `/home/harsh/.codex-tmp/fe2o3-quiescent-20260925-UGFH2f0Z`. Separate absence
checks passed. No MI300X resources were used. Compiled CPU test caches were
disposable and are not hardware execution artifacts.
