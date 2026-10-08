# Checked-rebind and exact-revision outcome series

Status: source-only test adapter and consistency parser. No genuine run, compiler
qualification, accepted performance budget, complete memory result, hardware
evidence, or milestone completion is supplied by these files.

This extends the existing recipe observation tests, not the production API. Two
closed workloads use the unchanged consuming
`compile_source_local_order_recipe_v1` route:

* `checked_rebind`: the existing renamed-source edit replays an original
  `reverse_ready`, exact `or_before_xor`, `rebind_current` recipe. Current source
  bytes and initializer span differ; the original instance axes and recipe bytes
  remain equal. The actual returned evidence must demonstrate these relations.
* `exact_revision_refusal`: the same edit replays an original exact-revision
  recipe. Only the original `RecipeBinding` phase and exact diagnostic
  `local-order recipe source revision changed`, with no compiler fatal, count
  as the intended outcome. Another error is not a successful negative.

The existing release recipe ladder already exercises checked edits, rejected
edits, independent whole-kernel simulation and fresh identities. This adapter
does not replace that correctness qualification. It fills the missing repeated
outcome-measurement seam: the earlier warm adapter accepts only successful
outputs and aborts on every original error.

## Source and oracle chain

A root-owned qualification campaign must first run the normal, non-measured
ordinary Create driver for each selected recipe binding mode. Retain the exact
`FE2O3_RECIPE_NORMAL_V1` JSON payload including its trailing newline as a pinned
regular file. The new adapter joins its emitted recipe bytes, recipe hash,
origin source hash, five instance axes, source initializer, order and constraint
to the unchanged retained recipe. Origin annotations alone do not authenticate
the old source; the original successful command and source/input receipts are
required externally.

Derive the current invocation using the existing real fixture/invocation
helpers, using `source_local_order_release_recipe_fixture_v1_tests::source`
for the `positive` and `renamed` source cases. Do not hand-construct a semantic
graph or reuse an admitted compiler owner. Save old and current source bytes
separately; never overwrite the original recipe or a prior output.

Next run this adapter in `ordinary` mode for the current workload. Retain its
exact `FE2O3_RECIPE_OUTCOME_V1` JSON payload, including newline, as the separate
ordinary outcome oracle. Only then run `series` for that same workload. The
normal successful command receipts, source/config/recipe pins and callback
environment remain root-owned prerequisites; the parser does not authenticate
them. Neither a prepared config nor a fabricated JSON record is execution proof.

## Closed input

Set `FE2O3_RECIPE_OUTCOME_CONFIG` to a pinned absolute regular config path and
`FE2O3_RECIPE_OUTCOME_CONFIG_SHA256` to its lowercase SHA-256. The JSON has
exactly these fields:

```json
{
  "schema": "fe2o3-recipe-outcome-series-input-v1",
  "mode": "ordinary",
  "workload": "checked_rebind",
  "invocation": {
    "schema": "fe2o3-recipe-series-input-v1",
    "mode": "ordinary",
    "source": "ROOT_DERIVED_CURRENT_RELATIVE_SOURCE",
    "source_sha256": "ROOT_MEASURED_CURRENT_SOURCE_SHA256",
    "intent": {
      "action": "replay",
      "recipe": {
        "path": "/ROOT_OWNED_RECIPE",
        "sha256": "ROOT_MEASURED_RECIPE_SHA256"
      }
    },
    "rustc_args": ["ROOT_DERIVED_COMPLETE_CURRENT_INVOCATION"],
    "oracle": null
  },
  "origin_oracle": {
    "path": "/ROOT_OWNED_ORDINARY_CREATE_PAYLOAD",
    "sha256": "ROOT_MEASURED_CREATE_PAYLOAD_SHA256"
  },
  "ordinary_oracle": null
}
```

These uppercase placeholders are intentionally invalid, not runnable defaults.
Series mode differs only in top-level `mode: "series"` and the actual pinned
`ordinary_oracle` object. The second workload uses
`workload: "exact_revision_refusal"` and an actual exact-revision recipe and its
own Create and ordinary-refusal oracles. No measured/cancel/arbitrary-error
mode, callback, pass list, source-owner import or proof override is accepted.

Ignored child selector:

```text
production_rustc_driver_v1::source_local_order_recipe_driver_v1::warm_series::outcome_series::actual_source_local_order_recipe_outcome_series_v1
```

The root must confirm exact discovery in the actual fresh backend test ELF;
the selector text alone is not discovery evidence.

## Measurements and failure retention

Each series reserves exactly 35 calls upfront: five calibration calls followed
by thirty retained samples. Each call admits a new request and retained source,
creates a fresh transaction in the same genuine frontend callback, and consumes
it through the original route. No transaction, graph, Work, source owner or
result owner is reused across calls.

The timer covers transaction creation through the original recipe return.
Request/file admission, origin comparisons, external-oracle serialization,
hashing, comparison, output and statistics are outside it. API-internal checks
stay inside. A valid designated refusal has its real elapsed duration; an
unexpected failure has no percentile result. Ordinary mode never reports a
duration. Statistics use nearest ranks 15, 29 and 30 of the thirty measured
samples; calibration, outliers and refusals are not silently discarded.

The unchanged original 35-call selected-input envelope is retained. Additional
origin and ordinary-oracle descriptor traffic is bounded separately by three
reads of at most 524,289 and 1,056,769 bytes respectively. These are I/O bounds,
not memory measurements. Config admission remains bounded by 2 MiB; the output
record is bounded by 1,056,768 bytes, each row by 2/4 KiB and each parser stream
by 4 MiB. The actual raw rows are flushed individually; later panic, output
refusal or deadline can leave only a completed prefix. Such a prefix is never
qualified.

A 60-second between-call stopping check does not interrupt an in-flight
compiler call or extend the root deadline. External CPU/process deadlines,
complete source/tool/dependency currentness, evidence cap and process cleanup
remain required. No automatic retries, target reuse, cap increases, cache
mutation, GPU work or subprocess campaign are implemented here.

Retained logical bytes remain null. Complete source/compiler/graph owners,
temporary overlap, heap and RSS are not measured. No budget is accepted from
the percentile values.

## Parser and controls

```sh
node scripts/recipe-outcome-series-v1.mjs /ordinary.stdout /ordinary.stderr /series.stdout /series.stderr
node --test scripts/recipe-outcome-series-v1.test.mjs
```

The parser compares the complete ordinary and series outcome bytes; it rejects
old schemas, duplicate JSON fields, wrong workload/source/recipe/oracle joins,
wrong original failure phase/diagnostic/fatal state, incomplete/extra/missing/
duplicate/reordered samples, calibration mistakes, fabricated statistics and
authority or memory overclaims. Its result is explicitly consistency-only,
not execution authentication. Pure Rust and Node controls use inert synthetic
records and must never be reported as genuine workloads.

Cancellation remains a separate owner/interface gap. A process timeout or
SIGTERM is not a compiler cancellation result. This change invents no
cancellation API and does not satisfy that acceptance item.
