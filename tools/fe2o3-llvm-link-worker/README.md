# fe2o3 LLVM link worker

This worker is the measured, workload-neutral native-link boundary for fe2o3.
It accepts the canonical worker protocol, validates request identities and
bounded input bytes, checks LLVM module target and symbol contracts, links the
compiler module with explicitly measured device-library providers, emits an
AMDGPU object, invokes the bounded LLD policy, and inspects the resulting HSACO
before returning inert bytes.

The production path does not select kernels by symbol, source marker, shape, or
algorithm. Kernel entry and descriptor names come from the authenticated
request. For Worker V2/V3 compiler requests, launch metadata in the output is
reconciled with the compiler module's `amdgpu_kernel` calling convention,
required workgroup dimensions, maximum workgroup size, and wavefront mode.

## Derivation custody

A successful compiler response includes one bounded, domain-separated
derivation record for the exact linked LLVM module, optimized LLVM module,
generated AMDGPU object, ordered native-link inputs, path-independent LLD
invocation, and final HSACO. The generated object must be the final ordered
input after the request relocatables. Module identities are computed while
serializing through LLVM's streaming APIs, and the object and HSACO identities
cover the exact emitted bytes.

The LLD identity covers the fixed policy arguments, ordered undefined-symbol
roots, and ordered content identities rather than temporary paths. The policy
arguments used for that identity are also used to construct the real
`lld::lldMain` invocation. Rust independently decodes the response,
reconstructs the input order and canonical linker arguments from the exact
request, checks the final payload identity, and requires strict Worker V3
bootstrap/replay agreement. The wire version records this schema migration; it
does not add another compiler route.

Legacy request V2 keeps its exact encoding and emits success V4 (`F3LRSP04`),
without retaining stage bodies or imposing the capture limit below. The
production caller remains V2. Capture requires explicit request V3 (`F3LREQ03`):
fields 1-14 retain their V2 meanings, field 15 is the single byte `1`
(capture required), and field 16 seals the prefix under request hash domain V3.
Rust callers opt in through
`preflight_protected_reproducible_first_build_worker_with_revision_v3` with
`WorkerRequestRevisionV1::CaptureRequiredV3`; the existing preflight API selects
V2. There is no environment switch.

Only capture-required requests emit success V5 (`F3LRSP05`). Field 10
retains the linked bitcode, optimized bitcode, and generated relocatable object
in that order; field 11 binds the complete response content with the
V5 digest domain. Capture version 1 uses three ordered tagged, length-prefixed
bodies and a 1 MiB aggregate payload bound. Oversized captures fail explicitly;
the worker never falls back to hash-only success. Rust rejects successful
responses whose capture presence disagrees with the sealed request revision.
Both request revisions can return an inert V2 failure echoing the exact request
identity, without output or stage bodies. The object is the exact last input
consumed by LLD, not a separately regenerated object. All older request and
response encodings remain unchanged.

Rust checks these bodies against the derivation and retains borrowed ranges in
the canonical response. The live inspected-worker adapter additionally checks
bootstrap/replay byte equality and obtains the source LLVM from the retained
compiler handoff. It produces only `CheckedPostLlvmStageContentsV1`, with no
invented pass observations. Instruction-selection correspondence, complete
expanded pass/assembly custody, and authenticated occurrence consumption still
need producers before independently checked machine refinement is available.
Frozen compact finalizer replay cannot retain V5 stage bodies and rejects them
explicitly rather than silently downgrading the response. This capture alone
does not enable the typed V5 finalizer continuation.

This evidence establishes exact content and policy custody through the
upstream LLVM/LLD stages. It does not prove LLVM optimization or code-generation
semantic preservation, machine-code safety, or publication, load, or launch
authority.

## Output inspection

Publication inspection verifies the AMDGPU ELF class, target flags, code-object
version, dynamic/static export closure, absence of unresolved symbols, AMDGPU
metadata schema, descriptor-to-entry naming, argument ranges and alignment,
workgroup dimensions, and compiler-derived launch contracts. These checks are
structural and resource-oriented. They do not infer workload semantics from
machine code or grant source-to-machine refinement authority.

