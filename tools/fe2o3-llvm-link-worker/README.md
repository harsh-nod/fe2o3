# fe2o3 direct LLVM link worker

This standalone process accepts only the canonical bounded V1 stdin protocol
defined by `fe2o3-hsaco-finalize`. It parses and links bitcode through LLVM
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
ID; the response carries this composite worker/toolchain measurement and binds
the complete request identity.

LLD's library API is path-oriented. The worker therefore materializes exact
validated bytes in a private temporary directory, supplies only those paths to
`lld::lldMain`, removes the directory before exit, and redacts it from bounded
diagnostics. No path crosses the protocol boundary.

The native pipeline validates each input's actual file kind, AMDHSA OS ABI,
processor flags, and code-object ABI before linking. It rejects duplicate
strong definitions and unresolved non-weak imports, preserves requested LLVM
exports through optimization, and accepts output only when its public dynamic
symbols exactly match the request. LLVM verification and LLD diagnostics are
bounded before they enter the response.

`fe2o3-worker-pipeline-tests` builds real fixtures through the configured LLVM
libraries and covers bitcode plus bitcode, bitcode plus AMDGPU relocatable, and
multiple AMDGPU relocatables. It also checks deterministic output and rejects
kind, target, code-object, import, definition, and export mismatches. Supplying
an optional path writes the successful mixed-input HSACO for independent
inspection. Two additional paths also export the exact bitcode and relocatable
inputs used for that link:

```sh
build/llvm-link-worker/fe2o3-worker-pipeline-tests /tmp/fe2o3-mixed.hsaco
build/llvm-link-worker/fe2o3-worker-pipeline-tests \
  /tmp/fe2o3-mixed.hsaco /tmp/fe2o3-mixed.bc /tmp/fe2o3-mixed.o
```

The repository integration driver performs a clean Release configuration,
native CTests, focused Rust tests, and a mixed-input execution through
`PinnedWorkerV1`. All package and build identities are explicit arguments; a
missing prerequisite is an error. The final line states that this is native
link integration rather than a GPU dispatch test:

```sh
scripts/test-direct-llvm-worker.sh build/llvm-link-worker \
  /opt/rocm/lib/llvm/lib/cmake/llvm \
  /opt/rocm/lib/llvm/lib/cmake/lld \
  22.0.0git /tmp/fe2o3-rocm-llvm-build-id.txt gfx942
```

The worker response is descriptive evidence. It grants no loading or launch
authority. The local `/home/harsh/llvm-project/build` tree is an LLVM 24
development CMake package with a separately reporting `llvm-config`; it is not
the installed ROCm LLVM 22 toolchain and can validate source/API mechanics only.
