# fe2o3 direct LLVM link worker

This standalone process accepts the disjoint canonical bounded V1 and V2 stdin
protocols defined by `fe2o3-hsaco-finalize`. V1 remains byte-compatible and
generic. V2 binds the complete opaque compiler-envelope identity, exact
compiler-module input, every external provider, directional and final symbol
closures, target/options, and worker/toolchain identities. It parses and links bitcode through LLVM
libraries, emits AMDGPU relocatables through `TargetMachine`, invokes the ELF
LLD library directly, and returns a measured response on stdout. It never
loads LLVM into rustc, invokes a shell, runs `clang`/`ld.lld`, accepts caller
paths or flags, or searches for implicit libraries.

## Bounded physical machine-effect profile

The `--machine-effects-gfx942-v1` path is a separate, narrow alpha/zeta
analysis for exact `gfx942:xnack-` COV6 finalized HSACO bytes. LLVM Object and
MC APIs build a closed direct-call graph and enumerate reachable static global
address, read, write, and return sites. The loader view must be unambiguous:
all allocatable sections and analyzed symbols must agree with bounded `PT_LOAD`
ranges and permissions, the AMDGPU metadata section must exactly equal the one
loader-visible `PT_NOTE`, and `.dynamic` must exactly equal the one
`PT_DYNAMIC`. Its `.dynsym`, `.dynstr`, SysV hash, and GNU hash
declarations must agree with their uniquely mapped sections. Metadata,
descriptors, and `.symtab`/`.dynsym` kernel exports must agree, and every
section or dynamic-table relocation form is rejected. Indirect calls,
ambiguous call materialization, unsupported control flow, and unsupported ISA
fail closed.

The accepted ISA subset is intentionally small: recognized `GLOBAL_LOAD_*`,
`S_LOAD_*`, and `GLOBAL_STORE_*` sites plus the scalar/vector ALU, wait,
materialized direct-call, forward-branch, and exact entry/helper return forms
emitted by the alpha/zeta fixtures. Atomics and all `DS_*`, `FLAT_*`,
`BUFFER_*`, `TBUFFER_*`, `IMAGE_*`, and `SCRATCH_*` families are unsupported.
Backward/external branches, recursive calls, indirect calls, helper
`S_ENDPGM`, `S_SWAPPC_B64` destinations other than `SGPR30_SGPR31`,
modified helper `S_SETPC` return pairs, and unknown memory widths are also
rejected.

The Rust authenticated execution API copies the exact worker into a sealed
memfd, clears the environment to `LANG=C`, `LC_ALL=C`, and `TZ=UTC`, retains
the dynamic loader and every mapped DSO descriptor, and uses a fresh-challenge
`READY`/`DONE`/`ACK` handshake. The deployment policy pins an ASLR-stable
file-object closure. Each execution receipt separately binds every observed map
instance, including its address range, permissions, file offset, device/inode,
path, digest, and object length. The exact mapping snapshot is measured after
runtime initialization and again while the worker is blocked before exit, so
persistent additions, removals, remaps, permission changes, and offset changes
fail closed. A mapping created and removed entirely between these two snapshots
is outside this guarantee; there is no continuous kernel-backed map audit.
Retained files are re-statted and rehashed after execution.

The no-fork containment profile requires all real, effective, saved, and
filesystem UIDs to be equal and nonzero, an initial full-range UID map, and
zero inherited, permitted, effective, and ambient capabilities.
`PR_SET_NO_NEW_PRIVS` and hard address-space, data, output-file, core, and
`RLIMIT_NPROC=0` limits are installed in the child before `exec`. The native
entrypoint verifies that state; its deployed containment probe confirms
`fork`, `clone`, thread creation, and double-fork/`setsid` attempts are
denied before stdin. No cgroup or seccomp claim is made. Process-group cleanup
and descendant scanning remain secondary checks.

This evidence describes reachable static instruction sites only. It does not
provide concrete runtime addresses or execution counts and does not prove OOB
absence, data-race freedom, compiler/source refinement, Verus obligations,
publication eligibility, HSA load authority, or launch safety.

Configuration requires explicit matching LLVM and LLD CMake package paths, an
exact LLVM package version, and a build-ID file whose contents equal the
expected build ID:

