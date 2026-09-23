# Closed instruction-native observation

This standalone developer tool reproduces one diagnostic native-observation lane.
It is not a production worker, executable selector, finalizer, load/launch path,
proof/admission API, or compiler-resume route. Nothing registers it with the root
build or changes the external worker.

**Bounded standalone qualification passed on mi350 on 2026-09-22.** This directory
was freshly configured and built with the explicit static LLVM/LLD SDK, then used
for four default/edited O0/O3 observations and a strict whole-HSACO/source join.
Fresh normal source exports supplied 90 independent CPU simulations; twelve
CMake refusal controls also passed. These are developer observations, not
production admission, hardware execution or a general portability claim.

| Check | Observed result |
| --- | --- |
| Independent unchanged-worker claim and standalone observer configuration | Both passed in separate fresh output directories |
| New observer build and synthetic shape/identity/stat controls | Passed; typed shape 4 positive/48 negative, identity 1/3, file stat 1/9 |
| Fresh default, edited and repeated source | 3 exports, 100 child stages, 90 independent whole-buffer CPU simulations |
| New observer native runs | Default and edited, each O0/O3; 4 complete HSACOs retained |
| Strict source/native join | Passed; 294 retained input pins, 137299031 bytes; exact whole-payload offset checks |
| CMake refusal ladder | 12 exact first-diagnostic refusals; original inputs and SDK census unchanged |

