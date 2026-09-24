# Ordinary-shell physical entry ABI experiment

This is a standalone, test-only transport fixture. Its four O0/O3 cases passed
[recorded native qualification](../../../../docs/physical-entry-ordinary-fixture-qualification-20260924.md).
It is not a source API, authenticated source owner, canonical owner, general ISA
checker, protected artifact, hardware test or milestone acceptance.

The pinned naked-function route has a proven entry-SGPR accounting mismatch
(see the separate `physical-entry-abi-r1/INCOMPATIBILITY.md`). This fixture
deliberately uses an ordinary LLVM AMDGPU kernel Function. It admits **zero**
compiler-added entry or tail instructions: all six-slot loads, index setup,
select/copy, guard, store, waits, EXEC save/restore and end are explicitly in
one literal assembly body. The normal compiler still owns generation of the
hardware entry descriptor. Author control of instructions does not mean
permission to forge that descriptor.

## Contract

- gfx942:xnack-, COV6, Wave64, required workgroup [64,1,1], maximum grid [2,1,1].
- Six explicit slots: data pointer at0/8 bytes, length at8/8, a at16/4,
  b at20/4, c at24/4, selector at28/4. Names and global/by-value roles are checked.
- Normal worker COV6 hidden extent remains256, so total kernarg size288.
  Thirteen exact hidden rows remain at both O0/O3 because unused services are
  excluded by explicit supported absence attributes.
- Exact entry descriptor: kernarg s[0:1], workgroupX s2, localX v0;
  user SGPR count2; no dispatch/queue/dispatchID/scratch/preloads or Y/Z IDs.
- Copy means scalar a to each guarded output index. Select chooses a when
  selector==0, otherwise b. It is not a two-buffer copy.
- All formal LLVM arguments are unused; there are no LLVM input/output
  constraints, SSA arithmetic, intrinsics, helper functions or module assembly.
  Clobber-only constraints declare physical temporaries and special state.
- Exactly21 instructions/one native block for copy;25/four for select.
  Every decoded instruction, operand order/register/immediate, implicit
  special-state effect, branch target, CFG membership and full-entry byte extent
  must match the independent native rows.
- Descriptor/metadata resource capacities cover all decoded registers, with
  no spills, AGPRs, private/LDS allocation or dynamic stack.

Native qualification is intentionally strict. If the ordinary backend adds
formal-argument loads, moves, waits, setup or a tail, it fails; the fixture does
not search for a matching subsequence or ignore the extra instructions.
A passing entry relation still does not prove runtime pointer validity,
launch bounds, general hazard freedom, source refinement or GPU execution.

## Standalone build and run (root-owned only)

The normal existing worker must be supplied to CMake; this directory is not
added to the production CMake/test roster. Use the same pinned LLVM_DIR,
FE2O3_EXPECTED_LLVM_BUILD_ID and linker/package settings as the existing
complete-body ABI fixture, changing only this standalone source directory and
executable name.

```text
cmake -S tools/fe2o3-llvm-link-worker/tests/physical-entry-abi \
  -B ABS_FRESH_BUILD \
  -DFE2O3_WORKER_SOURCE=ABS_REPOSITORY/tools/fe2o3-llvm-link-worker \
  -DLLVM_DIR=ABS_PINNED_LLVM/lib/cmake/llvm \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=PINNED_BUILD_ID
cmake --build ABS_FRESH_BUILD --parallel 2

ABS_FRESH_BUILD/physical-entry-abi-candidate copy O0 ABS_FRESH_COPY_O0
ABS_FRESH_BUILD/physical-entry-abi-candidate copy O3 ABS_FRESH_COPY_O3
ABS_FRESH_BUILD/physical-entry-abi-candidate select O0 ABS_FRESH_SELECT_O0
ABS_FRESH_BUILD/physical-entry-abi-candidate select O3 ABS_FRESH_SELECT_O3
```

Every output directory must not exist. Each invocation exclusively creates
`input.ll` and `output.hsaco` before checking native equality, so a failure's
actual inputs/output remain inspectable. Successful observations go to stdout
with exact hashes, full trace/CFG, descriptor, argument metadata and refusals.
The executable uses the existing worker V2 execute/link/inspect/decode route
with clearly marked synthetic test identities. It does not dispatch a GPU.

## Controls and bounds

Each mode/optimization must reject wrong CPU and missing required launch
metadata through normal worker input validation. Each successful baseline is
then subjected to18 actual instruction-byte mutations (20 for select),12
actual descriptor mutations and2 exact metadata string mutations.

Instruction controls cover each relevant pointer/length/selector slot,
load/store waits, index inputs/shift, pointer high/carry, store data, mask restore,
termination, 64-bit scale, bounds comparison/length pair, EXEC masking and
select branch self/skipped-result targets. Descriptor controls
cover kernarg enable/base-shifting dispatch service, missing/extra user count,
missing workgroupX, extra workitemY, preload/scratch, insufficient register
capacities, kernarg extent and reserved bits.

Native mutations get fresh payload hashes and fresh existing decoder results.
Reports distinguish decoder refusal from relation refusal. Metadata mutations
are tested by a fresh bounded metadata parser, not relabelled decoder checks.
These malformed bytes are negative controls held in memory; none are emitted
as accepted outputs.

Bounds:16KiB LLVM,64KiB HSACO,16KiB metadata blob,128 sections,4096 symbol scans,
at most25 expected native instructions/four blocks, at most32 native
mutations,64KiB final report,90-second process alarm. Existing decoder limits
also apply. Only one baseline and one mutation payload are live at a time
besides bounded observations. Supervisor build/time/disk/closure controls
remain the root's responsibility; these checks are not OS quotas.

## Source-backed basis and limits

The exact pinned upstream revision is
`f58b06dce1f9c15707c5f808fd002e18c2accf7e`.
Its supported absence attrs are in AMDGPUAttributes.def/AMDGPUUsage;
GCNSubtarget.cpp selects kernarg/dispatch inputs; SIMachineFunctionInfo.cpp
always enables kernel X IDs; SIISelLowering.cpp assigns user then system SGPRs
and v0; AMDGPUAsmPrinter.cpp emits descriptor fields. Unused argument lowering
and frame elimination may still affect native output, hence full-entry testing.

The LLVM source rules provide a reason to try this bounded experiment, not a
guarantee that an ordinary Function with opaque physical reads is a reusable
source-authenticated production mechanism. Public ownership/schema decisions
and exact production provenance/ranked/formal/native joins remain separate.
No body-level author-ownership claim may be expanded to compiler ABI generation.

The fixed MC opcode/operand/effect expectations were cross-checked by reading
earlier genuine native observations; those historical observations are not new
passes for this fixture. New O0/O3 results must be recorded by the parent.
