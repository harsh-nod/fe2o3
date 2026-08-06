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

The worker response is descriptive evidence. It grants no loading or launch
authority. The local `/home/harsh/llvm-project/build` tree is an LLVM 24
development CMake package with a separately reporting `llvm-config`; it is not
the installed ROCm LLVM 22 toolchain and can validate source/API mechanics only.
