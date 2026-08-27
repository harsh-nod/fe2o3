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
direct-call graph, and emits deterministic static evidence for global-address,
global-read, global-write, and return sites. It accepts arbitrary canonical
entry symbols and uses LLVM MC instruction properties rather than a
workload-specific opcode or CFG profile. Unsupported memory spaces, atomics,
indirect control flow, backward branches, recursion, and unmodeled side effects
fail closed.

The evidence enumerates reachable static instruction sites in the exact input
bytes. It does not prove dynamic execution counts, concrete addresses, bounds,
race freedom, compiler refinement, source properties, or launch safety.

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
