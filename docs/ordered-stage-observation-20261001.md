# Ordered program compiler stage observations

On 2026-10-01, three real Rust ordered-program profiles matched independent exports from the unchanged diagnostic compiler route. Three negative cases reached their designated rejection checks. This establishes the observation path and its byte comparisons, not the warm generation and validation performance target.

## Actual source results

The source is the checked-in `production-extraction-device/src/ordered_program_v32.rs` fixture, with separate Cargo features for 1, 3 and 16 instructions. Each row is one fresh rustc session using gfx942 with XNACK disabled and wave64. All work ran on the CPU of the `mi350` host; no GPU was executed.

| Ordered steps | Compiler call ms | Frontend to callback ms | Source collection ms | Original validation transition ms |
| --- | ---: | ---: | ---: | ---: |
| 1 | 65.979 | 21.739 | 20.106 | 23.086 |
| 3 | 62.196 | 19.332 | 18.825 | 22.897 |
| 16 | 65.841 | 22.489 | 19.360 | 22.856 |

The [numerical record](evidence/ordered-stage-observation-20261001.json) retains the exact nanoseconds. These are three different single observations, not a percentile series. Cold frontend time cannot be substituted for warmed generation time.

Collection times the original active-session transaction creation. Validation times its consuming `observe_ordered_program_v32()` call. After timing, the original live owner is checked for the exact descriptor profile and full canonical-byte equality with a separately generated baseline. Its semantic, canonical, source-inventory and preflight identities also match the original export.

All three canonical outputs contain 1,419 bytes. Two existing retained receipt components total 37,040 bytes in each case. That sum excludes other source, graph, transaction and rustc owners; it is neither complete retained memory nor peak heap or RSS.

## Rejection checks

| Actual case | Required failure |
| --- | --- |
| One changed byte in a separate baseline copy, with its truthful new hash | `baseline_mismatch` |
| One-instruction source presented as the three-instruction profile | `actual_profile` |
| Existing Rust marker with an invalid zero instruction count | `validation` |

Each negative produced one actual callback, recorded collection and validation intervals, emitted its exact failure and exited as one failing test. A generic panic, compiler fatal, missing callback, missing record or timeout would not satisfy these checks. The genuine baseline files were preserved.

## Build and execution scope

The measured candidate starts from compiler main `b88dc208d37401f88eb9cfdf1c3181035d6ee2d8`, with the source census and executable hashes in the numerical record. The installed compiler reports commit `55e86c996809902e8bbad512cfb4d2c18be446d9`, nightly 2026-04-03, LLVM 22.1.2. This is a backend test build, not release-code performance qualification.

The ordered-program controls passed 27 tests; the external recipe API visibility test passed separately. The earlier recipe control run passed 28 tests. Ignored genuine-source tests are not included in those passing counts.

The campaign used thirteen supervised processes: sysroot observation, Cargo metadata, a fresh dependency build, invocation preparation, three independent original exports, and six source observers. Preparation itself runs no source compiler session. The nine source sessions were not retried or replaced. The fresh dependency output tree had 459 files and 360,634,279 bytes and was hashed before and after the compiler cases.

The outer runner checked the complete current repository census and selected tools before and after execution. The campaign additionally retained exact configurations, invocations, raw streams, baselines and terminal states. This is not an exhaustive system-library or Cargo-cache input census, compiler-closure attestation, or proof of containment for escaped descendants. Failure-stream retention is not a power-loss durability guarantee.

## Interfaces and reproduction

The new private test modules separate controls, genuine-source observations and invocation preparation. The preparation module derives current arguments and crate bindings from real Cargo outputs; it never fabricates a compiler owner or launches subprocesses. Run the ordinary controls with the repository's installed pinned toolchain:

```sh
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib ordered_program -- --test-threads=2
cargo test --locked --offline -p rustc-codegen-fe2o3 --test source_local_order_recipe_measurement_api_v1
```

The ignored source probes additionally require a fresh bounded dependency directory, actual sysroot and Cargo captures, matching environment bindings, independent original-route baselines and a process supervisor. Their closed inputs and exact test selectors are defined in `ordered_program_stage_measurement_v1_tests.rs` and `ordered_program_stage_operational_v1_tests.rs`. Invoking a test filter alone is not equivalent to this source campaign.

The same batch exposes the opt-in `run_source_local_order_recipe_driver_measured_v1`, optional `callback_stage_elapsed_v1()` and `retained_logical_storage_v1()` observations to external callers. The ordinary recipe driver does not read the clock. Its measured sibling still uses a fresh frontend; a returned attempt's storage excludes the already-dropped compiler-stage owners. Genuine ordinary-versus-measured recipe equivalence and warmed replay qualification are separate follow-up work.

## Remaining qualification

Generation time, warm per-stage percentiles and the complete retained-owner aggregate remain unmeasured. These observations therefore do not qualify the 250 ms warm stage or 64 MiB full-payload targets. They do not change source admission, instruction limits, production authority, artifact routes or milestone acceptance. The accepted milestone count remains six of eighteen.

## Integration regression

Before publication, the candidate was fast-forwarded to concurrent main `e11f24c3b8af176b57d9fefd09dd8914a0084616`. Its five incoming changes affect tutorial selection scripts and documentation, not these Rust implementations or source fixtures. The combined candidate passed 27 ordered-program, 28 recipe and one external API test, plus 50 tutorial binding and 56 tutorial identity tests. The three source timings above were not rerun after that integration.
