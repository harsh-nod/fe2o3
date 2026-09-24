# Actual Manifest Fill: Source-Proof Prerequisite

Status: CPU and source-path validation passed; protected proof execution is
still unvalidated. This change does not complete #272, produce a signed source
capsule, establish safe GPU launch, or change curriculum qualification counts.

Primary integration passed the binding-aware CPU reference tests in both
feature modes, the actual no-GPU quickstart, both default-source regressions,
pre-proof ranked-output preparation for integer and float sources on gfx942 and
gfx950, and the missing-runtime refusal regression. Eleven focused reference
unit tests and three fill-harness unit tests also passed. These results do not
stand in for the protected positive, wrong-store, and post-bind integration.
See the [integration checkpoint](native-worker-finalization-replay-20260924.md)
for the complete validation scope and limitations.

Three focused fill manifest tests and the original-47 obligation-preservation
test passed after main-branch reconciliation. The earlier full Python manifest
suite reached its 300-second timeout and is not recorded as a full-suite pass.
Pinned toolchain staging to MI350 later reached its 900-second transfer limit
before any proof container started. The partial private staging directory was
removed and its absence verified; the shared protected runtime was unchanged.

## Source And Type Contract

The original `gfx942-fill-simulation` manifest selection still names
`examples/fill/src/lib.rs`, package `fe2o3-fill`, library `fe2o3_fill`, no extra
features, and kernel `fill`. Its GPU body, symbol, ABI and 64-thread workgroup
are unchanged. The migration adds an independent safe Rust
`fill_reference(usize, &mut f32)` and a nondefault `reference-proof` Cargo feature.
Mutually exclusive `cfg_attr` attributes select the ordinary kernel or attach
the existing `reference` annotation to the same body. The auxiliary proof
selection is distinct from the default manifest selection and grants no
qualification credit; it is not an additional manifest fixture or replacement.
Both stores specify `42.5_f32`, exactly `0x422a0000`; no approximate bound or
shared implementation is needed for this assignment.

`production_conditional_reference_output_v1.rs` now matches constant point
outputs as explicitly typed `u32` or `f32`. CPU signature, effect/store/RHS,
physical slice element and ranked operands must agree. Representation bits are
not numeric conversion: an integer constant with the same bits is rejected.
Float operands require the existing exact IEEE operator-congruence model;
bitvector, relaxed, wrong-width and changed-payload substitutions reject.
Matching unproved requests still fail the required-proof gate. Resource debit
and retained-owner checks remain in place.

The existing manifest validator rederived the fill package source closure,
compiler-input contract and SIMT source selection. The historical 308-byte
first-fill display at `7a536e0a001202ac0bb9d8647c5395661f8fa1ec` is unchanged;
its current-source association is pending because those bytes are no longer the
current library. The tile variant, execution evidence, aggregate proof and
qualification remain pending. Old receipts are not rebound to this migration.

Unsigned controls use the distinct `production-extraction-device` package's
`unannotated-fill` feature and `unannotated-fill-fixture` case identity. That
fixture is not a replacement manifest kernel, even though its symbol is `fill`.

Default `quickstart.sh no-gpu`, simulation export and `authoring-v6-smoke.mjs`
keep their ordinary source selection without requiring protected proof. There
is no production gate removal or automatic fallback: explicitly selecting
`reference-proof` still requires protected proof and conditional ownership.
The real quickstart regression passed, including exact output, untouched
canaries and temporary-file cleanup. Historical bundles and expectations are
not reused as proof or qualification of the changed source.

## Real-Source Integration Test

The registered backend test module is
`production_rustc_driver_v1::checked_output_source_v1_tests::manifest_fill_proof`.
It reuses the existing bounded subprocess and rustc-invocation helpers, actual
source callbacks, production effect producer/importer and post-bind observer.
It creates no alternate proof engine, fixed test signing key or authority path.

Preparation validates the manifest, resolves the pinned nightly sysroot and
Cargo package identity, and builds offline AMDGPU dependencies with one Cargo
job. It records the exact source, selected compiler input, lock, package, host,
reference tests, invocation and dependency-observation hashes. The default
manifest input is retained separately from the explicitly recorded auxiliary
`selected_features: ["reference-proof"]`; preparation and children assert the
actual rustc `--cfg feature="reference-proof"` argument, and the selected crate
binding must differ from the default binding. Metadata preparation also selects
the feature. The actual fill
source is used directly for positive cases. A separate private file changes
only the GPU store from `42.5` to `43.5`; a focused unit test requires exactly
that one-byte change and an intact CPU store. The mutation is never described
as the unchanged manifest source.

