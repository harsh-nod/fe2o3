# Inspect a bounded repeated instruction program after native compilation

This developer workflow connects an existing ordinary-source/CPU capture and
its ordinary LLVM observation to **fresh O0/O3 native compilation and final
HSACO inspection**. It is not GPU execution, production artifact admission,
proof, a physical-register lifetime analysis or general M2 completion.
This guide describes the implementation and commands. The
[dated qualification](evidence/authoring-repeat-native-20260923.md) records actual
passes, exact inputs and limitations.

The separate `ordered-repeat-source-candidate` executable accepts only the
three measured repeat counts 1, 2 and 15. This does not narrow the source macro's
existing literal grammar or silently qualify other counts.

## Start from real, unchanged source and LLVM captures

First follow [normal-source acceptance](ordered-repeat-source-acceptance-v1.md),
then [ordinary LLVM observation](ordered-repeat-llvm-observation-v1.md).
The original CPU/LLVM qualification remains its own
[dated evidence](evidence/authoring-repeat-fault-20260922.md#final-publication-qualification);
it is not a repeat-native result.

The native driver consumes two independently pinned successful `receipt.json`
files and all their original selected files:

- Source: four exports, four inspections, 120 complete independent CPU results
  and eight genuine frontend refusals, in 136 recorded stages.
- LLVM: four actual lowerer calls and complete emitted LLVM files for
  `one`, `two`, `fifteen` and a fresh identical-source `repeat`.
- The source capture's actual semantic MIR, retained inventory/preflight and
  canonical/raw KIR identities; none is guessed from an LLVM hash.
  Inventory describes the root/contract census, not an instruction-body hash.

The native driver revalidates those retained raw results through their existing
validators, including every output word, initialization bit and canary against
`(BigInt(a) + BigInt(N) * BigInt(b)) & 0xffffffffn`.
It runs **zero new source exports, LLVM lowerings or CPU simulations**.
Retained successful results do not authenticate the original toolchain.

### Original repository paths are part of this input contract

Run `scripts/ordered-repeat-native-observation.mjs` from the **same canonical
repository path whose adjacent helper scripts are pinned in both captures**.
`--repo` must name that exact owner; both receipts must remain at their original
absolute paths, with their original source, tool, stream and KIR/LLVM leaves.
File metadata as well as bytes/hash must still agree.

Do not copy a capture into another worktree, edit paths inside a receipt, strip
metadata, copy executable KIR, or relabel historical files as a fresh source run.
A checkout at a different path must first produce its own matching source and
LLVM captures. Staging this additive driver in an original capture repository
requires its owner's explicit scope; it is not permission to rewrite old inputs.
A copied source tree with identical text is not a substitute for retained file
identity. The canonical and mirror captures must be qualified separately.

## Build a separate observer with explicit inputs

Use the existing standalone tool's
[configuration and custody contract](../tools/fe2o3-instruction-native-observation/README.md#configure-and-build-templates).
Select the allowlisted unchanged external worker, exact LLVM22/static LLD SDK,
compiler/build tool, empty provider directories and independently reviewed build
claims. Keep a fresh build directory outside both source trees. Do not infer an
approved worker claim from a failed configure or reuse an unrelated build graph.

After that fresh configuration, explicitly build the additive target:

~~~text
ABS_CMAKE --build NEW_OBSERVER_BUILD
  --target ordered-repeat-source-candidate --parallel 2
~~~

The target is `EXCLUDE_FROM_ALL`; the existing observer, worker source/pins,
component graph and production routes are unchanged. The new target reuses
the unchanged retained-input/payload helpers and external worker pipeline and
decoder; it does not copy a second core ELF/MC decoder.

Before real observations, run the bounded shape controls under the same outer
supervisor:

~~~text
NEW_OBSERVER_BUILD/ordered-repeat-source-candidate --shape-controls
~~~

These are synthetic module, input-identity, file-snapshot and count-selection
controls, not source admission or native execution. The shape-only command does not invoke a worker or target machine.
Retain their actual result rather than assuming a build implies a pass.

Run the independent synthetic JS controls separately after approved staging:

~~~text
ABS_NODE --test ABS_ORIGINAL_REPO/scripts/tests/ordered-repeat-native-observation.test.mjs
~~~

These controls are not additional captured source or native cases.

## Run the source-to-native join

Keep the checked-in driver and imported scripts together; do not run a private
draft beside a different checkout's helpers. Independently select the observer
executable byte count/SHA and both receipt identities. Read the matching
[script](../scripts/ordered-repeat-native-observation.mjs) and
[pure controls](../scripts/tests/ordered-repeat-native-observation.test.mjs).

~~~text
node /ABS_ORIGINAL_REPO/scripts/ordered-repeat-native-observation.mjs
  --repo /ABS_ORIGINAL_REPO
  --source-receipt /ABS_ORIGINAL_SOURCE_OUTPUT/receipt.json
  --source-receipt-bytes SOURCE_RECEIPT_BYTES
  --source-receipt-sha256 SOURCE_RECEIPT_SHA256
  --llvm-receipt /ABS_ORIGINAL_LLVM_OUTPUT/receipt.json
  --llvm-receipt-bytes LLVM_RECEIPT_BYTES
  --llvm-receipt-sha256 LLVM_RECEIPT_SHA256
  --observer /ABS_FRESH_OBSERVER_BUILD/ordered-repeat-source-candidate
  --observer-bytes OBSERVER_BYTES
  --observer-sha256 OBSERVER_SHA256
  --llvm-build-id REVIEWED_LLVM_BUILD_CLAIM
  --worker-build-id REVIEWED_WORKER_BUILD_CLAIM
  --output /ABS_EXISTING_PARENT/NEW_NATIVE_OUTPUT
~~~

These are separate argv entries, not an executed shell command or authorization
to adopt whichever files are found. Paths must be canonical absolute paths.
The source and LLVM receipt directories are distinct. The output must be a
new leaf outside the repository, observer and both retained capture directories,
and must not contain any of those inputs. Its canonical parent already exists.

The current JS profile requires exactly this disclosed LLVM build claim:

~~~text
rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540
~~~

The worker claim is independently selected for the actual configuration and has
the form `fe2o3-worker-v1-sha256-<64 lowercase nonzero hex digits>`.
Claims are compatibility checks, not authentication of loaded libraries or an
executable digest. Another SDK requires a separately reviewed profile and fresh
qualification, not a string substitution to make this command pass.

Supply and pin the actual native loader environment externally. The driver
removes `LD_PRELOAD` and `FE2O3_*` variables but preserves the selected
`LD_LIBRARY_PATH`; it does not install dependencies or authenticate runtime
libraries through claim strings.

Exactly four child invocations are made. Each receives:

~~~text
N ORIGINAL_LLVM_PATH EXACT_LLVM_SHA256 EXACT_LLVM_BYTES NEW_PAYLOAD_DIRECTORY
~~~

Counts are 1, 2, 15 and 15 again. Each invokes the existing worker at O0 and O3.
The repeated fifteen case is a real fourth invocation, not inferred from equal
LLVM. Success therefore retains eight complete HSACOs in four
`<label>-payloads/` directories, each containing only `O0.hsaco` and
`O3.hsaco`, plus four stdout/stderr pairs and a new join receipt.

## What the native observation establishes

The raw LLVM parser verifies the actual module, exact kernel ABI and launch,
entry-first side-effecting statement, original three u32 inputs, sole direct
nonvolatile nonatomic global-store use and dominating result. It admits no
prefix, extra assembly/helper body or alternative register plan.
Ordinary surrounding i64 index arithmetic and checked-store control stay in
LLVM; this is not a whole-kernel LLVM bypass or full address/CFG proof.

The closed kernel is `ordered_repeat_u32`, `gfx942:xnack-`, wave64,
required/max workgroup `[64,1,1]`, code-object version 6. Roles remain
`[scratch32,out33,input34,input35,input36]`, with constraints
`=&{v33},{v34},{v35},{v36},~{v32}`.
Input36 and scratch32 stay declared even though this instruction text does not
read the third input or use the scratch operand.

The expected instruction literals are independent references, not claimed
measurements until an actual payload matches:

| Instruction | Decoded explicit operands | Expected little-endian bytes |
| --- | --- | --- |
| MOV e32 | VGPR33, VGPR34 | `2203427e` |
| Each ADD e32 | VGPR33, VGPR33, VGPR35 | `21474268` |

The external core decoder supplies the actual complete instruction roster and
ELF file offsets. The new matcher requires exactly one same-block contiguous
MOV plus N ADD run, each four bytes, with exact explicit/implicit effects and
registers. It rejects an extra following identical ADD, ambiguous duplicate
runs, gaps, wrong counts and wider encodings. The independent JS join rereads
each whole HSACO and compares instruction bytes at reported **ELF file offsets**;
it does not scan for an ambiguous byte pattern.

The exact 64-byte descriptor is independently located in the same ELF and joined
to the core decoder's digest. Its encoded capacity must cover the declared
high-water 37: `capacity=((rsrc1&63)+1)*8`,
`boundary=((rsrc3&63)+1)*4`, and `capacity >= boundary >= 37`.
This is not a promise of exactly 37 allocated registers, occupancy, metadata
usage, physical values, liveness or an inter-region ABI. Result-to-store use is
checked in LLVM, not proved after register allocation.

Each invocation exercises disclosed synthetic field/count/gap/duplicate/extent
refusals and privately changes a copy of the final ADD to SUB for real redecoding.
The negative requires the opposite opcode at the same offset; a decoder failure
alone cannot count as success. Original LLVM/HSACO bytes stay unchanged and no
mutated payload executes on hardware. The join also compares complete
fifteen/repeat O0 payloads and complete fifteen/repeat O3 payloads byte for byte.
That is a finite same-input observation, not universal reproducibility.

## Limits and remaining authority

The JS driver has four children, 180 seconds per child and a 19-minute
cooperative deadline beneath a separately enforced 20-minute outer deadline.
The C++ observer retains its own 90-second alarm. Neither is an RSS limit or
proof that descendants have stopped; outer process-group supervision remains
required.

Bounds are 64 KiB per child stream and LLVM input, 1 MiB per HSACO or receipt,
32 MiB output, 32 regular output files and five directories including the root.
At most 600 selected files, 512 MiB per file, 3 GiB unique selected bytes and
12 GiB cumulative selected reads are allowed. The 64 GiB available-RAM and
existing disk-reserve checks remain cooperative. SDK/loader/toolchain census,
concurrent-writer exclusion, task-root storage and fresh build budgets are the
outer owner's separate responsibilities; this guide grants no resource budget.

Selected original files, source helpers, observer, raw streams and complete
payloads are pinned/rechecked before and after relevant work. No original input
is rewritten; output names are create-new. A failed attempt retains partial
outputs and attempts `failure.json`; failures before output creation can have
only the enclosing command/stderr record. Never retry in an old output leaf,
delete failure evidence or weaken a predicate to obtain a result.

The receipt is observation-only. All source authentication, compiler/runtime
closure, protected/ranked proof/admission, artifact/launch/resume, physical
lifetime, native whole-kernel correctness, hardware and performance authority
remain false or unavailable. Synthetic worker-request identity fields remain
explicitly disclosed. This adds finite final-machine inspection to the earlier
source/CPU/LLVM chain; it does not complete M2, U2/U3, matrix/gfx950 coverage,
production qualification or the umbrella milestones.
