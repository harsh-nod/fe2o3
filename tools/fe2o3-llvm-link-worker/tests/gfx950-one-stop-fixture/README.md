# CPU/static gfx950 one-stop fixture

This is an inert test fixture, not a dispatch command. It runs the unchanged
ordinary LLVM/LLD worker on a fixed one-kernel LLVM module and independently
checks the resulting ELF, descriptor, complete ABI and entire native entry.
It never opens KFD, creates a queue, starts GDB, samples registers, loads the
code object into a runtime, or reconstructs source/protected authority.

The fixed source has one output pointer, WG/grid64, no LDS/private/AGPR/spills,
two scalar and two vector sentinels, full-width lane address calculation, one
store, full waits, exactly one s_trap3 and s_endpgm. The expected entry is16
instructions/84 bytes. Separately, pinned LLVM emits1068 bytes of S_NOP prefetch
padding; the checker requires that exact1152-byte executable section, not an
arbitrary tail. Expected COV6 kernarg264B/align8 includes all13 actual hidden
metadata rows, not only the8-byte explicit pointer.

No property above is an observed hardware result. Trap offset76/posttrap80 are
static. Source suggests future AMD-dbgapi WAVE_INFO_PC=entry+80 for DEBUG_TRAP;
root must review/allocate the exact native expectation before any future gate.
The report's observed PC is null and native expectation authorization is false.
An unexpected future PC must refuse; no either/or acceptance is proposed.

The optional future output page is4096B, logical272B: eight leading canary bytes,
256-byte payload, eight trailing canary bytes. The fixture only records expected
values; it allocates no runtime page and does not claim host visibility from
s_waitcnt (VM stores have completed toL2). Same-client attachment, runtime ACK,
TTMP/CWSR, no sampling, owned packet/current stop and actual completion remain
separate gates. Existing all-next-MI invalidation and all old routes are unchanged.

## Root-only qualification commands

Use the existing reviewed SDK/build-ID setup in [MEASUREMENT.md](../../MEASUREMENT.md).
Do not invent a build identity or install anything. Under root's bounded outer
supervisor, substitute exact reviewed absolute paths:

~~~sh
cmake -S tools/fe2o3-llvm-link-worker/tests/gfx950-one-stop-fixture \
  -B /absolute/fresh-one-stop-build \
  -DCMAKE_BUILD_TYPE=Release \
  -DFE2O3_WORKER_SOURCE=/absolute/fe2o3/tools/fe2o3-llvm-link-worker \
  -DLLVM_DIR=/absolute/reviewed-sdk/lib/cmake/llvm \
  -DLLD_DIR=/absolute/reviewed-sdk/lib/cmake/lld \
  -DFE2O3_PINNED_LLVM_VERSION=22.0.0git \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=REVIEWED_BUILD_ID \
  -DFE2O3_LLVM_BUILD_ID_FILE=/absolute/reviewed-build-id.txt
cmake --build /absolute/fresh-one-stop-build --target gfx950-one-stop-fixture --parallel 2
node --test tools/fe2o3-llvm-link-worker/tests/gfx950-one-stop-fixture/report.test.mjs
/absolute/fresh-one-stop-build/gfx950-one-stop-fixture O0 /absolute/fresh-one-stop-o0
node tools/fe2o3-llvm-link-worker/tests/gfx950-one-stop-fixture/check-report.mjs /absolute/fresh-one-stop-o0
/absolute/fresh-one-stop-build/gfx950-one-stop-fixture O3 /absolute/fresh-one-stop-o3
node tools/fe2o3-llvm-link-worker/tests/gfx950-one-stop-fixture/check-report.mjs /absolute/fresh-one-stop-o3
~~~

Both output directories must not exist. Each successful fixture retains input.ll,
output.hsaco and report.json. A report is NOT qualification without process exit0
and the completed root supervisor receipt; failed/partial outputs remain historical
and must not be consumed or retried in place. The standalone JSON checker verifies
report fields/roster and retained file hashes only; it does not validate a root
receipt or grant authority.

Expected, UNRUN at authorship:13 Node pure tests; per O0/O3 run one positive,
two phase-exact ordinary-worker input refusals and83 independently named actual
artifact mutations. Across both runs that is two positives, four source-input
refusals and166 artifact-mutation refusals. Source-input negatives are fixed
synthetic LLVM inputs, not Rust source-custody tests.

The whole entry is decoded by the actual gfx950 MC subtarget. Exact opcode,
operand order/values, definitions, implicit uses/defs, effect flags and every
byte must match; supported rows re-encode byte-identically with zero fixups.
The sole debugtrap is independently classified by opcode/id/encoding:
the pinned real SOPP table does not propagate its pseudo's isTrap flag.
The checker owns all JSON strings; it never retains a dangling LLVM StringRef.

Bounds: fixed LLVM<=16KiB; object/HSACO<=1MiB each; entry<=64rows/512B
(exact16/84 required); ELF<=128sections/4096symbols; metadata<=16KiB,
14exact arguments; report<=256KiB;83mutations one at a time. An original90s
steady-clock deadline is checked after work and final output; alarm90 is only
a fallback. Root must impose independent wall/CPU/RSS/log/process containment.
Exclusive no-follow finite writes can leave partial files on failure; there is
no overwrite/retry loop. These are logical bounds, not a proved LLVM RSS bound.
