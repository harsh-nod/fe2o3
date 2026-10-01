# Diagnostic Source Expectations

This is an untested compiler-side integration draft. The source-contract
validator and Rust test consumers distinguish diagnostic exports, and the
ordinary CPU driver requires its exact declared contract. The live tutorial
manifest declares that contract for a distinct, source-bound mixed example.
The metadata association is not evidence of driver execution or qualification;
the actual ignored parent and all submission gates still require execution.

## Contract

A source-driver case may declare this exact expectation:

```json
{
  "kind": "diagnostic-kir-export-v1",
  "canonicalKirVersion": 18,
  "diagnosticTileOrders": ["blocked", "striped"],
  "authority": "observation_only"
}
```

`diagnosticTileOrders` is a nonempty, duplicate-free subset of `blocked` and
`striped`, retaining that order. Each value names an explicit existing diagnostic
distribution. Different distributions need not have equivalent whole-kernel
outputs. The selected source, feature set, target and kernel symbol remain in
the existing case and compiler-input contract. The source-item digest binds the
expectation, including the distribution roster.

V18 output is raw canonical KIR, not a simulation bundle. This expectation has
no `bundleVersion`. Existing `verified-bundle-export` and `rejected` expectations
retain their exact bundle-version range of 1 through 6. Unknown kinds, fields,
versions, distributions and authority values are rejected; old consumers reject
the new kind rather than interpreting it as a legacy verified export.

An accepted diagnostic declaration grants no source authentication, proof,
native artifact, load, launch, hardware or performance authority. The declared
target is a compiler input, not evidence of target-matched GPU execution. A
source-bound variant records physical source association only. The pair report
preserves the explicit expectation, remains unqualified, and does not infer
execution, source authentication or semantic equivalence from that association.

## Remaining Integration

Implemented, but not executed or qualified:

- The Rust source-driver corpus reconciles diagnostic associations separately
  from its unchanged eleven verified/P4 inputs and three required negatives.
  Diagnostic cases cannot replace or satisfy those fixed verified selections.
- The ordinary-source driver checks its independent compiler inputs, source
  bytes, root and complete order roster. Before and after its one CPU workflow,
  it invokes the production validator to check the physical package closure and
  contract digest. It retains the binding separately from its existing summary.
- One unconditional ignored parent explicitly refuses unsupported hosts and
  delegates to a private Linux implementation. Existing simulator, debugger,
  independent oracle and refusal checks are retained; there is no second CPU
  workflow and no conditional-registration exception in the source scanner.
- The existing Python matrix includes the focused diagnostic controls once.

Before integration or publication:

- Update the website's strict curriculum expectation validator and matching
  negative tests before repinning it to a manifest with the new kind. Its
  publication checks still require exact compiler commit, tree and blob pins.
- Validate the composed manifest/source bindings against actual bytes and run
  their consumers; retain all existing obligations and zero qualified pairs.
  A source association is not a receipt.

The ordinary driver requires a whole-file source item for CPU lesson tab 7,
`examples/workgroup_sync_v1/src/kernel_mixed_tile_u32.rs`, with its actual byte
range and current package/lock/source-closure digests. The case selects
`mixed-tile-u32-kernel`, with default features disabled, library
`fe2o3_workgroup_sync_v1` at `src/lib.rs`, kernel `mixed_tile_probe`, target
`gfx942`, and both diagnostic orders. Its driver target is
`production_scoped_tile_cpu_driver_v1`; the ignored test remains
`ordinary_mixed_tile_source_executes_public_cpu_cli_paths`. These example-specific
values are test inputs, not special cases in the generic validators. The new
source selection has a distinct inventory identity and its exact display
association; no native qualification follows from either.

The Python-only focused test command is:

```bash
python3 -I -B scripts/tests/tutorial_diagnostic_expectations.py
```

These controls have not been executed as part of this source-only draft. The
full production matrix, relevant Rust tests, actual ignored source/CLI driver,
website validation and normal submission gates remain required. No compiler,
runtime, wire schema or GPU admission behavior is changed here.
