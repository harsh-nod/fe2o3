# Ordinary-source runtime acceptance

This bounded lab exports ordinary Rust and checks actual call/frame/storage identities through the public sealed CPU capture API. Instructions and validator unit tests are not a passing source capture: retain a fresh successful receipt for every qualification claim. The compiler and matching tools must come from the same reviewed checkout.

The observer has no private transcript attachment, deserialization, replacement, or mutable legacy-session escape. No GPU, browser, source-authentication or compiler-resumption authority is granted.

## Build

Use the selected pinned nightly 2026-04-03 direct cargo/rustc paths, existing checked build environment and external bounded target directory. The runner verifies rustc 1.96.0-nightly commit 55e86c996809902e8bbad512cfb4d2c18be446d9. Packages/targets:

~~~sh
cargo build --locked --offline -p rustc-codegen-fe2o3 --bins --lib
cargo build --locked --offline -p fe2o3-source-isa-observation --bin fe2o3-author
cargo build --locked --offline -p fe2o3-debug-cli --bin fe2o3-debug --example observe_runtime_observations_source_v1
cargo test --locked --offline -p fe2o3-debug-cli --example observe_runtime_observations_source_v1
node --test scripts/debug-runtime-observations-source-v1-test.mjs
~~~

The first build must supply fe2o3-export-sim, fe2o3-rustc-extract and librustc_codegen_fe2o3.so. Keep fe2o3-author in that same matching debug binary directory. The observer is debug/examples/observe_runtime_observations_source_v1. The surrounding owner must enforce its process, elapsed-time and root-storage lease while builds run.

## Bounded normal export + public observation

Choose two existing, empty/scoped resolved cache roots, an existing resolved persistent output parent and a never-before-created output directory. Repository, output and both cache roots must be disjoint. Do not reuse an earlier run directory or target on a retry.

~~~sh
/ABS/NODE22/node scripts/debug-runtime-observations-source-v1-smoke.mjs \
  --output /ABS/PERSISTENT/OUTPUT_PARENT/runtime-source-r1 \
  --bin-dir /ABS/BUILD_TARGET/debug \
  --observer /ABS/BUILD_TARGET/debug/examples/observe_runtime_observations_source_v1 \
  --rustc /ABS/PINNED_TOOLCHAIN/bin/rustc \
  --cargo /ABS/PINNED_TOOLCHAIN/bin/cargo \
  --rustc-driver /ABS/PINNED_TOOLCHAIN/lib/librustc_driver-HASH.so \
  --cargo-home /ABS/PREPARED_CARGO_HOME \
  --cache-root /ABS/LOOP_EXPORT_CACHE \
  --secondary-cache-root /ABS/WORKGROUP_EXPORT_CACHE
~~~

The runner creates the source fixture outside the repository, generates its offline lockfile, and uses the normal exporter. It does not edit the production workgroup fixture. Bundle input is regular/non-symlink, read with O_NOFOLLOW, checked before/after and SHA-bound. It requires a fresh successful diagnostic source census for each independent exporter invocation; a missing census is a failure, not permission to substitute handwritten KIR.

Per-command stdout/stderr each cap at 1 MiB; total retained streams at 8 MiB; receipt at 16 MiB. Each export caps at five minutes, each observer and real CLI admission command at two minutes. The process supervisor owns one subprocess group, kills it on timeout/limit, and reaps it. It checks 40 GiB disk and 64 GiB RAM reserves and a combined 20 GiB limit for the two scoped caches plus output. These checks supplement, not replace, the owner's whole-task lease.

## Artifacts / separate transport acceptance

Successful receipt schema: task-runtime-observations-source-capture-v1; status passed.
Failed runs retain failure.json and command logs, not a passed receipt.

- artifacts.loop.bundle = {path, bytes, sha256}, file loop-helper-v6.fe2sim.
- artifacts.workgroup.bundle = {path, bytes, sha256}, file workgroup-reduce-v5.fe2sim.
- artifacts.loop.source / artifacts.workgroup.source = {path, bytes, sha256}.
- artifacts.requests["loop-rounds-0" | "loop-rounds-1" | "loop-rounds-3" | "workgroup-reduce"] = {path, bytes, sha256}.
- artifacts.loop.census / workgroup.census = {path, bytes, sha256, utf8}; independent run_ids.loop/workgroup.
- artifacts.loop.observer / workgroup.observer = {path, bytes, sha256, utf8}.
- The loop V6 uses bounded fe2o3-author operation pages. The unchanged workgroup V5 uses the observer's separate inspect-workgroup mode: native verified V5/V10 decode, complete bounded operation roster, exact bundled source-map binding, same-run source-file identity/length and actual LDS source span. artifacts.workgroup.native_v5_census_join retains that checked join; no V5-to-V6 repacking occurs.
- artifacts.loop.cli_admissions / workgroup.cli_admissions retain the exact request pin, actual same-build fe2o3-debug JSONL replies, and closed handshake validation. All four generated request files must pass the real strict bundle/request parser; discover/get_state remain revision0/cursor0 and terminate alone advances torevision1. This is request admission, not browser/transport qualification.
- measurements_before/after bind selected source/tool inputs. This is not a hermetic dependency closure.
- source_authenticated, hardware_observed, compiler_resume_authority all remain false.

The HTTP/browser lane can use the exact generated bundle/request pins. It must independently retain the actual same-owner service replies and page limits. This source runner does not establish wire, HTTP, browser, nested-source-helper, source-private-Alloca or source-fault coverage.

## Acceptance matrix

Loop: normal Bundle V6 / Canonical KIR V11, gfx942, loop_helper, grid 4 / workgroup 64, rounds 0/1/3, canonical and seed 71. Both reuse-off/on executions use observation-enabled public sealed captures. Each pair compares full execution and the unchanged legacy transcript, all output bytes/init bits and both guard words. Counts are 32 real helper activations total across six reuse-on runs; each contains the retained XOR/AND/mask operations. Actual suspended-caller keys and child identities/values are checked. Reverse/repeat restores the same historical keys; a later helper at the same depth must not inherit an old activation.

Workgroup: unchanged production-ranked-bounds-device workgroup_reduce_u32, feature workgroup_reduce_u32, Bundle V5 / Canonical KIR V10, gfx942, grid 128 / workgroup 64, input 2. Canonical and seed 71, each off/on; exact 128 output words equal 128, all initialized, both guards unchanged. The graph contains actual WorkgroupMemory U32[64], not private Alloca. The literal retained transition prefix must show A created in slot S generation 1, A released, then different semantic allocation B in the same S generation 2 with previous A. New/reused LDS starts zeroed and entirely uninitialized. A is absent at B's checkpoint, B absent at A's checkpoint; historical seek/repeat preserves exact identities.

A final workgroup release may occur after the last retained record. The public query has no invented terminal checkpoint, so terminal_release_claimed remains false. Exact historical seek across workgroups is not relabeled reverse-operation navigation within one invocation.

Any failed bound, unsupported topology or mismatch remains a failed source gate. Do not alter fixture semantics, lower to synthetic Alloca, increase caps silently, or remove assertions to manufacture qualification.