## Physical machine effects

The companion machine-effect analyzer reads a bounded `gfx942:xnack-` COV6
HSACO, validates its loader-visible ELF and metadata views, resolves a bounded
direct-call graph, and emits one canonical analysis bundle. The bundle contains
both static global-address, global-read, global-write, and return sites and the
complete decoded instruction/CFG trace: exact encodings, operands, explicit and
implicit register facts, branches, memory widths, and exact decoded trap
classification. Every serialized code
range and instruction location is a payload file offset whose bytes must match
the exact HSACO. The trace hash-binds the effect record, and the outer bundle
keeps both records indivisible at the worker boundary.

The analyzer accepts arbitrary canonical entry symbols and uses LLVM MC
instruction properties rather than a workload-specific opcode or CFG profile.
Unsupported memory spaces, atomics, indirect control flow, self-targeting
instructions, recursion, and unmodeled side effects fail closed. The exact
gfx942 `S_TRAP_vi` form is retained as a `may_trap` trace fact so a downstream
machine-semantics checker can prove the site unreachable; analyzer acceptance
alone does not discharge or authorize a trap. Direct branch
backedges, including basic-block self loops, are retained in the finite CFG so
ordinary bounded-loop machine shapes can be analyzed without a
workload-specific route.

The bundle enumerates reachable static instruction sites in the exact input
bytes. Accepting a backedge does not prove termination, a trip count,
loop-carried dataflow, or recurrence semantics. The evidence also does not
prove dynamic execution counts, concrete addresses, bounds, race freedom,
compiler refinement, source properties, or launch safety.

The Rust authenticated execution API copies the exact worker into a sealed
memfd, clears the environment to `LANG=C`, `LC_ALL=C`, and `TZ=UTC`, retains
the dynamic loader and every mapped DSO descriptor, and uses a fresh-challenge
`READY`/`DONE`/`ACK` handshake. The deployment policy pins an ASLR-stable
file-object closure. Each execution receipt separately binds every file-backed
map instance and kernel-provided executable map, including address range,
permissions, file offset, device/inode, path, digest, and object length.
Anonymous non-executable allocator mappings may move during LLVM analysis;
every persistent anonymous executable mapping is rejected. The exact admitted
mapping snapshot is measured after runtime initialization and again while the
worker is blocked before exit, so persistent file-backed additions, removals,
remaps, permission changes, and offset changes fail closed. A mapping created
and removed entirely between these two snapshots is outside this guarantee;
there is no continuous kernel-backed map audit.
Retained files are re-statted and rehashed after execution.
Every mapped runtime file must be immutable to the analyzer UID; a development
LLVM build with a writable `libLLVM.so` is intentionally rejected until it is
deployed read-only or through an equivalently sealed runtime closure.

## Build

The CMake configuration requires explicit matching LLVM and LLD package roots,
the pinned LLVM version and build identity, and an LLVM build containing the
AMDGPU target. It hashes the complete worker source and build configuration into
the asserted worker identity.

```sh
cmake -S tools/fe2o3-llvm-link-worker -B /tmp/fe2o3-worker-build \
  -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DLLVM_DIR=/path/to/llvm/lib/cmake/llvm \
  -DLLD_DIR=/path/to/llvm/lib/cmake/lld \
  -DFE2O3_PINNED_LLVM_VERSION=22.1.8 \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=<measured-build-id> \
  -DFE2O3_LLVM_BUILD_ID_FILE=/path/to/llvm-build-id.txt \
  -DBUILD_TESTING=ON
cmake --build /tmp/fe2o3-worker-build -j2
ctest --test-dir /tmp/fe2o3-worker-build --output-on-failure
```

LLD inputs are written only beneath a fresh private temporary directory and are
removed when the request completes. Diagnostics are bounded and canonicalized;
successful output remains inert until the Rust admission/publication pipeline
accepts it.
