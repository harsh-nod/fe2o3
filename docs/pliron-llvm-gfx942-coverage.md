# `pliron-llvm` v0.17.0 coverage for the gfx942 finalizer

Status: upstream source audit at Pliron commit
[`2610651306ea3ba670f68d5d8b1e1159bcd521ed`](https://github.com/pliron-org/pliron/tree/2610651306ea3ba670f68d5d8b1e1159bcd521ed),
the commit published as `pliron`/`pliron-llvm` v0.17.0. fe2o3 implementation
status includes one bounded backend-fixture-to-MI300X scalar closure at
`fd6520d88`, `70f9c5ad7`, `e016833d3`, `c9e8ca702`, `62efd243e`, and
`228c88ed9`. The active dependency is reviewed fork commit
[`5bdf861bf03e7f20242b25717fb653336d02e487`](https://github.com/harsh-nod/pliron/tree/5bdf861bf03e7f20242b25717fb653336d02e487),
a strict descendant that adds mutation-attempt epochs without changing the
audited `pliron-llvm` surface below.

## Scope and conclusion

The root manifest pins that exact reviewed fork revision. fe2o3 now uses
`pliron-llvm` selectively as a typed dialect dependency with
`default-features = false`. Its optional `llvm-sys` converter is not used in
the producer or inside the production worker. LLVM target-machine and
in-process LLD work remains assigned exclusively to the isolated measured
upstream LLVM 22.1.8 finalizer.

The audited `pliron-llvm` is a useful target-neutral LLVM dialect and a partial
LLVM-C importer/exporter. It is **not** a drop-in representation or finalizer for
the fe2o3 gfx942 contract. It covers much of the ordinary SSA instruction set,
but it cannot preserve or emit several correctness-bearing AMDGPU ABI details:
calling conventions, function/parameter attributes, module target state,
metadata, strict constrained-FP metadata operands, and some memory-operation
flags. Its LLVM wrapper also has no target-machine object emission and no LLD
API. Those absences are architectural, not a short list of spelling changes.

The architecture decision is:

1. Keep the existing isolated finalizer worker as the only authority for target
   setup, device-library linking, optimization, object emission, LLD, and HSACO
   inspection.
2. Use only the reviewed `pliron-llvm` dialect surface. Do not run its
   LLVM-C/`llvm-sys` converter in any production component, including the
   worker.
3. Let fe2o3 derive canonical V2 from the live graph and serialize only the
   admitted subset to deterministic bounded LLVM assembly. Do not fork a
   second target machine or linker implementation into the Pliron path.

## Implemented scalar boundary

The current scalar slice structurally parses an embedded backend fixture and
constructs a real dialect graph for one load/strict-`fadd`/store/return kernel.
The extractor derives operations, operands, results, types, and CFG from that
live graph. A validated V1 sidecar still supplies the AMD calling convention,
target attributes, module metadata, and origin/obligation evidence because
upstream v0.17.0 has no lossless dialect representation for them. V2
construction requires exact graph/sidecar agreement. The fixture is not Rust
user source and does not demonstrate a Rust frontend-to-machine path.

The graph-derived extractor (`62e66209e`), V2 serializer (`3a3b43e90`), and
attempt-scoped bridge (`cb571012f`) are implemented. The serializer produces
deterministic bounded LLVM assembly and binds its digest to the source handoff
identity. The bridge binds those bytes through the compiler handoff, symbol
manifest, link plan, measured worker identity, and sealed Worker V2 request. It
is inert and grants no object, link, publication, load, or launch authority.
The closed route adds hardened Worker `fd6520d88`, exact ELF and machine
inspection `70f9c5ad7`, measured-HSACO admission `e016833d3`, move-only Worker
execution evidence `c9e8ca702`, dedicated repository-policy/finalizer/runtime
join `62efd243e`, and alignment correction `228c88ed9`. Existing low-level HSA
adapters were reused, but a dedicated sealed consumer was required. The COV6
descriptor reports a 280-byte kernarg segment (24 explicit plus 256 hidden)
with alignment 8; ROCr reports runtime alignment 16, which the consumer
enforces.

The exact MI300X run completed with
`evidence=69238ad704470649b9811b41cf0194bb392be8116a1b0618adb1dcbe7e1bbd4f`
and ROCr 1.18 runtime image
`7010eba894569c044749b71b63ff782080c4a91e19ff24d6dc93e857045ab37e`.
The embedded checkout policy and marker are consistency evidence, not an
external signature or CI attestation.

## Status definitions

| Status | Meaning in this audit |
| --- | --- |
| **Supported** | The v0.17.0 dialect and exporter preserve the semantics needed by the stated fe2o3 requirement. |
| **Extension required** | A nearby representation or LLVM-C conversion exists, but a correctness-bearing property is absent, dropped, or insufficiently verified. |
| **Missing** | The capability is absent from the dialect/wrapper or cannot represent the required LLVM construct. |
| **Out of scope** | LLVM supports the construct, but the closed fe2o3 gfx942 profile does not admit it and should reject it. |

"Supported" does not mean that arbitrary imported LLVM is trusted. It describes
the individual surface only; the fail-closed section below evaluates the whole
conversion boundary.

## Audited requirements

The comparison uses the repository's implemented requirements, not a generic
AMDGPU checklist:

- `amdgpu_kernel`, gfx942 target identity, exact data layout, PIC relocatable
  object emission, and configured code-object version
  ([implementation plan](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/docs/implementation-plan.md#L161-L194),
  [current module emission](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L5724-L5865));
- integer/FP/vector operations, branches, pointer arithmetic, loads/stores,
  calls, thread indices, and debug metadata
  ([milestone scope](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/docs/implementation-plan.md#L321-L329));
- named AMDGPU synchronization scopes, atomics, barriers, volatile memory,
  constrained FP, OCML and LLVM/AMDGPU intrinsics
  ([atomic lowering](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L7200-L7235),
  [fences](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L7775-L7813),
  [constrained FP declarations](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L3297-L3335));
- function, argument, and module attributes plus kernel and debug metadata
  ([function emission](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L5831-L5907),
  [debug/module metadata](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/rustc-codegen-fe2o3/src/source_debug.rs#L1544-L1574)); and
- measured device-library linking, pass pipelines, target-machine object
  emission, in-process LLD, and inspection of the resulting AMDGPU ELF and
  MsgPack metadata
  ([architecture gate](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/docs/pliron-wave0-architecture.md#L672-L688),
  [worker pipeline](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/src/WorkerPipeline.cpp#L5998-L6247)).

## Operations

The upstream operation inventory is implemented in
[`ops.rs`](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs).
The LLVM-C conversion behavior is in
[`to_llvm_ir.rs`](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs)
and
[`from_llvm_ir.rs`](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs).

| fe2o3-required surface | v0.17.0 coverage | Status | Required action |
| --- | --- | --- | --- |
| Integer arithmetic and bit operations | `add`, `sub`, `mul`, shifts, div/rem, `and`/`or`/`xor`; `nsw`/`nuw` exist where modeled ([ops](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L186-L325), [flags](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/attributes.rs#L37-L42)). | **Supported** for current lowering | Preserve the existing fe2o3 legality checks. Add `exact` only if admitted by a future profile. |
| Integer/FP comparison and `select` | `icmp`, `fcmp`, predicates, and `select` are represented and exported ([comparisons](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L339-L437), [FP/select](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L3300-L3540)). | **Supported** | None beyond profile verification. |
| Ordinary FP arithmetic and casts | `fneg`, `fadd`, `fsub`, `fmul`, `fdiv`, `frem` and integer/FP casts exist; fast-math flags are modeled ([fast math](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/attributes.rs#L44-L126), [ops](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L2870-L3370)). | **Supported** for non-strict profiles | Keep fast-math policy in fe2o3 rather than accepting imported flags unchecked. |
| Strict/constrained FP | Generic intrinsic calls exist, but LLVM constrained intrinsics require `metadata` operands for rounding and exception behavior. The type importer explicitly leaves `LLVMMetadataTypeKind` unimplemented ([intrinsic call](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L4230-L4435), [missing type](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L301-L309)). | **Extension required** | Upstream a metadata-as-value representation and round trip. Until then, the strict fe2o3 profile must reject this export path. |
| CFG and SSA joins | `br`, conditional branch, `switch`, `indirectbr`, `return`, and `unreachable` exist. LLVM phi nodes map to Pliron block arguments ([terminators](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L626-L1429), [phi conversion](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L390-L526)). | **Supported** | Reject indirect branches in the closed profile unless the existing CFG contract is expanded. |
| Stack allocation and pointer/integer/address-space casts | `alloca`, `bitcast`, `inttoptr`, `ptrtoint`, and `addrspacecast` exist ([ops](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L447-L624)). | **Supported** as representation | Keep fe2o3's narrower pointer-provenance and address-space policy. Dialect availability is not permission to use a cast. |
| GEP | Typed indices are modeled and exported, but the operation has no `inbounds` property ([GEP](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L1433-L1614), [export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L1492-L1525)). | **Extension required** | Upstream LLVM GEP flags, preserving poison semantics exactly. Fail export when fe2o3 requires `inbounds` and the property is absent. |
| Loads and stores | Alignment is represented. Ordinary and atomic forms exist, but ordinary load/store have no `volatile` property ([load/store](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L1617-L1948), [fe2o3 volatile emission](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L6340-L6380)). | **Extension required** | Upstream volatile and any other generally applicable memory flags. Never silently downgrade volatile to ordinary memory. |
| Atomics and fences | `atomicrmw`, `cmpxchg`, atomic load/store, orderings, and string-valued sync scopes exist. Export can create a named LLVM sync-scope ID. Import preserves only `singlethread`; any other non-system scope falls back to system ([atomic attrs](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/attributes.rs#L239-L320), [scope import](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L972-L983)). | **Extension required** | Upstream lossless named-scope import and missing `cmpxchg` flags such as `weak` if used. fe2o3 must whitelist exactly `workgroup`, `agent`, and `wavefront`, plus permitted orderings and widths. |
| Direct and indirect calls | Both are represented; ordinary call export and fast-math application exist ([call op](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L2004-L2300), [export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L1323-L1360)). Tail kind, calling convention, and call-site attributes are not modeled. | **Extension required** | Upstream general call properties. fe2o3 should continue rejecting indirect calls and unexpected call edges at its closed-world gate. |
| LLVM and AMDGPU intrinsics | `llvm.call_intrinsic` resolves known LLVM intrinsics and adds declarations ([export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L1363-L1419)). Ordinary work-item IDs, barriers, ballot, DS, MBCNT, MFMA, trap, memcpy, and integer intrinsics are expressible when their types are otherwise covered. | **Supported** with exceptions | Maintain an exact fe2o3 intrinsic allowlist and signatures. Constrained FP remains blocked by metadata; target-specific convergence/attributes still need the attribute extension. |
| OCML calls | OCML functions are ordinary external calls, so signatures and calls are expressible. `pliron-llvm` does not provide, measure, link, or validate OCML bitcode. | **Supported** as IR only | Device-library selection and C-ABI validation stay in the worker. |
| Inline assembly | Template, constraints, and a `convergent` field are modeled. Export hard-codes side effects and AT&T dialect and explicitly does not apply convergence; import sets convergence false ([export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L1038-L1097), [import](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L998-L1006)). fe2o3 uses physical `s_barrier` inline assembly ([lowering](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L6450-L6459)). | **Extension required** | Upstream all inline-asm semantic fields and lossless conversion. The fe2o3 gate must require side effects and convergence where the selected barrier/MFMA form needs them. |
| Constants, undef/poison/freeze/zero | Scalar/vector constants plus `undef`, `poison`, `freeze`, and zero are represented ([ops](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L2303-L2486)). Constant-expression import has an explicit `todo!` for unsupported opcodes ([import](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L730-L760)). | **Supported** for produced constants | Reject unsupported imported expressions before conversion or replace all importer panics with typed errors. |
| Globals and addresses | Globals carry type, initializer, linkage, address space, and alignment; address-of/block-address forms exist ([globals](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L2488-L2868)). Other LLVM global properties are not comprehensively modeled. | **Extension required** | Upstream general global properties as needed. Add fe2o3-specific validation for LDS globals, linkage, mutability, AS3, and initializer form. |
| Aggregate/vector insert, extract, shuffle | Represented, but exporter rejects multi-index `insertvalue`/`extractvalue` because the used LLVM-C builder API accepts one index ([conversion error](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L153-L170)). | **Extension required** for nested aggregates | Upstream lowering into a sequence of single-index operations or use an API that preserves the full index path. Current shallow uses remain supported. |
| EH, `invoke`, `callbr`, pads, and `resume` | Import arms are `todo!` and no complete dialect path exists ([import](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L1181-L1186)). | **Out of scope** | Explicitly reject. These operations are not part of the closed kernel profile. |

## Types

The dialect defines LLVM structs, opaque pointers with numeric address spaces,
arrays, void, function types, and fixed/scalable vectors. Integer and common FP
types reuse Pliron builtins
([type definitions](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/types.rs#L30-L433),
[type export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L315-L457)).

| fe2o3-required surface | v0.17.0 coverage | Status | Required action |
| --- | --- | --- | --- |
| Integers `i1`, `i8`, `i16`, `i32`, `i64`, and wider integers | Arbitrary-width builtin integers export to LLVM integer types. | **Supported** | fe2o3 still limits widths per operation/profile; representation support does not make every width legal. |
| `half`, `float`, `double` | F16/F32/F64 import and export are implemented. | **Supported** | Current profile policy still determines which operations permit F64. |
| `bfloat` | LLVM bfloat import is `todo!`; no corresponding export case exists ([import](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L286-L309)). Current fe2o3 carrier lowers F16/BF16 storage through `i16` in its admitted path ([mapping](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L7860-L7893)). | **Supported** only for the current `i16` carrier; otherwise **Missing** | Upstream native bfloat support before admitting native bfloat LLVM IR. Do not reinterpret native bfloat as `i16` implicitly. |
| Opaque pointers and AMDGPU address spaces | Opaque pointers store a `u32` address space and round trip. | **Supported** | fe2o3 must enforce the meaning and permitted uses of AS0/1/3/4/5 rather than merely accepting a number. |
| Arrays, literal/identified structs, fixed/scalable vectors | Represented and converted. | **Supported** | Reject scalable vectors in the closed gfx942 profile unless deliberately added and resource-accounted. |
| Function types | One LLVM result (which may be void), parameter list, and variadic bit are represented. | **Supported** | Kernel entry signatures must be non-variadic and pass the fe2o3 ABI validator. |
| `metadata` values | Type import is `todo!`; metadata-as-value is also unimplemented. | **Missing** | Upstream a first-class, typed metadata value path. This blocks constrained FP and debug intrinsics. |
| `token`, target extension, x86 AMX, non-AMDGPU extended FP types | Import cases are `todo!` ([type switch](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L301-L309)). | **Out of scope** for gfx942 | Return a structured unsupported-type error, never panic. |

## Calling conventions and attributes

The dialect's attribute module covers arithmetic overflow/fast-math flags,
comparison predicates, GEP indices, linkage, alignment, address spaces,
constant sentinels, atomic orderings/RMW kinds, and shuffle masks
([attributes](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/attributes.rs#L37-L320)).
It does not provide a general LLVM function/return/parameter/call-site attribute
model.

| fe2o3-required surface | v0.17.0 coverage | Status | Required action |
| --- | --- | --- | --- |
| Kernel calling convention `amdgpu_kernel` | `FuncOp` stores function type, linkage, and symbol name. Function conversion applies linkage but no calling convention ([function op](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L4526-L4688), [export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L2061-L2115)). | **Missing** | Upstream numeric/named LLVM calling conventions would improve dialect fidelity. The current scalar requires `amdgpu_kernel` from its validated V1 sidecar and emits it only through the fe2o3 serializer. |
| `nounwind`, `convergent`, memory effects, `speculatable`, `willreturn` | Not modeled or round-tripped as function attributes. fe2o3 emits these on kernel/intrinsic declarations ([current attributes](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L5831-L5856)). | **Missing** | Upstream a lossless general attribute-set representation. The current scalar's exact V1 sidecar supplies the closed required set to V2; the fe2o3 serializer rejects anything outside that set. |
| `target-cpu`, `target-features` | No function string-attribute representation or conversion. | **Missing** | Upstream general string attributes are useful. The current scalar obtains exact gfx942, sramecc, xnack, and wave policy from the validated V1 target sidecar. |
| `amdgpu-flat-work-group-size` and implicit-argument attributes | No representation or conversion. | **Missing** | Implement as general string attributes upstream where possible. The current scalar sidecar fixes `1..64`; Wave64 remains separate target policy, and hardware launch above one workitem is not admitted. |
| Argument/return attributes such as `noalias`, `nocapture`, `readonly`, `writeonly`, and `align` | No parameter or return attribute lists. | **Missing** | Upstream indexed LLVM attribute sets. fe2o3 derives them from its ABI/effect analysis and verifies that export did not lose them. |
| Linkage and global alignment | Represented and applied for functions/globals ([module conversion](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L2583-L2680)). | **Supported** | Whitelist linkages by symbol role. Keep exact export-set and unresolved-symbol checks in the worker. |
| Instruction flags | Fast-math and some `nsw`/`nuw` flags are represented; `inbounds`, volatile, `exact`, tail kind, and complete atomic/call flags are not. | **Extension required** | Upstream per-instruction LLVM semantic properties. Add verifier rules so omission is an error whenever the source contract requires a property. |

## Module state and metadata

`convert_module` creates a fresh LLVM module and converts globals/functions. It
does not apply a target triple, data layout, module flags, named metadata,
function/parameter attributes, or calling conventions
([export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L2583-L2680)).
The importer similarly walks globals and non-intrinsic function definitions but
does not import those module contracts
([import](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L1749-L1791)).

| fe2o3-required surface | v0.17.0 coverage | Status | Required action |
| --- | --- | --- | --- |
| Target triple `amdgcn-amd-amdhsa` | Not represented or converted. | **Missing** | A general module target property is upstreamable. The current scalar's validated V1 target sidecar fixes the triple, and the fe2o3 serializer emits it exactly for worker revalidation. |
| gfx942 data layout | Not represented or converted. | **Missing** | Upstream a module data-layout property. The current scalar sidecar and serializer fix the expected layout; the measured target machine remains authoritative and must reject disagreement. |
| `amdhsa_code_object_version`, sramecc, and xnack module flags | No module-flag model. | **Missing** | Upstream general module flags if lossless behavior can be defined. The current scalar retains exact module/target policy in its V1 sidecar and V2 handoff. |
| `!reqd_work_group_size` and kernel argument metadata | No instruction/function metadata attachment or named metadata model. | **Missing** | Upstream generic metadata nodes/attachments. The current scalar sidecar admits no named metadata and carries its exact flat-workgroup attribute; broader metadata-bearing profiles remain unsupported. |
| `!llvm.dbg.cu`, `!llvm.module.flags`, `!llvm.ident`, DI nodes, `!dbg`, and `llvm.dbg.value` | Metadata types, nodes, attachments, and metadata operands are not covered. | **Missing** | This is a substantial upstream feature. Until complete, preserve the current late debug injection path or disable this exporter for debug-bearing modules; never silently strip required source/coverage provenance. |
| Module inline assembly identity section | No module-inline-assembly property. fe2o3 currently emits one for identity material ([source debug](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/rustc-codegen-fe2o3/src/source_debug.rs#L1332-L1420)). | **Missing** | Upstream a generic module-assembly property, or retain deterministic late injection in fe2o3 and inspect the resulting section. |
| AMDHSA kernel descriptors and MsgPack metadata in the final ELF | These are backend/linker outputs, not LLVM dialect metadata. `pliron-llvm` has no object reader or AMDGPU metadata validator. | **Missing** as finalizer capability | Keep generation in LLVM/LLD and structural plus semantic inspection in the finalizer worker. Do not duplicate the backend format in the dialect. |

## Target machine, linking, and LLD

The optional `llvm-sys` feature targets LLVM 22 and contains wrappers named
`core`, `lljit`, and `target`; there is no LLD dependency
([manifest](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/Cargo.toml),
[module list](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/llvm_sys/mod.rs#L1-L42)).
This describes audited upstream capability, not an admitted fe2o3 route. The
feature remains disabled everywhere in production, including the worker.

| finalizer capability | v0.17.0 coverage | Status | Ownership |
| --- | --- | --- | --- |
| Parse/print/verify LLVM IR and read/write bitcode | Core LLVM-C wrappers provide these operations. | **Supported** upstream | Not used by fe2o3. The canonical V2 serializer emits bounded LLVM assembly, and the measured upstream LLVM 22.1.8 worker parses and verifies those exact bytes directly. |
| Initialize LLVM targets | `target.rs` can initialize all targets or the native target ([wrapper](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/llvm_sys/target.rs#L15-L110)). | **Supported** upstream, but insufficient | Not used by fe2o3. Initialization and target policy stay in the measured worker. |
| Lookup AMDGPU target and create a gfx942 target machine | No wrapper for lookup/configuration of triple, CPU, features, relocation model, code model, or optimization level. | **Missing** | Keep in fe2o3 worker. A generic target-machine wrapper could be upstreamed, but it would not replace policy validation. |
| Derive/check target data layout | No target-machine data-layout API in the wrapper. | **Missing** | Keep worker comparison against the exact expected input layout. |
| Link measured OCML/device bitcode | No LLVM module-linking/device-library provider capability. | **Missing** | Keep digest-pinned provider selection, ABI checks, and `LinkOnlyNeeded` in the worker. |
| Run the required LLVM pass pipeline | `pliron-llvm` exposes Pliron rewrite passes, not LLVM PassBuilder target pipelines ([library](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/lib.rs#L34-L94)). | **Missing** | Keep O0-O3 policy and PassBuilder execution in the worker. |
| Emit a relocatable AMDGPU object | No target-machine emission API. | **Missing** | Keep worker emission and immediate relocatable ELF inspection. |
| Link HSACO with in-process LLD | No LLD dependency, wrapper, or link policy. LLJIT is native JIT support and is not an HSACO linker. | **Missing** | Keep exact in-process `lld::lldMain` invocation and reusable-driver check in the worker ([LLD policy](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/include/WorkerLldPolicy.h#L1-L14), [link invocation](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/src/WorkerPipeline.cpp#L5542-L5591)). |
| Inspect symbols, ELF flags, kernel descriptors, resources, and AMDHSA MsgPack | No object/ELF/AMDGPU metadata inspection layer. | **Missing** | Keep worker checks for exact exports, no undefined symbols, gfx942/COV identity, kernarg/LDS/private sizes, wavefront size, register/spill limits, and dynamic stack/call closure. |

The worker already binds against an explicitly selected matching LLVM and LLD,
requires the AMDGPU target, records build identities, and links `lldELF` and
`lldCommon`
([CMake contract](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/CMakeLists.txt#L7-L68),
[link libraries](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/CMakeLists.txt#L161-L250)).
Moving those duties into `pliron-llvm` would create another security-critical
implementation without eliminating the existing worker's inspection duties.

## Fail-closed assessment

| boundary | Observed behavior | Assessment and requirement |
| --- | --- | --- |
| Pliron operation verification | Operations generally have verifiers, but coverage is per operation. `FuncOp::verify` returns `Ok(())` unconditionally ([function verifier](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L4679-L4682)). | **Not a complete module/ABI gate.** Add a fe2o3 whole-module verifier for symbol roles, ABI, call graph, target properties, metadata, memory effects, scopes, and permitted operations. |
| Dialect-to-LLVM export | Unknown/unconvertible operations can return `ToLLVMErr`, including multi-index aggregate errors ([error type](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L153-L179)). The implementation also uses `expect` for malformed/missing state. | **Not an admitted production boundary.** Do not call this converter in the producer or worker. Translate the reviewed live graph into canonical V2, then use the bounded fe2o3 serializer. |
| LLVM-to-dialect import | Unsupported valid LLVM types, opcodes, constant expressions, and values reach `todo!`; other paths use `expect` ([types](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L301-L309), [opcodes](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L1181-L1186), [values](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L852-L864)). | **Not an admitted production boundary.** Do not import through this converter in the producer or worker. The worker parses only the bounded fe2o3 assembly with measured upstream LLVM. |
| Semantic round trip | Target triple/layout, attributes, calling conventions, and metadata are omitted. Named sync scopes and inline-asm convergence can be changed on import/export. | **Unsafe silent loss.** The production path does not round-trip through the upstream converter. The selected profile must carry every missing property in canonical fe2o3 data and reject graph/sidecar disagreement before serialization. |
| LLVM FFI safety | The wrapper states that it is not a fully memory-safe interface and does not manage all value lifetimes ([module warning](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/llvm_sys/mod.rs#L4-L38)). | **Excluded from production.** Process isolation does not make this converter an admitted route; keep the optional feature disabled in every production component. |
| Target and link policy | `pliron-llvm` has no target-plan binding, device-provider digest, closed symbol policy, resource limit, object inspection, or LLD invocation. | **No finalizer fail closure.** Preserve the worker's bounded request, stage-coded errors, measured identities, exact profiles, repeated verification, and output-only-after-inspection behavior. |

The existing worker validates bounded requests, target/profile identities, module
contracts, device-library ABI, the closed symbol graph, LLVM verification,
relocatable output, LLD success, and the linked ELF before returning bytes
([request validation](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/src/WorkerPipeline.cpp#L1954-L2020),
[target contract](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/src/WorkerPipeline.cpp#L2062-L2321),
[output inspection](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/tools/fe2o3-llvm-link-worker/src/WorkerPipeline.cpp#L5105-L5395)).
That is the minimum security envelope for any future Pliron-backed route.

## Extension ownership

### Recommend upstream in `pliron-llvm`

These changes are target-independent LLVM fidelity or robustness improvements:

1. Replace all importer/exporter `todo!`, panic, and reachable `expect` paths
   with structured errors that identify the unsupported type, value, opcode, or
   missing property.
2. Add lossless function, return, parameter, and call-site attribute sets,
   calling conventions, global properties, and call/inline-asm/instruction
   flags.
3. Add target triple and data-layout module properties, generic module flags,
   metadata nodes, metadata-as-value, named metadata, and metadata attachments.
4. Round-trip arbitrary named synchronization scopes and every modeled
   inline-assembly semantic property.
5. Fill native bfloat and multi-index aggregate conversion gaps.
6. Expose a supported extension hook for downstream operation/type conversion.
   The current `ToLLVMValue` trait is private to `to_llvm_ir.rs`
   ([trait](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L297-L305)),
   so an external AMDGPU dialect cannot simply plug custom export behavior into
   `convert_module`.

A generic target-machine wrapper could also be upstreamed, but it is not on the
critical path for fe2o3 adoption and must not absorb fe2o3 target policy,
measured-provider selection, LLD policy, or output inspection.

### Keep minimal and fe2o3-specific

The narrow downstream layer should contain only facts whose meaning belongs to
the closed gfx942 contract:

1. A verified `amdgcn.*` vocabulary or equivalent typed properties for exact
   kernel entry roles, target plan, workgroup shape, ABI/effects, resource
   expectations, permitted scopes, and approved intrinsic/inline-asm forms.
2. A whole-module V2 construction and serialization gate that requires every
   target, ABI, attribute, metadata, symbol, memory, FP, and convergence
   property. Missing information is an error; there are no inferred
   security-relevant defaults.
3. A deterministic extraction from the reviewed live `llvm.*` graph to
   canonical V2, with exact sidecar equality checks for properties absent from
   the dialect, followed by bounded fe2o3 LLVM-assembly serialization.
4. A request to the existing isolated worker containing the exact serialized
   bytes and existing measured target/device/output policy. The worker parses
   with upstream LLVM 22.1.8 and remains solely responsible for LLVM linking,
   passes, target-machine emission, LLD, and final inspection.

Do not add fe2o3-specific target-machine or LLD wrappers to `pliron-llvm`, do not
accept arbitrary LLVM attributes/metadata as opaque strings, and do not make
the Pliron verifier the only authority for the resulting machine artifact.

## Generalization gate

The exact scalar fixture profile has passed finalization and one measured
MI300X execution. A general Pliron-backed gfx942 path should remain disabled
until all of the following hold:

- every **Extension required** or **Missing** property used by the selected
  profile is represented in canonical fe2o3 data or rejected before V2
  construction; no unchecked default may supply target, ABI, metadata,
  evidence, or machine-resource facts;
- live-graph extraction and sidecar validation return bounded structured
  errors for malformed, unsupported, stale, substituted, or inconsistent
  inputs;
- golden and hostile V2 serializer tests demonstrate exact preservation of the
  selected calling convention, target attributes, parameter attributes, FP
  flags, module metadata, source identity, and evidence;
- the optional `pliron-llvm` converter remains disabled in the producer and
  worker;
- each additional profile passes the same closed LLVM, symbol, ELF, AMDHSA
  metadata, resource, spill/stack/call, and code-object checks; and
- each additional runtime profile binds its descriptor and observed ROCr ABI,
  exact runtime image, device, dispatch, result, canary, wait, and unload facts
  through a separately reviewed one-shot consumer.

Until that gate is met, the closure remains one backend fixture and one exact
runtime lane. It must not be described as CUDA-Oxide parity, general memory
safety, race freedom, or replacement of the direct LLVM/finalizer security
boundary.