```sh
cmake -S tools/fe2o3-llvm-link-worker -B build/llvm-link-worker \
  -DLLVM_DIR=/pinned/llvm/lib/cmake/llvm \
  -DLLD_DIR=/pinned/llvm/lib/cmake/lld \
  -DFE2O3_PINNED_LLVM_VERSION=22.0.0git \
  -DFE2O3_LLVM_BUILD_ID_FILE=/pinned/llvm/fe2o3-build-id.txt \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=llvmorg-22-rocm-7.2+commit
cmake --build build/llvm-link-worker
ctest --test-dir build/llvm-link-worker --output-on-failure
```

CMake prints a `fe2o3-worker-v1-sha256-*` response measurement derived from
the worker sources, LLVM version/build ID, C++ compiler identity, language
level, and fixed exception/RTTI settings. The request names the raw LLVM build
ID; V2 additionally names the expected worker measurement and pinned executable
content identity. Responses use the same version as their request and bind the
complete request identity. Unknown versions, V1/V2 mixing, and malformed V2
requests fail without downgrade.

LLD's library API is path-oriented. The worker therefore materializes exact
validated bytes in a private temporary directory, supplies only those paths to
`lld::lldMain`, removes the directory before exit, and redacts it from bounded
diagnostics. No path crosses the protocol boundary.

The native pipeline validates each input's actual file kind, AMDHSA OS ABI,
processor flags, and code-object ABI before linking. It rejects duplicate
definitions and every unresolved output symbol, including `__ockl_` and
`__ocml_` names because the protocol carries no authenticated runtime-import
allowance. Requested LLVM exports survive optimization, and post-link
inspection accepts output only when its public dynamic symbols exactly match
the request. The `gfx942` processor is independently checked from ELF
`e_flags`. AMDGPU MsgPack notes are decoded strictly and each metadata kernel
must bind an allowed entry plus its `.kd` descriptor. A `gfx942` output with
expected descriptors is admitted only when every kernel has required
workgroup size `[256, 1, 1]`, maximum flat workgroup size `256`, and wavefront
size `64`; descriptor-free generic requests do not acquire that G1 profile.
Before aggregate linking, V2 additionally requires each declared export to be
a public compiler-module definition and each declared import to be unresolved
by that module and defined by an external provider. LLVM verification, stable
post-link diagnostics, and LLD diagnostics are bounded before they enter the
response.

The closed `tiled_gemm_lds_v1` Slice1 profile additionally requires the sole
kernel metadata record to contain explicit zero SGPR/VGPR spill counts,
physically emitted `uses_dynamic_stack: false`, and the complete `.args` ABI.
Its three pointer/length pairs must declare exact names, source type names,
offsets, sizes, and value kinds. Pointers also require global address spaces and
declared access; the producer-guaranteed A/B read-only actual access and const
qualifiers and C restrict qualifier must be present. C actual access may be
absent or any valid subset of its read-write contract, while C const and A/B
restrict may be absent or false. LLVM-schema annotations that upstream LLVM 22
does not consistently emit (`.align`, deprecated `.value_type`, and global
`.pointee_align`) are optional, but every present value must agree with the
canonical ABI. The six-argument explicit span ends at byte 48, the COV6 hidden
span follows the upstream layout through the exact 304-byte kernarg segment,
and missing producer-guaranteed fields receive field-specific diagnostics. The
worker only decodes and validates emitted MsgPack; it never synthesizes, fills,
or rewrites HSACO metadata.

Argument range, ordering, ABI alignment, and pointee-alignment closure is
profile-specific. The generic metadata path retains LLVM's strict metadata
schema checks but does not impose Slice1's canonical argument geometry on
other kernels. The exact Slice1 path applies a closed root, kernel, and
argument-key allowlist matching the pinned producer and the optional fields
its contract recognizes; unknown keys fail closed. This policy does not alter
the metadata note or broaden the accepted Slice1 values.

The exact producer data layout is also allowlisted literally for this closed
profile. Upstream LLVM 22.1.8 adds the equivalent ELF mangling component
`m:e`; after accepting only the known producer spelling, the worker installs
its measured target-machine layout before code generation and validates the
final ELF symbol closure. Other explicit layout spellings remain rejected.

