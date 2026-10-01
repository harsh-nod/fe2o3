# Recipe replay and ordered const observations

On 2026-10-01, one source-derived recipe passed 35 warm replays with exact ordinary-output equality. Its measured p95 was 94.76 ms, below the 500 ms target for this named case. Three ordered-program fixtures also produced source-attributed const-provider intervals. These CPU observations advance the performance evidence for issues #280 and #282; they do not complete a milestone.

## Warm recipe replay

The fixture is the existing positive source-local-order kernel, `choose_order`, computing `(a ^ b) & (c | d)`, with workgroup size 64 by 1 by 1. The compiler target is gfx942, XNACK disabled, wave64. The recipe requests `reverse_ready` and exact-revision replay. No GPU executed.

The campaign first created a recipe, then ran an independent ordinary Replay to retain its complete output. A measured Replay and all 35 warm Replay results matched those 15,326 bytes exactly. Create and Replay were compared on their shared identities and outputs, not on mode-specific recipe-creation fields.

| Observation | Milliseconds |
| --- | ---: |
| Separate measured Replay callback stage | 113.543141 |
| Warm p50 | 94.105939 |
| Warm p95 | 94.760289 |
| Warm maximum | 94.804949 |
| Named warm p95 target | 500 |

The warm series contains five calibration calls followed by thirty measured calls. Percentiles use nearest ranks 15 and 29 of the thirty sorted measured values; the maximum is rank 30. The [numerical record](evidence/recipe-and-const-observation-20261001.json) retains all samples, input hashes, terminal records and audit references.

Warm means repeated work inside one active compiler frontend. Every call creates and consumes a fresh transaction; it does not reuse an admitted compiler owner. Timing covers transaction creation through the original consuming recipe return, including the API's internal checks. Request admission, retained-source opening, companion serialization, hashing and equality checks are outside that interval. This is not fresh CLI latency, debugger reverse replay, a shipping-release measurement, or qualification of every supported recipe.

Eight directly supervised processes performed setup, preparation, Create, ordinary Replay, measured Replay and warm Replay. There were four adapter frontend processes and 38 original adapter calls. Setup Cargo descendants are outside that adapter count. Complete retained logical storage, peak heap and RSS remain unmeasured; the 128 MiB owner target is not qualified.

## Source attributed const observations

Each row uses a separate fresh compiler session on the existing ordered-program source. The observer delegates to the selected const-allocation provider exactly once and joins its row to the actual HIR definition and normalized source callsite. An independent original-route export supplies the full canonical-byte baseline.

| Ordered instructions | Inclusive selected provider ns | Canonical output bytes |
| --- | ---: | ---: |
| 1 | 166840 | 1419 |
| 3 | 177710 | 1419 |
| 16 | 394480 | 1419 |

These intervals include provider work, nested work and observation overhead. Nested durations are not summed. The selected upstream provider's identity was not independently observed, so the accepted metric is a selected-provider inclusive interval, not isolated helper generation or proven exclusive CTFE cost. The raw sidecar retains its historical CTFE-and-checking field spelling; that spelling does not strengthen this interpretation.

The campaign ran three original-route baseline exports and three observations: six adapter compiler sessions across ten directly supervised processes including setup. These are three single observations, not a warm percentile series. Warm generation, the 250 ms per-stage target and the complete 64 MiB payload target remain unqualified.

## Implementation and validation

All new compiler code is private test instrumentation. The production recipe API, source admission, instruction limits and ordinary compiler behavior are unchanged. The additions include a fixed-capacity provider ledger, source attribution, strict configuration and output parsing, fresh-source preparation, and the warm series with an independent full-byte oracle.

The final build passed 49 ordered-program tests, 37 recipe tests, five preparation tests and one external recipe API test: 92 passing tests. Ignored genuine-source entry points are excluded from that count. The operational recipe parent's parser, terminal, roster, percentile and artifact-boundary controls also passed. The source campaigns then ran those ignored entry points under explicit supervision.

Run the ordinary controls with the pinned toolchain:

```sh
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib ordered_program -- --test-threads=2
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib source_local_order_recipe -- --test-threads=2
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib series_prepare -- --test-threads=2
cargo test --locked --offline -p rustc-codegen-fe2o3 --test source_local_order_recipe_measurement_api_v1
```

The ignored entry points require current Cargo metadata, a fresh dependency build, exact derived rustc arguments and crate bindings, independent baselines, closed configurations, and a bounded process supervisor. Their schemas and selectors live in `ordered_program_const_provider_v1_tests.rs`, `source_local_order_recipe_warm_series_v1_tests.rs` and `source_local_order_recipe_series_prepare_v1_tests.rs`. An isolated test-filter invocation is not a reproduction of these campaigns.

The initial warm campaign failed during preparation because its selected core metadata was 41,173,933 bytes, exceeding a setup observer's 16 MiB text-capture limit. It made zero adapter calls and remains a recorded failure. The corrected observer streams artifact hashes in 64 KiB chunks with a separate 128 MiB setup-artifact limit and file-identity checks. The text-capture limit and production limits are unchanged. A new campaign used fresh directories; no failed timing was replaced or converted to zero.

The initial const campaign passed on the preceding test build. It is retained separately; the table above comes only from the second campaign on the final corrected binary. Values from those two campaigns are not pooled.

## Evidence scope and remaining work

Both successful campaigns used the same test binary and complete source census recorded in the numerical record, based on main `188b0a6984460ecf7441b9c39a61072ab6af434e`. The installed toolchain is nightly 2026-04-03, rustc commit `55e86c996809902e8bbad512cfb4d2c18be446d9`, LLVM 22.1.2. Work ran on the CPU of `mi350`. The source build uses `cfg(test)`; no equivalence to a shipping build is inferred.

Each campaign used a separate fresh dependency tree of 459 files and 360,634,279 bytes. The runner retained raw streams and terminal states and checked source, selected input and executable hashes before and after. This does not attest every system library or cache input, guarantee containment of escaped descendants, or provide power-loss durability.

For #282 U4, the result supports the named positive replay-latency subcriterion and corroborates the already accepted U3 deterministic replay path. Complete owner accounting, refusal and rebind workloads, small and tiled end-to-end workflows, independent correctness, target coverage and curriculum integration still need their own evidence. The const observations support #280 M0 baseline work but do not close its generation and validation budgets. Neither compiler campaign directly qualifies #281 debugger milestones. Accepted milestones remain six of eighteen.

In particular, live compiler artifacts coexist with copied output bytes, and source rechecking temporarily retains another source buffer. Complete ownership walkers are still required; adding reservation maxima or measuring only returned attempts would not establish the active-pipeline memory target.
