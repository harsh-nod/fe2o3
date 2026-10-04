# Source-fed guarded output ABI experiment

Qualified diagnostic experiment: see the [exact source/CPU/native compilation
observations](../../../../docs/evidence/source-body-abi-20260920.md). This is not
native functional or GPU qualification. This standalone test does not change production worker
registration, protocol, admission, the finalizer, the device API or any schema.

The companion diagnostic_source_body_gfx942 example borrows a freshly decoded
VerifiedCanonicalKernelIrModuleV17, checks every operation/block/dataflow edge
in the supported complete source shape, then emits one LLVM kernel Function.
The ordinary LLVM function owns argument loading, system inputs and global-index
calculation. One constrained side-effecting unit owns the compute, bounds mask,
address carry, one global store, wait, EXEC restoration and termination.
It is not a naked whole-body contract or a general user-chosen memory program.

Profile: gfx942:xnack-, Wave64, required64x1x1, dynamic D1 launch, writable
global DisjointSlice<u32> plus three u32 values, one authored ordered program
and one index<len guarded store of its exact result. Every unsupported operation,
helper, branch, extra memory effect, different ABI and uncovered block refuses.
Authored registers must avoid v0..v7. Pointer-low uses s16, pointer-high uses
v4, length halves use s18:s19, global index uses v0:v1, address uses v2:v3,
and saved EXEC uses s20:s21. The vector pointer-high avoids the gfx942 carry
instruction's scalar constant-bus restriction when VCC is also an input.
All net VGPR/scratch/VCC/SCC/memory clobbers are declared. EXEC is preserved
by the exact unconditional save/restore sequence inside the single opaque
assembly unit, not declared as a reserved-register clobber. This reasoning
depends on restoration even when the narrowed mask is zero, not merely the
absence of a returning LLVM successor. The constraint operands are
actual LLVM dataflow from the original arguments and system intrinsics.
These are diagnostic compiler choices, not new caller-authored ABI promises.
The store uses `global_store_dword ..., off` for the verified global-address-space
pointer, not a generic flat access. The unchanged machine-effect worker rejects
FLAT opcodes; that historical failure remains retained. The standalone native
test checks the exact eight store bytes, including zero offset/cache modifiers.

The source fixture leaf is applied via apply_patch to an isolated source-only
copy of the existing production-extraction-device fixture's
src/ordered_program_v32.rs. Keep its existing manifest and exact dependency
closure; use its existing ordered-program export feature and normal exporter.
No separate simulator implementation is introduced. The CPU execution uses the
existing simulator of that exact source-produced V17 owner.

Native test only accepts the literal three-operation source fixture plan
(v32/v33/v34/v35/v36). It reuses the unchanged native test helpers by including
OrderedProgramPrototypeTests.cpp with its main renamed, never executing that
synthetic main. It independently checks typed LLVM parameter/dataflow shape,
seventeen clobber/effect/ABI/metadata/poison/operand-bundle/calling-convention substitutions, original literal arithmetic encodings,
the complete entry bytes and decoded compute/store/mask/wait/termination path.
Both O0 and O3 must preserve the tail while reporting actual compiler prologues.
The 256-byte hidden kernarg area is not disabled: expected28 explicit bytes,
aligned to32, plus256 hidden, total288. LDS/private/dynamic stack must be absent.
Pinned module layout, external/default entry linkage and the exact required
workgroup metadata are checked; extra instruction/function/module metadata,
unreviewed attributes and poison-generating flags are refused.
Native opcode spellings and descriptor assumptions are checked against the
pinned actual O0/O3 builds; the initial failures remain recorded, not relabelled.

Qualification sequence (primary/integrator owns all execution):

1. Review/format and build the new Rust example on the primary checkout only;
   run its fifteen structural-control tests with retained logs.
2. Create a fresh source-only checkout/device target under the charged cache;
   apply the source fixture via apply_patch, then export through the existing
   normal fe2o3-export-sim diagnostic path to a new V17 file.
3. Retain source/lock/exporter identity and the actual typed V17/source IDs.
   Inspect the same file with the existing inspector and simulate it normally.
4. Capture new-example stdout to a new LLVM file and stderr identity observation.
   The raw file SHA and typed canonical identity are separate domains.
5. Configure this directory with the unchanged pinned LLVM/LLD/version/build-ID
   variables and FE2O3_WORKER_SOURCE; build source-body-abi-candidate with jobs2.
6. Run source-body-abi-candidate ABS_LLVM. Capture O0/O3 complete-entry report
   and recheck LLVM/source/helper/binary/library bytes before claiming success.

Independent functional oracle for this source is (a & c) | (b & ~c), reduced
to u32; do not use the program's own evaluate helper as the independent oracle.
Cover0,1,63,64,65,127,128,129 lengths with matching/multiple rounded workgroups;
check every active output, inactive/canary bytes and initializedness. Include
0xffffffff,0x80000000,alternating bits and [19,23,42] scalar triples.
The native report alone does NOT establish native output or hardware behavior.
External pointer allocation, alignment, address representability and valid
launch are premises. Tail structural/encoding checks are not hazard, race,
bounds, physical-lifetime or semantic-refinement proofs.

Resource supervision: existing40GiB disk/64GiB RAM floors;20GiB combined charged
cache/output cap; new native build/output <=256MiB; two jobs. Configure/build/
native request limits60/300/90seconds, bounded1MiB stdout/stderr capture (native
report64KiB maximum). These are supervision/accounting limits, not OS quotas.
No cleanup of peer caches/processes, GPU dispatch, public API promotion or
milestone closure is permitted by a passing diagnostic experiment.