See [source provenance](SOURCE_PROVENANCE.md#phase20-standalone-qualification)
for the new receipts, exact binary/source/LLVM identities and retained failed
attempts. The six runtime source/matcher files remain byte-identical to the older
private observer; the new run is independently recorded rather than inferred from
that history. Another SDK, host, relocation or configuration requires fresh checks.

## One fixed observation

The two accepted profiles are:

| Selector | Instruction descriptors | Final scalar expression |
| --- | --- | --- |
| `default` | XOR / AND / XOR: 133, 307, 413 | `b ^ ((a ^ b) & mask)` |
| `edited` | XOR / AND / OR: 133, 307, 412 | `b | (a & mask)` |

The edited expression is an independent algebraic reference, not a claim that
the edit preserves the default semantics. Both use the fixed declared register
plan `[scratch, output, a, b, mask] = [4, 5, 0, 1, 2]`. The only kernel is
`choose_bits`, with target `gfx942:xnack-`, wave64, workgroup `[64,1,1]`,
and code-object version 6. No live-prefix, high-register, helper, matrix, memory
instruction, or alternative ordering profile is admitted.

Raw LLVM must come from a fresh normal source export and normal LLVM emitter,
with its complete bytes/hash/size already selected in the source-runner receipt.
The input guard verifies the module and checks the exact kernel ABI, launch
metadata, target features, target-machine data layout, one inline-assembly
statement, exact constraints, direct original u32 arguments and the single result
use: the sole nonvolatile nonatomic global store dominated by the assembly.
Only ordinary i64 add/multiply launch indexing is allowed outside that statement.
This is a bounded typed shape/dataflow check, not a full address/CFG/memory proof.

The unchanged external worker builds those exact input bytes independently at
O0 and O3, links, checks symbol/target/launch closure and derivation, and decodes
the actual final HSACO. The byte-exact local `DefaultSourceMachine.inc` is a
small fixed-profile matcher and descriptor reader, not a second core decoder.
The core decoder remains the external worker's `WorkerMachineEffect.cpp`.
The default XOR matcher is unchanged; the edited profile has an independent OR
literal `0x280a0901`, opcode `V_OR_B32_e32_vi`, registers
`[VGPR5,VGPR1,VGPR4]`. Both require one unique contiguous same-block e32 triple,
exact raw/decoded bytes and registers, EXEC read/no implicit writes, and a final
descriptor whose encoded capacity covers v0..v5. Result use is checked in LLVM,
not proved after register allocation.

## Explicit build inputs

Use a native Linux host, CMake 3.20 or later, a C++20 compiler, a single Release
configuration and the reviewed LLVM 22.0.0git API. The actual qualified SDK
exports static LLVM components and static `lldELF`/`lldCommon`, with no
monolithic `LLVM` target. CMake requires that graph and leaves the worker's
component selection unchanged. The observer links `AsmParser` explicitly because
it calls `parseAssemblyString`; disabling worker tests must not depend on a
test-target component-mapping side effect. Monolithic/mixed SDK graphs, cross
compilation and multi-config generators are intentionally unsupported. No SDK
discovery or download is provided. This corrects an unrun first portable draft;
the historical successful Phase19 link used component archives throughout.

Supply every input explicitly:

- `FE2O3_INSTRUCTION_WORKER_SOURCE`: canonical absolute external worker source
  directory. [WorkerSourcePins.cmake](WorkerSourcePins.cmake) requires exact sizes
  and SHA256 values for the twelve measured worker files plus
  `tests/OrderedProgramWorkerSupport.inc`, from reviewed base
  `c60cd746e63b34b9072a493744d73b87ed1defc3`. A different checkout location is fine;
  different bytes fail. Updating these pins is a new review, not a fallback.
- `LLVM_DIR` and `LLD_DIR`: canonical absolute CMake package directories in
  the same SDK package tree.
- `FE2O3_PINNED_LLVM_VERSION=22.0.0git`.
- `FE2O3_EXPECTED_LLVM_BUILD_ID` and the canonical absolute
  `FE2O3_LLVM_BUILD_ID_FILE`: an independently selected claim and its retained
  file. The unchanged worker compares the file, claim and package version.
  This string is not authentication of loaded LLVM/LLD bytes.
- `FE2O3_INSTRUCTION_EXPECTED_WORKER_BUILD_ID`: an independently reviewed
  `fe2o3-worker-v1-sha256-<64 lowercase nonzero hex digits>` claim. This CMake
  compares it with the unchanged worker's newly generated claim and refuses a
  mismatch; it never silently adopts a new claim.
- `FE2O3_GFX942_DEVICE_LIB_DIR` and `FE2O3_GFX950_DEVICE_LIB_DIR`: explicit,
  canonical, existing empty directories, which may be the same directory.
  This closed kernel needs no provider; host default OCML directories are not
  inherited.
- Explicit absolute C++ compiler and build tool, plus any required matching
  SDK/system library inputs, for example zstd include/library paths.

All six path variables checked by the wrapper must be canonical, at most
4096 characters, with no semicolon/newline. The outer owner retains the SDK
manifest, source hashes, compiler/linker/CMake/build-tool identities, loader and
selected libraries before and after configure, build and execution. The source
allowlist is a compatibility check, not transitive toolchain attestation or a
defense against a privileged concurrent writer. The caller owns parent-directory
custody and resource/process bounds.

The worker claim depends on configuration, including compiler spelling/version,
flags and the gfx950 device-library directory. Relocation can legitimately change
it even with identical source. Do not copy the historical Phase19 claim merely
because this directory copied six runtime files. If no independent reviewed claim
exists for the chosen configuration, first prepare a fresh standalone build
configuration of the unchanged allowlisted worker under the same explicit
compiler/SDK/flags/device-library choices. Inspect and retain its
`fe2o3-worker-build-id.txt`, its configure transcript, source pins and inputs.
Only after that independent review pass the exact claim to a separate fresh
observer configuration. A claim emitted by a failed observer configure is not
automatically approved. The claim does not measure the final executable.

## Configure and build templates

These are parameterized argv templates for the bounded configuration, not a
verbatim past command or an automatic qualification of new inputs. Replace each
uppercase placeholder with one reviewed exact argument. Use fresh, distinct
build directories outside both source trees, not a shared Cargo/CMake target.
The canonical parent of each output directory must already exist; do not reuse
failed output directories. The outer runner must enforce deadlines, stream caps,
process-group cleanup and the current task's storage/RAM reserves.

For the optional independent worker-claim preparation, use the same options in
`COMMON_CONFIGURATION` below and run:

~~~text
ABS_CMAKE -S ABS_ALLOWLISTED_WORKER_SOURCE -B NEW_WORKER_CONFIGURATION
  COMMON_CONFIGURATION
  -DBUILD_TESTING=OFF
~~~

Inspect the generated claim before proceeding. `COMMON_CONFIGURATION` means
the following separate argv entries, not a shell variable or a hidden preset:

~~~text
-G "Unix Makefiles"
-DCMAKE_BUILD_TYPE=Release
-DCMAKE_CXX_COMPILER=ABS_CXX
-DCMAKE_MAKE_PROGRAM=ABS_MAKE
-DLLVM_DIR=ABS_SDK/lib/cmake/llvm
-DLLD_DIR=ABS_SDK/lib/cmake/lld
-DFE2O3_PINNED_LLVM_VERSION=22.0.0git
-DFE2O3_EXPECTED_LLVM_BUILD_ID=REVIEWED_LLVM_CLAIM
-DFE2O3_LLVM_BUILD_ID_FILE=ABS_RETAINED_LLVM_BUILD_ID_FILE
-DFE2O3_GFX942_DEVICE_LIB_DIR=ABS_EMPTY_DEVICE_LIB_DIRECTORY
-DFE2O3_GFX950_DEVICE_LIB_DIR=ABS_EMPTY_DEVICE_LIB_DIRECTORY
-Dzstd_INCLUDE_DIR=ABS_ZSTD_INCLUDE
-Dzstd_LIBRARY=ABS_ZSTD_LIBRARY
~~~

No historical task-root path is required. The SDK package layout is supplied by
the caller; `ABS_SDK/lib/cmake/*` illustrates the package directories, not an
assumed distribution root. Select and pin required loader search paths externally;
this directory does not alter a system loader configuration.

Then configure the observer with those exact same common choices and the
independently reviewed expected worker claim:

~~~text
ABS_CMAKE -S ABS_REPOSITORY/tools/fe2o3-instruction-native-observation
  -B NEW_OBSERVER_BUILD
  COMMON_CONFIGURATION
  -DFE2O3_INSTRUCTION_WORKER_SOURCE=ABS_ALLOWLISTED_WORKER_SOURCE
  -DFE2O3_INSTRUCTION_EXPECTED_WORKER_BUILD_ID=REVIEWED_WORKER_CLAIM

ABS_CMAKE --build NEW_OBSERVER_BUILD
  --target instruction-source-candidate --parallel 2
~~~

The dependency is added with `EXCLUDE_FROM_ALL`; only the selected observer and
its required worker libraries build. Worker tests are disabled in this standalone
scope without forcing a cache value. There is no installation or test-registration
step. Preserve the generated worker claim and all build logs. A successful build
alone does not qualify source, native results or the report join.

## Run templates and source custody

Run the existing synthetic guard controls, then the two real selected inputs:

~~~text
NEW_OBSERVER_BUILD/instruction-source-candidate --shape-controls

NEW_OBSERVER_BUILD/instruction-source-candidate
  default ABS_DEFAULT_LLVM EXACT_DEFAULT_SHA256 EXACT_DEFAULT_BYTES
  NEW_DEFAULT_PAYLOAD_DIRECTORY

NEW_OBSERVER_BUILD/instruction-source-candidate
  edited ABS_EDITED_LLVM EXACT_EDITED_SHA256 EXACT_EDITED_BYTES
  NEW_EDITED_PAYLOAD_DIRECTORY
~~~

Every native invocation performs both O0 and O3. Capture stdout and stderr into
separate new-only files under the outer supervisor, not shell redirections that
can overwrite old evidence. For a fresh repeated edited source export, the source
runner must already establish its exact LLVM equality with the edited input.
That join does not imply a third native invocation was performed.

The input is 1..65536 bytes and requires a lowercase nonzero SHA256 and exact
decimal size. It is opened no-follow and retained across both builds. Canonical
named path, complete raw bytes/digest, device/inode/mode/link count, size and
mtime/ctime are repeatedly compared; mutation refuses. No normalization,
re-serialization, metadata stripping, IR regeneration or preflight-hash
substitution is performed. This assumes caller-controlled parent directories
and ordinary filesystem semantics, not hostile privileged race resistance.

The payload directory must not exist. The observer creates it mode 0700, holds a
no-follow directory fd, and creates only `O0.hsaco` and `O3.hsaco`, each with
O_EXCL/no-follow and mode 0600. Each payload is 1..1048576 bytes. It fsyncs,
completely rereads, checks exact worker length/hash and metadata, and rechecks
both files before success. Partial failure can leave new incomplete files or a
directory: retain them, never overwrite/delete/retry in place, and do not infer
success from their existence. The outer owner checks the complete output-file
census and custody again after exit. This is not atomic production publication.

## Report and independent join

The report names and shape intentionally remain unchanged:

- `private-instruction-edit-shape-controls-v1`, authority `none`.
- `private-instruction-edit-native-observation-v1`, authority
  `unauthenticated-test-transport`.

Do not rename these as protected production protocols. Read complete stdout
with a bounded duplicate-key-rejecting JSON reader at 65536 bytes. Native reports
contain exactly two case wrappers in O0, O3 order, each with
`machine_observation`, `mutation_controls` and `retained_payload`.
The old low-register machine-case grammar remains separate; do not relax a legacy
XOR-only parser to accept the new whole report or OR profile.

Use a separately reviewed strict join such as the instruction-edit native report
validator, not diagnostic strings alone. Re-read the actual source-runner
receipt, source variants, raw stages and selected LLVM; join exact size/hash to
each native top-level and case identity. Re-read each complete retained HSACO,
require its exact output-directory/name/length/hash, and compare the selected
instruction bytes and 64-byte descriptor at the reported **ELF file offsets**.
Do not scan for ambiguous byte sequences or substitute decoded section-relative
offsets. Require target/wave/launch, exact declared registers and descriptor
bounds, both independent opcode literals, counts, build claims and all
non-authority flags. Source/MIR/KIR identities come only from the genuine
source-runner observation, never fabricated from the native LLVM or preflight.

In the source runner, a same-root/contract opcode edit must preserve retained
inventory identity, which describes the instance/root/contract census, not the
instruction body. Default versus edited source, semantic MIR, canonical KIR and
LLVM identities change; edited versus repeated identities agree. This tool does
not generate or assert those source-level joins. Finite independent whole-buffer
CPU results likewise come from the separate source runner, not this native tool.

## Existing controls and bounds

The unchanged code runs four typed-shape positives and 48 negatives across the
two profiles. Refusals include opposite last opcode, wrong registers/constraints,
e64, duplicate/hidden assembly, unused/extra-use result, wrong live-ins/arity,
effects, non-global/atomic/volatile/extra store, nonentry assembly, live/dead
prefixes, target/wave, module assembly and extra helper. Modules are verified
before the same shape predicate is tested. Input identity controls are one
positive/three negatives; file-stat controls are one/nine. Controls-only does
not invoke the worker or a target machine. These are synthetic controls, not
formal proof or filesystem race injection.

Each actual optimization adds seven decoded-field refusals, one stale-identity,
one isolated synthetic gapped-sequence, one raw-byte mismatch, and one redecoded
opposite-opcode refusal. The gap uses a disclosed 16-byte synthetic matcher view;
it is not a decoded ELF. The final control changes a private copy of the real
linked payload XOR-to-OR or OR-to-XOR, decodes it again, requires the opposite
profile and never executes it. Original LLVM and payloads remain unchanged.

Internal caps remain 90 seconds per command, 64 KiB input/report, 1 MiB per
HSACO, two retained payloads, 512 decoded instructions, 8192 LLVM instructions,
128 blocks and existing worker diagnostic bounds. Proposed outer deadlines are
60 seconds/configure, 300 seconds/build at two jobs, and 90 seconds/controls or
native command, with explicit bounded stdout/stderr. These are planning values,
not a new resource authorization. The outer owner must select current disk/RAM
reserves, a total retention limit, cancellation and process-group cleanup.
The observer's alarm is not a hard memory limit or descendant containment.

When qualifying another configuration, independently review the CMake/pins,
configure/build it in fresh output, run controls, produce fresh normal source
variants, run all four real native cases, run strict join positive and negative
controls, and compare complete before/after input/dependency/output custody. Refuse wrong worker-source size/hash, missing/noncanonical paths,
worker/LLVM claim mismatch, wrong SDK/version, nonempty provider directories,
stale/mutated inputs and reused payload directories; do not broaden on failure.

No hardware execution, whole-kernel native correctness, physical register
allocation/lifetime proof, source authentication, compiler/runtime closure
attestation, protected/ranked proof/admission or milestone closure is asserted.
All corresponding report flags remain false/unavailable. Synthetic worker
request identity fields remain explicitly disclosed.

## Separate bounded-repeat observer

The additive `ordered-repeat-source-candidate` target has its own closed
MOV-plus-ADD profile; it does not extend `instruction-source-candidate`'s two
selectors or relabel the earlier four-case qualification above.
See the [repeat-native workflow](../../docs/ordered-repeat-native-observation-v1.md)
for the exact original-repository-path capture contract and independent join.
The separate [dated qualification](../../docs/evidence/authoring-repeat-native-20260923.md)
records the actual repeat-native builds, controls and observations.

Configure a fresh standalone build with every existing guard, worker-source pin,
static SDK component and independently reviewed claim unchanged, then explicitly
build `--target ordered-repeat-source-candidate --parallel 2`.
This new target is `EXCLUDE_FROM_ALL`; no root build, test registration,
production worker, finalizer, decoder or launch route is added.

Its CLI is `--shape-controls`, or exactly
`1|2|15 ABS_LLVM SHA256 BYTES NEW_ABS_PAYLOAD_DIR`. Counts 1, 2 and 15 select
one MOV followed by precisely that many ADDs, with roles 32..36 and exact
constraints `=&{v33},{v34},{v35},{v36},~{v32}`. The encoded descriptor must
cover high-water 37. That is a capacity observation, not allocation/lifetime proof.
The existing input/payload helpers are shared without modification.

The separate source-to-native driver revalidates real retained source/CPU/LLVM
captures and invokes four fresh native commands, each O0/O3, retaining eight
whole HSACOs and joining instruction/descriptor bytes at actual ELF file offsets.
It executes no new source export, ordinary LLVM lowering, CPU simulation or GPU
kernel. Repeat-native results have their own dated evidence;
the original tool's successful receipts do not establish them.
