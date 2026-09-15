# Result::map_err Source Helper

`authenticate_reviewed_safe_core_result_map_helper_v1(tcx, instance) -> bool`
discharges the unavailable source observation for the exact core inherent
`Result<T, E>::map_err<F, O>` method. It does not admit its callback, closure
shim, capture destructors, or function pointer target.

The proof anchors Result to its core lang-item variants and the method to its
inherent impl. It checks the four nominal generic roles before substitution,
the safe Rust signature, the exact RustCall `FnOnce<(E,)>::call_once` signature
and resolved instance, and the original generic optimized MIR. The contract
is bound to the complete instance, including T even when the callback is shared.

Both Result variants are interpreted with ownership tokens. Err moves O and
the original one-element E tuple into FnOnce exactly once. Ok transfers T and
drops O exactly once. Last-use Copy operands in pinned optimized aggregates
consume tokens, so duplicate transfers, stale reads, and lost captures fail.
Output cleanup drops and rustc's double-panic edges are retained. A missing
cleanup for a dropping active payload on an unwind target fails authentication;
an inactive variant's destructor does not create a cleanup obligation.

Bounds are 32 locals, 32 blocks, 96 total statements, 8 scopes, 32 debug entries,
3 mentioned items, and 1024 path-work units. Bounds precede body metadata scans.
Cleanup recursion is limited to one level. Cycles, invalid edges, unexpected
calls, and unvisited executable blocks fail. Only empty non-cleanup Unreachable
blocks may be unvisited. No body is cloned or changed by production code.

## Collector Integration

`trusted_device_items.rs` exports the authenticator to the collector's
external-source safety loop. Successful authentication applies only to the
current function in the already collected closure. Recursive discovery and
admission still cover the FnOnce call, shims,
callback body, and every Drop edge. Callback panic/unsafe bodies and unsafe
capture destructors remain subject to the ordinary downstream checks.
`production_mir_v1` continues to borrow `tcx.instance_mir(instance.def)` for
map_err. No expansion proof, terminal classification, ABI flattening, or
synthetic replacement body is needed. No new Cargo dependencies are required.

`compiler_tests.rs` contains six host collector regressions. The parent mounts
it under `collector::production_importer_v1`, following the bool/Option fixtures:

```rust
#[cfg(test)]
#[path = "../trusted_device_items/core_result_map_v1/compiler_tests.rs"]
mod core_result_map_compiler_tests;
```

The tests require exact retained instances for a moved-capture FnOnce callback,
the FnOnce adapter of a reusable closure, and a recursively called leaf. Unsafe
calls, user unsafe blocks, and panic paths must reject with their callback
chains. Empty and panicking payload destructors must hit the callback's
nontrivial Drop edge; dropping captures must hit the earlier capture-admission
guard while the original helper Drop still classifies as nontrivial. Fixture
callers return their Result directly and have no incidental Drop terminator.
These collector tests are left for the primary to compile and run after wiring;
they are not included in the standalone results below.

## Standalone Tests

From the worktree root, using the already installed pinned compiler:

```sh
RESULT_MAP_RUSTC=/home/harsh/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu/bin/rustc
RESULT_MAP_SYSROOT="$("$RESULT_MAP_RUSTC" --print sysroot)"
RESULT_MAP_TEST_DIR="$(mktemp -d /tmp/fe2o3-map-err.XXXXXX)"
trap 'rm -f -- "$RESULT_MAP_TEST_DIR/tests"; rmdir -- "$RESULT_MAP_TEST_DIR"' EXIT
export PATH="$RESULT_MAP_SYSROOT/bin:$PATH"
export LD_LIBRARY_PATH="$RESULT_MAP_SYSROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
"$RESULT_MAP_RUSTC" --test --edition=2024 \
  crates/rustc-codegen-fe2o3/src/trusted_device_items/core_result_map_v1/standalone.rs \
  -C prefer-dynamic -C rpath -L "native=$RESULT_MAP_SYSROOT/lib" \
  -o "$RESULT_MAP_TEST_DIR/tests"
"$RESULT_MAP_TEST_DIR/tests" --test-threads=1
```

The host suite covers actual installed core, nominal and body mutations,
non-Copy payloads, mutable references, captured moves and destructors, callback
identity, and metadata/work bounds. A separate locally compiled cleanup model
tests output cleanup and double-panic edges; it is not actual-core evidence.

Two AMDGPU tests are ignored by default and read only explicitly supplied,
completed core/builtins metadata. Set `FE2O3_WRAPPING_AMDGPU_CORE` and
`FE2O3_WRAPPING_AMDGPU_BUILTINS` to matching pinned files, then run before
the shell exits and removes the test executable:

```sh
FE2O3_WRAPPING_TARGET_CPU=gfx942 \
  "$RESULT_MAP_TEST_DIR/tests" map_err_actual_amdgpu_metadata \
  --ignored --nocapture --test-threads=1
```

Use gfx950 with its matching metadata for that profile. The tests use
`-Cpanic=abort`, `-Copt-level=0`, `-Cdebuginfo=2`, `-Zinline-mir=no`, and
`-Zmir-enable-passes=-JumpThreading`, once with default MIR optimization and
once with `-Zmir-opt-level=0`. These settings affect the caller; encoded core
MIR comes from the supplied artifact. Missing or mismatched metadata fails
without a host substitution, runtime skip, Cargo invocation, or sysroot build.
Proof failures print bounded MIR diagnostics. These are source authentication
tests, not collector-to-producer, all47 kernel, LLVM, or GPU execution evidence.

## Verified 2026-09-10

Standalone compilation used `rustc 1.96.0-nightly (55e86c996 2026-04-02)`
from nightly-2026-04-03. Six host tests passed. Both actual-core AMDGPU tests
passed with gfx942 and again with gfx950, including caller debuginfo=2.
The same completed, parent-supplied exporter-profile metadata pair was read
for both CPU settings under:

```text
/home/harsh/work/fe2o3-47-endtoend-20260910/local-amdgpu-metadata/amdgcn-amd-amdhsa/debug/deps
```

Artifact SHA-256 identities:

```text
e09b7198077801798ced6a41aed25ee21865ecddd96898ba4b2684ebbb6dd6f1  libcore-d4cdd9c8b80fe1e2.rmeta
87a47a54234634ce3a3148258a69764e2676cd20e36cebce02aa672b82756f31  libcompiler_builtins-dfaaf6f113af6683.rmeta
```

Rustc warned about the unstable/unknown wavefrontsize32, wavefrontsize64, and
xnack feature names; all profile tests passed. No Cargo, SSH, or network
commands were used. The standalone executable was removed after verification.
The collector wiring and full production/all47 integration validation remain
outside this helper's standalone evidence.
