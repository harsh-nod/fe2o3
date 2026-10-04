# Actual helper-source native observation (test only)

This is a bounded observation fixture, not a completed native semantic matcher.
It feeds unchanged canonical LLVM from the generated public helper-source ladder
to the ordinary pinned LLVM/object/LLD worker. It does not create LLVM, rename
symbols, force physical registers, patch linked output, use another assembler,
open KFD/HSA, launch a GPU kernel, or confer protected publication authority.

The first admitted live-source shapes are one or two specialized **VOR** helpers:
default256, edited512, repeat, and two. The separate inert six-operation source
and native-template tests are not six live-source/native qualifications.

## Prerequisite and invocation

Root must first integrate and qualify the helper-source continuation and generated
source ladder through r3. This fixture requires the r3 observation fields in the
exact successful ladder observation.json. No successful source or native output
existed when this private fixture was authored; refusal is an expected and useful
result, never a reason to rewrite source or relax a relation silently.

Build this directory using the same reviewed standalone CMake procedure and
pinned LLVM/worker inputs as the neighboring source-body-abi test. Supply the
absolute existing worker source as FE2O3_WORKER_SOURCE and the reviewed
FE2O3_EXPECTED_LLVM_BUILD_ID. Root owns all builds and native runs.

The executable arguments are:

    helper-source-native-observer ABS_LADDER_JSON SHA256 LABEL O0|O3 ABS_FRESH_OUTPUT_DIR

LABEL is default256, edited512, repeat, or two. The output parent must already
exist; the final directory must not exist. Run each label/optimization in a
separate fresh directory. Pin the executable, SDK, worker source/build ID,
ladder record, source closure and output outside this test according to the
existing root qualification procedure. A user-supplied SHA is an identity join,
not compiler-origin authentication.

## What is checked

The reader pins the entire ladder record by explicit SHA256, selects one exact
descriptor observation and the matching LLVM/handoff child observations, and
checks their schema, inert-authority flags, label and byte-domain joins. The
same source invocation must occur in both child records. It hashes the unchanged
canonical LLVM, handoff, descriptor bytes, actual Rust source, Cargo manifest and
lockfile, and rereads every pin after the native observation. Paths are bounded
canonical absolute regular files without symlink leaves; reads check stable
inode/size/mtime/ctime plus exact EOF. These checks detect substitutions in this
controlled test. They are not general filesystem-race exclusion or live
source-owner custody. The Rust ladder remains responsible for the typed
handoff/descriptor/source-symbol relation; this C++ fixture does not re-decode
the Rust binary formats or authenticate the observation JSON.

LLVM parsing and verification enforce the exact four-component ptr/len/a/b kernel
signature, required [64,1,1] workgroup, exact defined helper roster, internal
one-i32-argument/one-i32-result helper signatures, direct root/helper calls, one
side-effecting =v,v,v VOR operation per helper, actual helper argument and 256/512
constant operands, and return of that actual VOR result. Constants and call
census are joined to the selected one/two-helper variant. The structural check
does not re-prove the root expression, index arithmetic, guard or store.

The existing worker request construction, execute route, exact response and
LLVM/object/LLD derivation joins, and post-link target/export/unresolved/metadata/
launch checks are reused unchanged. Request/executable identity placeholders are
the same explicitly synthetic native-test markers used by the existing tests.
They are not executable measurement, source custody, or finalizer admission.

The new analyzer request admits at most the root plus the exact one/two retained
helper names, with global-address/read/write/return/direct-call budgets
16/8/4/3/2. Only root-to-helper direct calls are allowed. Every decoded instruction
encoding is joined to the linked bytes and every decoded call target to an exact
retained helper entry. Original complete-body, source-body and physical-entry
fixtures are not modified or widened. Unknown clones, extra functions, indirect
calls, unsupported instructions or exceeded budgets refuse, with no invented
success expectation.

## What the observation deliberately does not prove

Rust #[inline(never)] is not currently propagated to ordinary helper LLVM as a
noinline attribute by the model emitter. O0 may retain calls; O3 may inline them.
This observer records the actual LLVM noinline state and decoded call/return/VOR
sites. It neither requires retained calls nor calls inlining an ABI regression.
Compiler-chosen helper registers and any normal prologue remain compiler-owned.

A passing observation is not a native proof of argument/result physical-register
flow, preservation across helper calls, the complete one/two-source expression,
bounds guard/store safety, memory safety, race freedom, or functional refinement.
All corresponding qualification, hardware, authority and completion flags are
false. A V_OR opcode count alone is expressly not a semantic match. Full function,
block, operand, effect and branch traces are retained for the next evidence-based
matcher. No CPU or GPU functional execution is performed by this observer.

## Bounds, output and refusal

The record is at most 4 MiB, LLVM 1 MiB, handoff 2 MiB, descriptor 64 KiB,
source and Cargo manifest 64 KiB each, and lockfile 1 MiB; at most 12 file pins
are allowed (the present shape uses seven). Each size bound applies before its
allocation. Selected LLVM is bounded to 16 functions, 64 blocks and 512
instructions after the pinned LLVM parser/verification; those library calls keep
their existing behavior and are not a new adversarial-parser resource proof.
Worker output is at most 1 MiB. The existing analyzer's global allocation/work
limits still apply; this fixture further bounds the result to three functions,
64 blocks/effects, 512 instructions and 16 operands/implicit-register rows per
instruction. The final JSON is at most 1 MiB.

output.hsaco is saved exclusively after response/derivation checks and before
post-link inspection/physical decoding. A later refusal leaves those exact bytes
for diagnosis but no observation.json success report. Outputs are mode 0600 in
a fresh 0700 directory and never overwrite existing files. There is no cleanup
deletion. Process exit closes descriptors on errors. The external supervisor
must bound wall time, memory and logs; this in-process observer is not a sandbox
or timeout controller.

## Root qualification and next implementation

First run strict build/lint, then unchanged default256 and edited512 at O0/O3,
followed by repeat and two. Compare exact repeated inputs and outputs without
claiming nondeterminism a semantic defect. Retain failures and native output
rather than guessing a passing instruction shape. Negative controls should
cover wrong record SHA, changed source/LLVM/handoff bytes, substituted entry or
helper roster, unknown label, changed helper constant/return/call ABI, and any
native trace/encoding/call-target substitution in the eventual matcher.

After reviewing genuine native outputs, add the smallest independent matcher
for scalar argument materialization, VOR operands/result, retained-call return
and register preservation where present, inlined value flow where applicable,
and the root expression/guard/store. Mutate each safety-relevant relation and
retain separate O0/O3 records. If the analyzer or normal source ladder refuses,
report that concrete unsupported seam before extending it. Until that work is
implemented and tested, native_semantics_qualified and physical_helper_abi_qualified
must remain false.
