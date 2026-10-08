# Checked-rebind and exact-revision outcome series

Status: test adapter and consistency parser with the source-qualified genuine
CPU checkpoint below. These files alone are not execution evidence. No accepted
performance budget, complete memory result, hardware evidence or milestone
completion is supplied.

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

## 2026-10-08 CPU qualification snapshot

This checkpoint is scoped to `73dacccba6eebdd74e6b9b0ed92e76e338c58d51` plus the ten U4 report/series leaves.
The before/after source census was 15,375 files / 221,500,471 bytes,
SHA-256 `0e94deab4c89f7ae3b8296f0c7da5e6e1bc5f6b617fabf1cfa5e823746a3e843`.
It identifies the tested uncommitted candidate, not a subsequently published
compiler commit or a claim about arbitrary current main. Documentation-only
updates are separate from that tested source census.

A fresh campaign on `mi350-2` completed six genuine children: two ordinary
Create calls retaining original recipe/origin bytes, two independent ordinary
changed-source outcomes, and two series. Each series made 35 fresh consuming
transactions in its genuine frontend callback: five calibration calls and
thirty retained calls. Every outcome matched its independent ordinary oracle;
all 70 raw sample rows were retained. The checked rebind succeeded; the exact
revision workload produced only the designated original source-revision
refusal, not an arbitrary error.

| Workload | p50 (ns) | p95 (ns) | Maximum (ns) |
| --- | ---: | ---: | ---: |
| `checked_rebind` | 168237386 | 168552778 | 168646367 |
| `exact_revision_refusal` | 15954744 | 16136026 | 16151498 |

The values are nearest ranks 15, 29 and 30 of each thirty-sample measured
sequence. They were recomputed from the retained raw rows without dropping
outliers. The measured boundary is fresh transaction creation through the
original consuming recipe return inside an already entered frontend; it is
not cold rustc startup, exclusive generation, whole-process elapsed time,
compiler-owner reuse, a cross-machine result or an accepted SLO. The frontend
is reused, but admitted transaction/graph/Work owners are not.

Retained evidence (SHA-256):

- `u4-main-outcome-six-r1/receipt.json`: 427,327 bytes,
  `9a63d4e394db83ad84a357dadb282fd6a29b0df0e9e74990a94caf233663d1cc`.
- `outcomes/result.json`: 169,477 bytes,
  `0bd18808137c32c0446ee1b8a2c195804ff168f789a467ff95924ddb29965391`.
- `checked-rebind-series.stderr`: 13,699 bytes,
  `5c84115acd592f677d79933e885e618dc0d44ac411a6781a259abd485ac0448e`.
- `exact-revision-refusal-series.stderr`: 13,640 bytes,
  `a395004a4f23f119dd89f0e838907ddcc4a9d8656079681f8a4188c9025b30f8`.

The terminal normal receipt preserves identical selected inputs and source
before/after. Raw stdout and stderr, the original recipe/origin files and
ordinary oracles remain separate evidence; the consistency parser alone is
not execution authentication. Earlier measurements from the separate
`d6653c608210d84f8bde4d7c781492d01357d818` candidate are historical and
are not substituted for this snapshot.

Complete-owner retained heap, temporary-overlap peak and RSS remain unmeasured;
`retained_logical_bytes` remains null and `budget_accepted` remains false.
No cancellation result, full-backend qualification, site publication, native
or GPU authority, or #282 U4 completion follows from these measurements.

## Published implementation and separate backend regression

The ten tested report/series leaves were committed unchanged as
`5654782a316526aef9bb76602343a9a6a903e44d` and pushed to `main` in both
`harsh-nod/fe2o3` and `powderluv/fe2o3`. The dated source census above precedes
that commit; this documentation update is separate.

A genuine Cargo backend-library run on that same candidate passed 4,300 tests,
with no failures and 351 ignored tests. Its terminal receipt is 45,593 bytes,
SHA-256 `923271924a902d7ac6f76187dc78e94e6deff47d2c4f678a6150740d107f8552`.
Ignored tests were not executed. This regression result does not close the
remaining U4 memory, cancellation or performance-budget acceptance items.
