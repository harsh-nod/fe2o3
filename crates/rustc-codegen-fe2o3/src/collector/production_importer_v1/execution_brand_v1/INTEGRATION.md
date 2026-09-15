# Reusable Execution Brand Source Transport

`rust_execution_brand_v1` accepts the reviewed `KernelCapabilityBrand` and chains
of authenticated `ReusableWorkgroupBrand` definitions. It retains the complete
outer rustc type in `RustKernelBrandV1.ty`; only kernel/target/launch coordinates
come from the root. Traversal uses the existing semantic MIR type ceiling.

The plain `rust_kernel_brand_v1` parser is unchanged. Matrix/global-access and
policy helpers which deliberately extract a root keep their existing meaning.

Mounted consumers: workgroup memory brands, Workgroup, Subgroup, WorkgroupLds,
pending async copy, WorkgroupEpoch, scoped atomics, matrix capability, and the
subgroup-partition source checker. Their existing exact brand equality, epoch
transition, reference, ABI, and authenticated-root checks remain intact.

The cached-AMD regression is:

`collector::production_importer_v1::subgroup_partition_v1::workgroup_source_v1::canonical_transport_v1::import_tests::reusable_phase_import_tests::reusable_phase_full_import_gfx950`

It uses the existing `FE2O3_CORE_TRY_*` harness and production macro/session
binding. Source has two reusable phases with `[f32; 2]` LDS of extent 256.
The test checks live published-read types, root/phase distinction, rejects
same-name foreign wrappers, and requires complete canonical import/roundtrip.
It has not been run by this worker; the parent owns central builds/tests.

This is source authentication and transport, not a phase-issuer proof. Distinct
dynamic phase occurrences must still be bound to actual SSA owners, loans,
storage, and epoch transitions by downstream checked lowering. No source-name,
layout, type-only owner, machine evidence, or root-brand substitution is added.
The parent-owned SSA graph/path-region and Context entry files are untouched.
