# fe2o3-hsaco-finalize

`fe2o3-hsaco-finalize` performs bounded post-link finalization of an already embedded canonical
`DeviceDescriptorTableV1` in an AMDHSA HSACO. The one normative ELF section is
`.fe2o3.kd.v1`. It is an 8-byte-aligned, file-backed `SHT_PROGBITS` section with no ELF flags,
so it is neither allocated, writable, executable, nor compressed.

The finalizer accepts at most `fe2o3_hsaco::MAX_HSACO_BYTES` and one descriptor table of at most
`fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES`. V1 deliberately clones the bounded whole
file. It hashes the complete HSACO under `FE2O3/AMDHSA-CODE-OBJECT/V1\0` with only the schema's
fixed 32-byte digest field zeroed, patches that field, then independently reparses, reinspects,
decodes, and recomputes the result. This canonical digest is distinct from any transport or raw
payload digest.

Every path first uses `fe2o3_hsaco::inspect_and_bind_kernel_descriptors`, so every metadata kernel
must resolve through real `STT_FUNC` and `STT_OBJECT` symbols, RO/RX load mappings, and a valid
64-byte AMDHSA kernel descriptor. The embedded table must then agree with that bound evidence on
code-object version, canonical target, complete kernel-name/symbol closure, kernarg size and
alignment, flattened explicit argument order/kind/offset/size/address/access/alignment facts, static
group memory, and represented launch constraints. Optional pointee alignment is checked against the
canonical source element alignment. When present, LLVM `.access` must equal the canonical declared
contract; optimized `.actual_access` may narrow a read-write contract but may never broaden it.
Absence of either field is absence of evidence. True volatile or pipe qualifiers fail closed because
V1 has no canonical representation for either; source type-name strings are not compared. V1 has no
wavefront-size field, so the binding layer checks that fact between metadata and the AMDHSA
descriptor while the table adds no second declaration to compare.

The V1 table intentionally describes caller-provided host arguments. Runtime-populated hidden
arguments are excluded from its flattened argument list, but their boundary and the complete
kernarg size remain checked against metadata. Evidence identities, evidence digests, capabilities,
producer strings, and source identities remain untrusted declarations. Finalization proves only
internal byte integrity and declared metadata closure. It is not Verus verification, compiler
attestation, module-load authority, launch authority, or evidence that a target device matches.

A future compiler integration is responsible for creating the canonical table, embedding exactly
one zero-digest `.fe2o3.kd.v1` section after kernel metadata is known, and invoking this post-link
step before packaging. That responsibility intentionally remains outside `rustc-codegen-fe2o3`,
`cargo-fe2o3`, and this first finalization slice.

## Multi-input native link plans

`MultiInputLinkPlanV1` is a linker-independent description of a reproducible native link.
It binds a canonical concrete AMD target to one or more SHA-256-addressed AMDGPU relocatable inputs,
bounded structured options, an expected executable HSACO identity, and a complete provenance DAG.
Inputs, options, nodes, and parent edges have one canonical order. Duplicate inputs, conflicting
digest lengths, conflicting options, target mismatches, output/input aliasing, unknown parents,
cycles, orphan nodes, and incomplete output-to-input closure fail closed. The output node's direct
parents must be exactly the complete input set.

The plan has a domain-separated stable identity and canonical byte representation. It can verify a
candidate output's expected digest and size without executing a linker. A direct LLVM/LLD worker remains
responsible for mapping each supported option through a structured API, preserving the canonical
input order, inspecting the produced AMDGPU object, and independently finalizing its embedded
descriptor table. A plan does not prove that LLVM/LLD ran, that an option is supported, that the bytes
are valid AMDGPU ELF, or that any device can load or launch them. The existing single-HSACO
inspection and finalization functions are unchanged.

## Compiler FFI request closure

`CompilerFfiClosureV1` is the compiler-neutral input to the G4-to-G1 bridge. Each FFI symbol
retains its exact contract identity, direction, physical ABI, target, code-object version, declared
effects, semantic claim, and stable source owner. The canonical bytes label direction, symbol,
physical ABI, source ownership, and definition location as compiler-derived facts. Target,
code-object version, effects, and semantics remain declaration claims. The bridge also requires a
separate compiler-derived complete required-symbol set, so it never invents kernel entry points from
FFI names.

`bind_compiler_ffi_closure_v1` accepts exact caller-supplied input roles and provider bindings. Input
bindings must match the existing `MultiInputLinkPlanV1` canonical input sequence exactly by digest
and byte length. Every Rust definition must bind to the one Rust compiler LLVM-bitcode input;
every external import must bind to one exact external provider input and its exact
`WorkerInputKindV1`. Contract IDs, source-owner IDs, target, code-object version, input identities,
kinds, roles, ordering, cardinality, and reciprocal references are checked before the bridge creates
`LinkSymbolClosureV1` and `LinkInputKindClosureV1`.

The provider map is still an unauthenticated caller claim. Successful closure does not prove that an
input defines a symbol, that declared effects or semantics are correct, or that any output may be
linked, loaded, or launched. Provider authentication and artifact admission remain later boundaries.