The protected parent runs three fresh children in order:

1. **Original:** the normal production transaction must reach the protected
   producer, import exactly one genuine freshly signed effect receipt, then
   fail at `FE2O3-OWN-002` for dynamic launch dimension zero. The later error
   alone is insufficient: the existing live producer/import hooks must observe
   the request, verified import and inert signature.
2. **Wrong store:** the changed GPU source must reach actual Verus execution and
   fail its semantic assertion, with one request and zero imports. Missing
   runtime, timeout, extraction failure or later ownership failure cannot pass.
   The kernel MIR and normalized obligation must differ from the original.
3. **Conditional:** the unchanged original source must acquire a new real proof
   and reach the existing post-bind observer. Its checked source/CPU join must
   identify one global `f32` output, raw reference argument one, one memory
   effect and one value expression. The current conditional owner runs its
   existing two fresh diagnostic pipeline checks; all nine mandatory checks
   remain pending for production admission. No lowering conversion is made.

Each child rederives preparation before and after execution. Compiler output
must remain absent. Results retain exact diagnostics, current invocation,
conditional observations and inert proof transport; these are test evidence,
not artifact or launch authority. The parent preserves separate stdout/stderr
logs. Successful test execution must be reported separately from this document.

## Coordinated Commands

Use nightly `2026-04-03` and frozen matching source/tool/dependency inputs. Run
these through the primary's existing serialized, source-stability build guard:
one Cargo job, incremental disabled, 12 GiB virtual-memory ceiling, nice 10 and
1200-second outer timeout. The integration checkpoint lists executed selectors
and results; the protected preparation and three-child run remain unexecuted.
Reproduction commands include:

```sh
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib production_conditional_reference_output_v1 -- --test-threads=1
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib manifest_fill_proof -- --test-threads=1
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib --no-run --message-format=json
```

