# Inspect authored roles and logical values

This development exercise extends the [fixed-register source walkthrough](ordered-region-authoring-v1.md).
It observes the same actual source-produced V16 owner with the CPU debugger.
It is not a shipping V16 debugger input option, a physical-register debugger,
or protected source/artifact admission.

## Two different kinds of information

| Information | What supplies it | What it means |
| --- | --- | --- |
| Scratch/output/input VGPR numbers | Authored region descriptor | Planned roles local to one instruction region |
| Logical input values before the region | CPU checkpoint | Three source values for the selected logical invocation |
| Logical result after the region | CPU checkpoint | One wrapping `u32` result from the whole atomic operation |
| Source and KIR coordinates | Retained lowering correspondence | Which source call produced the logical operation |
| Scratch, EXEC and physical VGPR contents | Unavailable | Neither a static plan nor a logical checkpoint measures these |
| Final-machine mapping and lifetimes | Unavailable | Separate static native reports do not provide a debugger mapping |

For `scratch(32); out(33); in(34)=a; in(35)=b; in(36)=c`, seeing the number
`32` does not mean the debugger observed a value in physical `v32`. There is no
checkpoint between XOR and ADD. The simulator executes the pair as one operation;
the consecutive before/after records surround that complete operation.

## Run the actual-source exercise

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

## Borrowed compiler inspection

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
profile and all original milestone requirements remain unchanged. Ordinary
Bundle-V6/source-edit commands still do not consume this V16 owner, and no new
production selector, persisted debugger transport or source-variable map is
introduced by this inspection exercise.
