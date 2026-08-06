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

`fe2o3-worker-pipeline-tests` builds real fixtures through the configured LLVM
libraries and covers bitcode plus bitcode, bitcode plus AMDGPU relocatable, and
multiple AMDGPU relocatables. It also checks deterministic output and rejects
kind, target, code-object, import, definition, export, descriptor, unresolved
runtime-name, metadata, and G1 launch-profile mismatches while retaining a
descriptor-free generic path. Supplying an optional path writes the successful
mixed-input HSACO for independent inspection. Two additional paths also export
the exact bitcode and relocatable inputs used for that link:

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
that retained evidence, and the artifact transaction durably publishes the
exact admitted bytes and provenance receipt for the same managed build attempt
before completing the attempt. The worker response and the intermediate
evidence remain non-authoritative by themselves.

This flow does not authenticate the compiler or its origin, authenticate or
bind Verus verification, run canonical `.fe2o3.kd.v1` descriptor finalization,
or grant HSA load or kernel-launch authority. Exact retry is supported while
the prepared publication intent remains in the same Cargo process, but restart
recovery after the compiler handoff has been consumed remains incomplete.

On `mi300x`, the ignored Debug-worker integration tests
`worker_v2_real_source_publishes_inspected_gfx942_hsaco` and
`worker_v2_real_source_links_an_external_bitcode_provider` pass for
`gfx942:xnack-`. They cover real-source handoff consumption, direct LLVM/LLD
linking, reproducibility, independent raw-HSACO admission, and durable
publication, including a closed external bitcode-provider import. They do not
load or launch the published HSACO, and no optimized Release-worker result is
claimed.
