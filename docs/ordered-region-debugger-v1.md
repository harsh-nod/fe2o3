# Inspect authored roles and logical values

This development exercise extends the [fixed-register source walkthrough](ordered-region-authoring-v1.md).
The ordinary JSONL debugger accepts an explicit raw diagnostic V16 file for
bounded CPU capture. A separate optional compiler qualification observes the
same live source-produced owner. Raw bytes do not reconstruct that source-owner
custody; neither exercise is a physical-register debugger or protected
source/artifact admission. The ordinary CLI qualification passed 30 sessions
and 1,020 commands across five source variants and six requests per variant;
see the [exact evidence scope](ordered-region-authoring-v1.md#evidence-scope-and-further-work).
It checked lane 0 logical values/navigation plus all 64 lanes' output memory and
write history, not all-lane SSA values or physical registers. The retained
private qualification remains a separate exercise.

## Two different kinds of information

| Information | What supplies it | What it means |
| --- | --- | --- |
| Scratch/output/input VGPR numbers | Authored region descriptor | Planned roles local to one instruction region |
| Logical input values before the region | CPU checkpoint | Three source values for the selected logical invocation |
| Logical result after the region | CPU checkpoint | One wrapping `u32` result from the whole atomic operation |
| Source and KIR correspondence | Private retained-source-owner qualification only | Which source call produced the logical operation; absent from raw-file custody |
| Scratch, EXEC and physical VGPR contents | Unavailable | Neither a static plan nor a logical checkpoint measures these |
| Final-machine mapping and lifetimes | Unavailable | Separate static native reports do not provide a debugger mapping |

For `scratch(32); out(33); in(34)=a; in(35)=b; in(36)=c`, seeing the number
`32` does not mean the debugger observed a value in physical `v32`. There is no
checkpoint between XOR and ADD. The simulator executes the pair as one operation;
the consecutive before/after records surround that complete operation.

## Open the ordinary diagnostic debugger

First follow the [ordinary-tool build, export and request steps](ordered-region-authoring-v1.md#use-the-ordinary-diagnostic-tools).
Run the checked-in `inspect_diagnostic_ordered_region_v16` example against the
exact current KIR and request. Its `canonical` identity and `coordinate` roster
ordinals, `input_value_ids`, and `result_value_id` provide the identifiers to use
for this compilation. `raw_block_id` is a separate internal identifier, not a
block ordinal. Do not reuse historical fixture IDs after editing or re-exporting.
The inspector preflights CPU admission but does not execute, authenticate source,
or register a new debugger transport schema.

The minimal JSONL exchange below discovers capabilities and closes the session.
It does not qualify region stepping or values. Use a new response output path:

```sh
printf '%s\n' \
  '{"schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"operation":"discover_capabilities"}' \
  '{"schema":"fe2o3-debug-request-v1","request_id":2,"expected_revision":0,"operation":"terminate"}' \
  | "$ordered_bin/fe2o3-debug" sim --diagnostic-kir-v16 "$ordered_cli_run/used.kir" \
      --request "$ordered_cli_run/request.json" --wave-width 64 --protocol jsonl \
      > "$ordered_cli_run/debugger-responses.jsonl"
```

For interactive requests, run the same debugger command without the pipe and
send one JSON object per line. Follow the [protocol contract](../crates/fe2o3-debug-protocol/README.md),
using the latest session revision returned by the debugger. Select the exact
current function/block/operation roster coordinate, a logical invocation, and
the three current input SSA IDs. Before the operation, the result must be typed
out of scope, not zero. After one atomic region step, inspect its result ID and
compare it with `(a ^ b).wrapping_add(c)`. There is no scratch-value checkpoint
between XOR and ADD. The unused-result variant still retains this logical result
although its surrounding kernel stores `a`.

Allocation, access and memory resource queries require the complete current
checkpoint anchor and matching session/configuration, revision and cursor. Page
tokens are session-local and single-use; stale anchors, tokens and another
request's observations are not interchangeable. Preserve exact JSONL bytes when
recording a session. A wave64 `active_mask` can exceed JavaScript's safe integer
range: do not parse and reserialize arbitrary protocol integers through `Number`.
Use a lossless integer parser/encoder (for example Node.js 22 parse source context
and `JSON.rawJSON`), or retain the raw lines. Fields defined as decimal strings
remain strings.

Logical forward/reverse navigation, SSA values and ordinary allocation-relative
memory/resource views are CPU observations. Source spans/variables, physical
VGPR/SGPR/AGPR values, EXEC, intermediate scratch, physical waves, lifetimes and
final-artifact mappings remain unavailable. The command rejects wave32,
source-map overrides, persisted schedule replay and mixed input selectors; it
does not add a V16 file-descriptor selector. Diagnosis V2 returns
`unsupported_schema` because that frozen evidence type describes canonical V7;
it never relabels V16 or disables the separate resource-query protocol.

## Run the optional retained-source-owner qualification

Run the source ladder command in the [source walkthrough](ordered-region-authoring-v1.md#reproduce-the-six-actual-source-callbacks).
The two positive variant directories additionally contain
`debugger-observation.json`. The existing aggregate observation, canonical bytes
and LLVM records remain separate outputs; negative source cases produce no
successful region capture.

Each positive variant runs six arithmetic cases across 64 logical lanes. For
every lane the harness selects the exact operation's before/after records,
checks the three inputs, verifies that the result is unavailable before its
definition, and compares the after value with independent host arithmetic:

```text
expected = (a XOR b) wrapping-add c
```

It also checks output bytes and unchanged canaries. The unused-result variant
still records the region result, although the surrounding kernel stores `a`.
Inputs are broadcast scalar arguments in these cases; this is not a qualification
of lane-varying input loading or physical wave execution.

Four extra captures per variant exercise a second request with the same KIR but
different data, a one-record truncated transcript, and deliberately unavailable
values. Truncation and value limits remain explicit failures of availability;
they must not appear as zero values or successful complete traces.

The private capture helper retains the actual source owner and immutable request
throughout capture and selection. Its checks include those same live borrows,
canonical digest and length, source occurrence identities, raw operation site,
index width, declared wave width, logical invocation and schedule. Matching a
KIR hash alone does not make two requests interchangeable.

The compact sidecar preserves selected observations, not the complete debugger
transcript. Loading its JSON elsewhere cannot recreate the live-borrow checks,
authenticate source, resume compilation, or admit a detached transcript. Source
availability flags without span coordinates do not authorize source highlighting.

## Borrowed compiler inspection in the optional qualification

`ProductionOrderedRegionPreRankedKirOwnerV16::inspect_ordered_region_v1` borrows
the immutable owner. Supply its expected canonical identity and optionally an
exact roster coordinate. The view exposes the validated two-step plan, actual
logical operands, source occurrence and terminator correspondence, and declared
target/launch fields. A raw block ID is distinct from a block's roster ordinal.

The inspector rejects stale identity or coordinate, unsupported profile,
ambiguous/missing regions, and inconsistent source/physical-role correspondence.
Canonical verification rejects malformed executable input before such an owner
can exist. The view has no public raw constructor or mutation path and cannot
outlive its owner. It grants no proof, source-insertion, artifact or launch
authority; the direct-root profile cannot use the existing helper materializer.

Queries use the caller's cumulative verification ledger plus a local maximum of
1,048,576 precharged logical work units. Reserve the executable and call-mapping
receipts before inspection and the returned fixed-size view receipt while
retaining the view. Every result/unwind restores incoming storage without resetting
accepted work, peak storage or earlier failure history. These are logical bounds,
not total source storage, allocator usage or RSS measurements.

The qualification harness caps each capture at 16,384 records, 128 values per
checkpoint, 1,048,576 retained values and 16 MiB retained memory. Simulation is
limited to 64 invocations, 8,192 steps and 64 MiB resident accounting; each report
is at most 64 KiB. Its truncated/value-unavailable controls deliberately use
smaller limits. These limits do not bound the complete compiler build.

## Scope

This is partial progress on #280 M5, #281 V2/V3 and #282 U1. The fixed source
profile and all original milestone requirements remain unchanged. Ordinary raw
diagnostic V16 input is available; Bundle-V6/source-edit commands still do not
consume this owner. No protected production selector, detached-transcript import,
persisted schedule format or source-variable map is introduced. The debugger does
not infer source-owner or final-artifact authority from diagnostic bytes.