For the CPU reference test, invoke the binding-aware CLI, not raw Cargo on fill.
The guard injects compiler/wrapper variables this CLI rejects even when empty.
Use the measured `CARGO_FE2O3` executable in a clean environment under the same
outer resource limits (`TC` is the pinned nightly directory; `SC` is the
primary's private build scratch):

```sh
env -i HOME=/home/harsh RUSTUP_HOME=/home/harsh/.rustup \
  PATH="$TC/bin:/usr/bin:/bin" CARGO="$TC/bin/cargo" \
  CARGO_HOME="$SC/cargo-home" CARGO_TARGET_DIR="$SC/target" TMPDIR="$SC/tmp" \
  CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 RUST_TEST_THREADS=1 \
  "$CARGO_FE2O3" test --locked --offline -p fe2o3-fill --test reference \
  -- --test-threads=1
```

Cargo home/workspace configuration must also contain no compiler, wrapper or
runner overrides. Repeat with `--features reference-proof` before the final
`--` for the auxiliary host selection. Neither host test executes GPU proofs.

Run the real default-selection regressions under the guard with warmed pinned
dependencies (the quickstart subprocess has a 300s bound):

```sh
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::checked_output_source_v1_tests::manifest_fill_proof::default_manifest_fill_quickstart_simulates_without_reference_proof \
  -- --ignored --exact --test-threads=1 --nocapture
cargo test --locked --offline -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::checked_output_source_v1_tests::manifest_fill_proof::default_manifest_fill_reaches_checked_native_output \
  -- --ignored --exact --test-threads=1 --nocapture
```

The first executes the advertised CLI and checks complete output/canaries and
temporary-file cleanup. The second compiles actual default source and simulates
boundary lengths, retaining the missing-source-proof refusal probe. Neither
substitutes the dedicated unannotated fixture for the manifest source.

Select the backend test executable from the no-run command's Cargo artifact
record, not a glob or an old binary. With `PROOF_PREPARED` a new absolute private
directory, run this exact ignored preparation selector under the same guard:

```sh
FE2O3_TEST_MANIFEST_FILL_PREPARE_V1="$PROOF_PREPARED" \
  cargo test --locked --offline -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::checked_output_source_v1_tests::manifest_fill_proof::prepare_actual_manifest_fill_inputs \
  -- --ignored --exact --test-threads=1 --nocapture
```

The preparer gives each subprocess 300 seconds and 16 MiB combined output,
with a 500 MiB observed fresh dependency-directory ceiling. Do not run it in
the proof container. Freeze its output and preserve all recorded absolute
paths when staging to MI350; preparation is not a dependency-closure authority.
The primary must retain source/tool/dependency measurements and recheck them.

After agreeing the measured backend executable, mount table, container name
and private results location, the proof-container payload is exactly:

```sh
FE2O3_TEST_MANIFEST_FILL_INPUTS_V1="$PROOF_PREPARED" \
FE2O3_TEST_MANIFEST_FILL_RESULTS_V1="$PROOF_RESULTS" \
  timeout --signal=TERM --kill-after=10s 960s "$PROOF_BACKEND_TEST" \
  --ignored --exact \
  production_rustc_driver_v1::checked_output_source_v1_tests::manifest_fill_proof::actual_manifest_fill_protected_effect_and_mutation \
  --test-threads=1 --nocapture
```

`PROOF_RESULTS` must not exist; its private parent must be writable by the
chosen non-root UID. Use 2 CPUs, 12 GiB memory/swap ceiling, 128 PIDs, no network
or GPU devices, read-only root/source/tool/dependencies/runtime, all capabilities
dropped, no-new-privileges and default seccomp. Only private results and a
bounded 64 MiB temporary filesystem are writable. The parent allows 300 seconds
per sequential child; the production producer's per-effect proof timeout is
unchanged. Do not use `seccomp-unconfined`, alter the shared runtime volume or
count an isolation/setup failure as a semantic failure. A refused runtime lease
requires a supported isolated runtime profile, not an authority bypass.

MI350's pinned image and named volume were revalidated read-only in this
subtrack. Under the above default-seccomp restrictions, short non-root probes
found root-owned `/proof/runtime` and `/proof/interpreter`, runtime root mode
0555, manifest mode 0444, and manifest version `0.2026.08.02.b677dd5`. These
probes ran only `find` and bounded manifest reading. They did not hash the full
closure, open the protected lease or execute Verus. Both disposable probe
containers were automatically removed and their absence checked. No shared
volume changes were made.

## Exact Remaining Producer Work

The current normal call chain is
`production_ranked_projection_v1.rs::project_and_verify_ranked_root_with_induction_scope_v1`
(reference branch),
`CompilerOwnedReferenceEffectRequestV2::prove_and_compile`, `prove_and_bind`,
`CompilerOwnedBoundReferenceEffectV2::into_staged`, then
`CompilerOwnedStagedReferenceEffectV2::compile` in
`production_reference_effect_join_v2.rs`. The last function invokes ordinary
ranked lowering, which cannot discharge dynamic TotalView for this source.

The shortest next producer change needs coordinated ownership of those two
modules and `fe2o3-pliron/src/production/{conditional_ranked_v1,conditional_pipeline_v1}.rs`:

1. Retain the genuine pre-ranked source owner, selected ownership occurrence,
   CPU binding, imported proof, signatures and staged graph across the normal
   call. Reuse `with_conditional_reference_output_v1`; do not select by fixture
   name or enable a test observer in shipping builds.
2. Add a consuming, source-bound conditional continuation. The existing
   `ProductionConditionalRankedAnalysisV1::check_pipeline_v1` already performs
   two diagnostic runs. Its `ProductionConditionalPipelineAnalysisV1` expressly
   cannot become `ProductionRankedKernelLoweringInputV1`. Keep that distinction:
   normal admission needs checked ownership under retained premises, all other
   mandatory analyses, source/graph replay and exact resource/owner accounting,
   not an unconditional clean report or removal of the ownership operation.
3. Bind the dynamic `N <= G` coverage premise, legal inactive dimensions and
   representable physical addresses to the actual source output and eventual
   descriptor/launch arguments. Empty output does not waive inactive-axis or
   address obligations. Preserve these assumptions through aggregate proof and
   the later host discharge. Changing any subject/premise invalidates the join.
4. Continue through `authenticate_ranked_root_v5` with retained signed staging,
   MIR/parallel reconciliation and the required aggregate proof. A per-effect
   receipt or a successful diagnostic pipeline is not that aggregate receipt.
   Native source-witness/custody admission must stay mandatory.

The shortest execution regression remains this actual-source parent. First
require its original and mutation cases to pass their stated boundaries, then
extend it with a normal conditional-continuation success, while retaining a
negative for genuinely uncovered output. Add `N=0`, `N>G`, inactive-axis,
address-overflow, cross-owner/premise replay and wrong typed-constant negatives.
Do not replace the original kernel with an empty or assembly-only fixture.

Main's complete-body v19 route does not close this dependency:
`ProductionCompilation::lower_complete_body_target_v19` dispatches only from
an authenticated complete-body terminal and explicitly rejects reference
bindings and protected publication. This fill has no such terminal. The new
source path must retain the production semantic/proof owners instead.
