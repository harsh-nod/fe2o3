# `pliron-llvm` v0.17.0 coverage for the gfx942 finalizer

Status: source audit at fe2o3 commit
[`2f7c4fd1dfef7b9056caab0880700e3da7eeef03`](https://github.com/powderluv/fe2o3/tree/2f7c4fd1dfef7b9056caab0880700e3da7eeef03)
and Pliron commit
[`2610651306ea3ba670f68d5d8b1e1159bcd521ed`](https://github.com/pliron-org/pliron/tree/2610651306ea3ba670f68d5d8b1e1159bcd521ed),
the commit published as `pliron`/`pliron-llvm` v0.17.0.

## Scope and conclusion

The root manifest pins the Pliron workspace commit, and the lockfile resolves
`pliron` v0.17.0 at that exact revision. `pliron-llvm` is in the pinned upstream
workspace but is deliberately not a fe2o3 dependency. The architecture records
that exclusion and assigns LLVM target-machine and in-process LLD work to the
isolated finalizer worker
([pin](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/Cargo.toml#L108),
[lock](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/Cargo.lock#L2093-L2095),
[architecture](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/docs/pliron-wave0-architecture.md#L28-L37)).
No repository-local `AGENTS.md` file exists at this commit.

The audited `pliron-llvm` is a useful target-neutral LLVM dialect and a partial
LLVM-C importer/exporter. It is **not** a drop-in representation or finalizer for
the fe2o3 gfx942 contract. It covers much of the ordinary SSA instruction set,
but it cannot preserve or emit several correctness-bearing AMDGPU ABI details:
calling conventions, function/parameter attributes, module target state,
metadata, strict constrained-FP metadata operands, and some memory-operation
flags. Its LLVM wrapper also has no target-machine object emission and no LLD
API. Those absences are architectural, not a short list of spelling changes.

The recommendation is:

1. Keep the existing isolated finalizer worker as the only authority for target
   setup, device-library linking, optimization, object emission, LLD, and HSACO
   inspection.
2. Upstream target-independent representation and conversion fixes to
   `pliron-llvm`.
3. Add only a bounded fe2o3 AMDGPU contract layer and a fail-closed export gate.
   Do not fork a second target machine or linker implementation into the Pliron
   path.

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
| Kernel calling convention `amdgpu_kernel` | `FuncOp` stores function type, linkage, and symbol name. Function conversion applies linkage but no calling convention ([function op](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/ops.rs#L4526-L4688), [export](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L2061-L2115)). | **Missing** | Upstream numeric/named LLVM calling conventions on functions and call sites. fe2o3 export must require `amdgpu_kernel` on entries and the expected convention on helpers/calls. |
| `nounwind`, `convergent`, memory effects, `speculatable`, `willreturn` | Not modeled or round-tripped as function attributes. fe2o3 emits these on kernel/intrinsic declarations ([current attributes](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/fe2o3-amdgcn-model/src/lowering.rs#L5831-L5856)). | **Missing** | Upstream a lossless general attribute-set representation. The fe2o3 gate supplies and validates a closed required set rather than trusting defaults. |
| `target-cpu`, `target-features` | No function string-attribute representation or conversion. | **Missing** | Upstream general string attributes; fe2o3 sets exact gfx942, sramecc, and xnack values from the target plan and rejects disagreement. |
| `amdgpu-flat-work-group-size` and implicit-argument attributes | No representation or conversion. | **Missing** | Implement as general string attributes upstream where possible; keep permitted values and code-object-version rules in fe2o3. |
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
| Target triple `amdgcn-amd-amdhsa` | Not represented or converted. | **Missing** | A general module target property is upstreamable. fe2o3 must set and validate the exact triple at its export/worker boundary. |
| gfx942 data layout | Not represented or converted. | **Missing** | Upstream a module data-layout property. The worker remains authoritative and rejects an input layout that disagrees with the target machine. |
| `amdhsa_code_object_version`, sramecc, and xnack module flags | No module-flag model. | **Missing** | Upstream general module flags if lossless behavior can be defined; keep profile selection and exact values in fe2o3. |
| `!reqd_work_group_size` and kernel argument metadata | No instruction/function metadata attachment or named metadata model. | **Missing** | Upstream generic metadata nodes/attachments. A small fe2o3 layer defines the required schema and cross-checks it against ABI facts. |
| `!llvm.dbg.cu`, `!llvm.module.flags`, `!llvm.ident`, DI nodes, `!dbg`, and `llvm.dbg.value` | Metadata types, nodes, attachments, and metadata operands are not covered. | **Missing** | This is a substantial upstream feature. Until complete, preserve the current late debug injection path or disable this exporter for debug-bearing modules; never silently strip required source/coverage provenance. |
| Module inline assembly identity section | No module-inline-assembly property. fe2o3 currently emits one for identity material ([source debug](https://github.com/powderluv/fe2o3/blob/2f7c4fd1dfef7b9056caab0880700e3da7eeef03/crates/rustc-codegen-fe2o3/src/source_debug.rs#L1332-L1420)). | **Missing** | Upstream a generic module-assembly property, or retain deterministic late injection in fe2o3 and inspect the resulting section. |
| AMDHSA kernel descriptors and MsgPack metadata in the final ELF | These are backend/linker outputs, not LLVM dialect metadata. `pliron-llvm` has no object reader or AMDGPU metadata validator. | **Missing** as finalizer capability | Keep generation in LLVM/LLD and structural plus semantic inspection in the finalizer worker. Do not duplicate the backend format in the dialect. |

## Target machine, linking, and LLD

The optional `llvm-sys` feature targets LLVM 22 and contains wrappers named
`core`, `lljit`, and `target`; there is no LLD dependency
([manifest](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/Cargo.toml),
[module list](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/llvm_sys/mod.rs#L1-L42)).

| finalizer capability | v0.17.0 coverage | Status | Ownership |
| --- | --- | --- | --- |
| Parse/print/verify LLVM IR and read/write bitcode | Core LLVM-C wrappers provide these operations. | **Supported** | Useful at an IR boundary, subject to the importer caveats below. |
| Initialize LLVM targets | `target.rs` can initialize all targets or the native target ([wrapper](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/llvm_sys/target.rs#L15-L110)). | **Supported**, but insufficient | Initialization alone does not establish a target contract. |
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
| Dialect-to-LLVM export | Unknown/unconvertible operations can return `ToLLVMErr`, including multi-index aggregate errors ([error type](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/to_llvm_ir.rs#L153-L179)). The implementation also uses `expect` for malformed/missing state. | **Partially fail-closed, not robust against untrusted IR.** Replace reachable panics with structured errors and verify before crossing FFI. Run conversion in the isolated worker until this is demonstrated panic-free. |
| LLVM-to-dialect import | Unsupported valid LLVM types, opcodes, constant expressions, and values reach `todo!`; other paths use `expect` ([types](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L301-L309), [opcodes](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L1181-L1186), [values](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/from_llvm_ir.rs#L852-L864)). | **Not fail-closed in-process.** A controlled process abort is containment only if import runs in the worker. Prefer an allowlist preflight plus typed `Unsupported*` errors for every switch default. |
| Semantic round trip | Target triple/layout, attributes, calling conventions, and metadata are omitted. Named sync scopes and inline-asm convergence can be changed on import/export. | **Unsafe silent loss.** These are harder failures than an explicit unsupported error. Export must compare a required-property manifest before and after LLVM construction and reject any omission or change. |
| LLVM FFI safety | The wrapper states that it is not a fully memory-safe interface and does not manage all value lifetimes ([module warning](https://github.com/pliron-org/pliron/blob/2610651306ea3ba670f68d5d8b1e1159bcd521ed/pliron-llvm/src/llvm_sys/mod.rs#L4-L38)). | **Isolation remains required.** Do not move parsing/conversion of untrusted modules into the trusted producer process merely because the API is Rust. |
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
2. A whole-module export gate that requires every target, ABI, attribute,
   metadata, symbol, memory, FP, and convergence property. Missing information
   is an error; there are no inferred security-relevant defaults.
3. A deterministic lowering from that bounded contract to upstream `llvm.*`,
   followed by LLVM verification and a post-export manifest comparison.
4. A request to the existing isolated worker containing the canonical LLVM
   text/bitcode and the existing measured target/device/output policy. The
   worker remains solely responsible for LLVM linking, passes, target-machine
   emission, LLD, and final inspection.

Do not add fe2o3-specific target-machine or LLD wrappers to `pliron-llvm`, do not
accept arbitrary LLVM attributes/metadata as opaque strings, and do not make
the Pliron verifier the only authority for the resulting machine artifact.

## Adoption gate

A production Pliron-backed gfx942 path should remain disabled until all of the
following hold:

- every **Extension required** or **Missing** row used by the selected profile
  has either landed or is rejected before conversion;
- malformed and valid-but-unsupported inputs return bounded structured errors,
  with no reachable panic across importer/exporter tests and fuzzing;
- golden modules demonstrate exact preservation of calling conventions,
  target attributes, parameter attributes, named scopes, volatile/inbounds and
  FP flags, kernel metadata, strict-FP metadata operands, and debug provenance;
- canonical output from the direct and Pliron-backed lowerings is compared at
  the LLVM contract level, with expected normalization explicitly documented;
- both routes enter the same measured finalizer worker and pass the same symbol,
  ELF, AMDHSA metadata, resource, spill/stack/call, and code-object-version
  checks; and
- no target identity, ABI property, metadata field, or machine-resource fact is
  supplied by an unchecked default.

Until that gate is met, `pliron-llvm` can be evaluated as a pre-finalization IR
tool, but it must not replace the current direct LLVM/finalizer security
boundary.