`fe2o3-worker-pipeline-tests` builds real fixtures through the configured LLVM
libraries and covers bitcode plus bitcode, bitcode plus AMDGPU relocatable, and
multiple AMDGPU relocatables. It also checks deterministic output and rejects
kind, target, code-object, import, definition, export, descriptor, unresolved
runtime-name, metadata, and G1 launch-profile mismatches while retaining a
descriptor-free generic path. Supplying an optional path writes the successful
mixed-input HSACO for independent inspection. Two additional paths also export
the exact bitcode and relocatable inputs used for that link:

The Slice1 metadata matrix independently mutates the offset, size, value kind,
and order of every required and optional COV6 hidden argument. It checks every
one of the 1,024 optional-hidden presence masks, required-workgroup omission
and mismatch, and unknown keys at each metadata map level. Companion generic
fixtures prove that noncanonical but LLVM-schema-valid argument layouts and
unknown extension keys do not inherit Slice1-only policy.

At this boundary, `COV6` means request code-object version `6`, LLVM module flag
`amdhsa_code_object_version = 600`, and AMDHSA ELF ABI version `4`. The pipeline
suite directly links a COV6 Worker V2 request containing two kernel entries and
one shared helper, then checks both AMDGPU metadata entries and both `.kd`
symbols in one output and proves equivalent producer input orderings canonicalize
to identical bytes. A canonical `.fe2o3.kd.v1` table may be carried opaquely
through compiler IR and the raw HSACO, but its parsing, executable-agreement
checks, and digest finalization are downstream duties; `ArtifactContainerV1` is
also constructed downstream. This worker neither parses nor authenticates
either format.

`fe2o3-worker-codec-tests` decodes a Rust-produced V2 golden, rejects every
single-byte mutation and V2-to-V1 downgrade, and checks V2 response framing.
The pipeline suite also executes the same mixed link as V2 and verifies that
the response echoes the exact compiler-envelope identity.

```sh
build/llvm-link-worker/fe2o3-worker-pipeline-tests /tmp/fe2o3-mixed.hsaco
build/llvm-link-worker/fe2o3-worker-pipeline-tests \
  /tmp/fe2o3-mixed.hsaco /tmp/fe2o3-mixed.bc /tmp/fe2o3-mixed.o
```

The repository integration driver requires an absolute build directory that
does not exist, then performs a Release configuration, native CTests, focused
Rust tests, and a mixed-input execution through `PinnedWorkerV1`. Cargo, rustc,
the source commit, and the Rust toolchain manifest are content-pinned. The
Cargo and rustc paths must share the declared toolchain directory; Cargo runs
with only that toolchain's library directory inherited in `LD_LIBRARY_PATH`.
The driver accepts success only after machine-readable Cargo/libtest JSON proves
that the exact ignored integration test ran and passed. A missing prerequisite,
dirty source tree, reused output path, empty evidence stream, or stale HSACO is
an error. The final line states that this is native link integration rather
than a GPU dispatch test:

```sh
toolchain=nightly-2026-04-03
toolchain_root="$HOME/.rustup/toolchains/${toolchain}-x86_64-unknown-linux-gnu"
cargo_bin="$toolchain_root/bin/cargo"
rustc_bin="$toolchain_root/bin/rustc"
scripts/test-direct-llvm-worker.sh /tmp/fe2o3-llvm-link-worker-fresh \
  /opt/rocm/lib/llvm/lib/cmake/llvm \
  /opt/rocm/lib/llvm/lib/cmake/lld \
  22.0.0git /tmp/fe2o3-rocm-llvm-build-id.txt gfx942 \
  "$cargo_bin" "$(sha256sum "$cargo_bin" | cut -d' ' -f1)" \
  "$rustc_bin" "$(sha256sum "$rustc_bin" | cut -d' ' -f1)" \
  "$toolchain" "$(sha256sum rust-toolchain.toml | cut -d' ' -f1)" \
  "$(git rev-parse HEAD)"
```

The narrower scalar GEMM V1 runner builds this same worker, rejects a COMGR
dynamic dependency, constructs the canonical scalar Kernel IR and LLVM module
inside the Rust integration test, and performs two complete Worker V2
first-build workflows. It requires byte-identical `gfx942:xnack-` COV6 output
and validates the sole `scalar_gemm_v1` entry and descriptor, 320-byte kernarg
(64 explicit plus the 256-byte COV6 suffix), workgroup 256, wave64, exact target,
and closed defined/undefined symbol sets. It produces and inspects an HSACO but
does not load or dispatch it:

