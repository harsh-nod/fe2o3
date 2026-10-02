# Generated Ordinary Context Arguments

General typed kernels containing only supported primitive scalars and ordinary
shared, write-only disjoint, or read/write disjoint slices now generate
`kernel_name_gpu::ContextArguments`. This type implements the existing
`fe2o3_runtime::RuntimeArgumentsV1` interface for ordinary Context allocations.
The existing borrowed `Arguments` and owned Worker V3 `RuntimeArguments` types
and their authority boundaries are unchanged.

## Public Flow

For a typed kernel with the signature
`context_map(factor: f32, source: &[f32], destination: WriteOnlyDisjointSlice<f32>)`,
the application-side flow is:

```rust,ignore
use fe2o3_host::{GeneratedContextReadSlice, GeneratedContextWriteSlice};

let arguments = context_map_gpu::ContextArguments::new(
    2.0,
    GeneratedContextReadSlice::new(source_allocation, 0, elements)?,
    GeneratedContextWriteSlice::new(destination_allocation, 0, elements)?,
);
let kernel = context.resolve_kernel::<context_map_gpu::ContextArguments>(
    admitted_module,
    "context_map",
)?;
let submission = context.launch_producer_aware_v1(
    stream, &kernel, &arguments, geometry, &producer_events,
)?;
```

This is an integration fragment, not a standalone native program. The Context,
module, stream, allocations, geometry and dependency events must already satisfy
the backend's admission and ownership contracts. For multi-device pipelines,
resolve on each device's admitted module and use allocations resident on that
launch's device; peer-copy submissions move data between devices. Argument
generation does not implicitly copy memory or redirect a cross-device binding.
The real-host downstream fixture at
`crates/fe2o3-macros/tests/fixtures/generic-worker-v3-adapter/src/context_arguments.rs`
exercises this public flow and a two-device queued pipeline with scripted CPU
effects, not generated machine-code execution.

## Contracts

Descriptors are Copy metadata with private fields and fixed Read, Write or
ReadWrite access. Construction checks nonzero element counts, scalar-aligned
offsets and checked byte extents. Descriptors retain no allocation custody or
Rust borrow. Context admission checks live identities, device membership and
bounds; backend-specific kernel/effects/alias checks still apply.

Encoding uses canonical ABI offsets, little-endian scalar/count bytes, zero
padding and zero pointer slots. Bindings identify allocation-relative regions
and exact pointer patch positions. The signature is the generated host contract
identity, not an authenticity proof. Supported primitive types are i8/u8,
i16/u16, i32/u32, i64/u64 and f32/f64. Mapped slices, raw device-global pointers
and compiler-laid-out aggregates do not generate this adapter.
When renaming the device dependency, import the marker types and use their
unqualified names; the existing parser does not recognize renamed-qualified
spellings such as `gpu_device::WriteOnlyDisjointSlice` as ordinary slices.

No descriptor, generated signature, or example authorizes an HSACO. Native
execution still requires authenticated compiler lineage and invocation-specific
effects, alias, initialization, bounds and quiescence evidence. This change
neither supplies the production verifier/refinement provider nor converts
private Worker V3 generated storage into ordinary allocations. It does not
establish native execution, formal machine refinement or performance parity
for these generated arguments.

The [CPU qualification packet](evidence/dev-context-arguments-2026-10-02/README.md)
records the exact source, executables, tests and retained diagnostics.
