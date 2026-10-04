# Complete-body transport experiment

Private, production-free native test; not a kernel-authoring API or M0/M2 closure.

A real LLVM AMDGPU kernel Function owns target, workgroup and ABI metadata.
One side-effecting inline-assembly call contains all seven authored instructions,
including s_endpgm, followed by LLVM unreachable. Every touched VGPR is declared
as a clobber. The existing worker emits and links it, checks exports/metadata,
then independently decodes the final entire entry.

Both naked and ordinary shells must preserve exactly28 bytes at O0 and O3.
The literal expected encoding is fixed in the source before compilation.
Missing workgroup metadata and wrong CPU must be refused before code generation.
No assembly-only export, dummy replacement function, external kernel object,
manual descriptor, protocol allocation, production owner or finalizer bypass
is introduced.

This standalone CMake entrypoint intentionally does not change the production
worker CMake test roster or its build claim. It links the existing worker libraries
unchanged and the existing OrderedProgramWorkerSupport.inc helper. The explicit
FE2O3_WORKER_SOURCE must name tools/fe2o3-llvm-link-worker in the same checkout.

Use the project's reviewed LLVM/LLD package, matching version/build-ID file and
existing resource supervision. Configure this directory with those mandatory
worker CMake variables and FE2O3_WORKER_SOURCE, then build target
whole-body-transport-candidate with at most two jobs. Run the executable twice:

    whole-body-transport-candidate naked
    whole-body-transport-candidate ordinary

Separate runs are not fallbacks. Retain both outcomes and failed logs. Inputs,
actual binary/static dependencies, package and emitted report bytes must be
measured; a build claim or JSON report is not executable authentication.

Required bounded test supervision: at least40GiB persistent free disk and64GiB
RAM; at most20GiB combined task cache/output and512MiB new build/output.
Configure/build/native termination-request limits60/300/60 seconds, logs1MiB
per stream, native report32KiB. These are supervision/accounting bounds, not
OS-enforced quotas or guaranteed child drain/reap deadlines.

The initial task-owned version passed both shells on mi350-2 at clean compiler
f798cf93d0eff1c7e951488b489e959bae6c2e3e. Whole entry bytes were identical at
both optimization levels; descriptors had capacity/boundary40 and hidden kernarg
storage256 bytes, not an empty ABI. Ordinary descriptor fields changed with
optimization. The repository copy requires its own fresh qualification; do not
relabel that historical receipt.

The fixture initializes every arithmetic input but has no observable output
other than wave termination. It proves only this bounded transport/encoding
mechanism: no useful argument ABI, functional algorithm result, source semantics,
arbitrary helpers/branches, memory/synchronization, register lifetime, hardware
execution or protected launch. See docs/whole-body-transport-qualification-v1.md
for the qualified revision and exact evidence boundaries.