```sh
tools/fe2o3-llvm-link-worker/run-scalar-gemm-v1.sh \
  /tmp/fe2o3-scalar-gemm-worker-build \
  /opt/rocm/llvm/lib/cmake/llvm \
  /opt/rocm/llvm/lib/cmake/lld \
  22.0.0git /opt/rocm/.info/version \
  /tmp/scalar-gemm-v1-gfx942.hsaco
```

`FE2O3_CARGO` and `FE2O3_RUSTC` may pin absolute toolchain executable paths for
noninteractive environments whose default `PATH` does not select the repository
toolchain.

The successful request is constructed from a `MultiInputLinkPlanV1` whose
expected output is the freshly generated deterministic native fixture. This is
a two-stage test fixture arrangement: it exercises plan-bound execution and
exact output verification, but it is not a first-build production API for an
output identity that is not known yet.

The worker response is descriptive evidence. It grants no loading or launch
authority. The local `/home/harsh/llvm-project/build` tree is an LLVM 24
development CMake package with a separately reporting `llvm-config`; it is not
the installed ROCm LLVM 22 toolchain and can validate source/API mechanics only.

The opt-in rustc `kernel-ir-worker-v2` path is connected through durable
publication. rustc publishes one attempt-scoped compiler handoff containing the
exact textual LLVM module, compiler FFI envelope, and compiler-derived
symbol-role manifest. `cargo-fe2o3` consumes that handoff once, executes a
GenericLink candidate and a compiler-FFI-aware V2 request with the same exact
module, providers, options, target, and measured worker, and requires the two
executions to produce byte-identical output. The worker performs both links
through LLVM and LLD library APIs directly; this path uses neither COMGR nor
command-line linking.

Cargo then independently admits the exact raw HSACO against the retained
target, code-object version, symbol roles, descriptors, and launch metadata.
`PreparedWorkerV2HsacoPublicationV1` derives a private publication plan from
that retained evidence. Descriptor-free COV5 remains a raw compatibility path;
descriptor-bearing COV6 is canonically finalized downstream, and the artifact
transaction durably publishes the exact admitted raw or finalized bytes plus
the provenance receipt for the same managed build attempt. The worker response
and the intermediate evidence remain non-authoritative by themselves.

The closed `wave64_collectives_v1` Worker V2 profile is narrower still. It
accepts only the exact `gfx942:xnack-` COV6 compiler input, O2/strip/verify
options, fixed kernel/descriptor symbol pair, canonical masked collective LLVM
body, and pinned MIR/KIR/profile identity sections. The worker audits that
module before target-machine emission and independently closes the resulting
WG64/Wave64 72-byte explicit and 328-byte complete ABI, resources, symbols,
metadata, relocations, and dynamic dependencies after in-process LLD.
The post-link check also disassembles the exact emitted kernel symbol through
the in-process gfx942 LLVM MC tables and rejects every machine call opcode.

For `.fe2o3.kd.v1`, this worker proves only transport identity: the output
section must be byte-identical to the compiler-input section. The Rust pinned
handoff expectation and canonical finalizer are the sole semantic descriptor
parser and exact descriptor-admission boundary. A successful worker diagnostic
therefore includes `rust_descriptor_admission=required`; it is not publication,
load, launch, compiler-origin, or functional-correctness authority.

This flow does not authenticate the compiler or its origin, authenticate or
bind Verus verification, construct an `ArtifactContainerV1`, or grant HSA load
or kernel-launch authority. Cargo owns canonical `.fe2o3.kd.v1` finalization for
COV6 and persists the exact publication kind, plan, upstream identity, bytes,
route/admission, and receipt. Raw and finalized publications recover across
process crashes, including migration of legacy raw markers; fault exits are
compiled only by the non-default `worker-v2-fault-injection-test-only` feature.

On `mi300x`, the ignored Debug-worker integration tests
`worker_v2_real_source_publishes_inspected_gfx942_hsaco` and
`worker_v2_real_source_links_an_external_bitcode_provider` pass for
`gfx942:xnack-`. They cover real-source handoff consumption, direct LLVM/LLD
linking, reproducibility, independent raw-HSACO admission, and durable
publication, including a closed external bitcode-provider import. They do not
load or launch the published HSACO, and no optimized Release-worker result is
claimed.
