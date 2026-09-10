# R66 Local Coexistence Validation

This record covers restricted compute/directional-SDMA coexistence and the
updated [A1/A2 swarm assignments](../../runtime-a1-a2-swarm-plan.md). It is local
CPU/proof evidence, not MI300X qualification, physical overlap, native-depth
qualification, generated production authority or HIP/HSA performance parity.

## Source And Scope

Baseline: `9e822ad737a21bd7cb563623ba3266066a6179d9`, branch
`codex/r65-runtime-drain-versions`. `source-files.sha256` identifies the changed
implementation, model, proof, runner and pin files tested here, relative to the
repository root. Unchanged files are inherited from the baseline. Documentation
and this record are not proof inputs. `retained-files.sha256` covers raw local
logs; these hashes are not hardware receipts or independent provenance claims.

The [R66 contract](../../runtime-compute-sdma-coexistence-v1.md) defines the
one/three-binding primary persistent compute plus directional-persistent
H2D/D2H profile. Native extraction checks complete retained ledgers, generation,
domain and storage identity; runtime integration narrows three scheduling
guards. Other copy/compute compositions remain excluded. Three implementation
agents contributed code, tests and model work; the primary integrated, reviewed
and ran the final local gates. The same agents independently reviewed the next
lane assignments without adding further production changes.

## Results

| Gate | Result |
| --- | --- |
| GNU all-feature/all-target tests, four crates | 1,977 passed; 15 failed; 5 ignored; 45 harnesses |
| musl all-feature/all-target tests, four crates | 1,977 passed; 15 failed; 5 ignored; 45 harnesses |
| GNU doctests | 64 passed; 4 harnesses |
| musl doctests | 64 passed; 4 harnesses |
| Existing R26/R40/R60/R61/R62/R63/R65 runner/checker tests | 105 passed |
| Clippy, all features/all targets, warnings denied | Passed |
| Clippy, no-default-feature production libraries, warnings denied | Passed |
| Formatting and source diff checks | Passed |
| Production Cargo metadata audit | 42 packages; 8 permitted build scripts |
| Full authenticated Verus rerun | Passed: 55 positive sources; 1,316 obligations; 632 expected-negative rejections |

The four Rust crates are `fe2o3-completion`, `fe2o3-runtime-model`, `fe2o3-kfd`
and `fe2o3-runtime`. Final tests include three-binding H2D/D2H in both publication
orders, destination-only dirty marking and exact restored storage custody. The
new R66 tests pass; the full unrestricted suite does not pass in this environment.

### Unrestricted Follow-Up

After the execution restrictions were lifted, both complete all-feature/all-target
commands were rerun against signed R66 source `ea71ce9c`, before the next
implementation wave. GNU and musl each passed **1,992 tests, zero failures and
five ignores across 45 harnesses**. Every changed source hash above was rechecked
unchanged after those runs. Their separate `*-unrestricted.log` files are
retained; the earlier failures remain recorded, not rewritten as successes.
Publication of `ea71ce9c` to both topic-branch remotes also succeeded. SSH became
reachable, but this follow-up did not execute a GPU qualifier.

The 15 failures per target are confined to existing socket/ptrace-dependent
tests: one KFD library telemetry test, one live-debug ptrace test, two telemetry
environment tests, eight telemetry tests and three runtime authorization
telemetry tests. The independent socket diagnostic succeeds at `socketpair` but
fails `getsockopt(SO_TYPE)` with EPERM. Ptrace fails in child pre-execution at
`PTRACE_TRACEME`; these operations are unavailable under the current sandbox.
Some rejection tests see the earlier sandbox error instead of their expected
socket-type/peer error. No tests were disabled or reclassified as passing.

The first integration run also found a stale source-order assertion referencing
the renamed private directional preparation method. That assertion was corrected
and both final target runs include the fix. An initial manifest-digest test used
an unsupported formatting trait; it was replaced with the existing byte-rendering
pattern before final validation. These development failures are not hardware
results.

## Proof Boundary

The R66 positive source verifies 16 obligations, including the executable bounded
scan's agreement with its quantified specification. Eight deliberate mutations
cover omitted endpoints, generation-hidden aliasing, foreign VM, duplicate
compute allocation, roster bounds and window extent/generation/completion.
The separately compiled Rust scan has reviewed correspondence, not a shared
compiled-source refinement theorem. Native extraction, physical identity,
publication/completion observations, whole executor behavior, firmware/GPU and
general machine semantics remain outside that proof.

The final runner exited successfully with its exact authenticated transcript.
Pre/post inventory, source, runner and pinned release-closure checks passed;
the closure contains 190 files and 129,019,839 bytes. These totals aggregate
property-specific proofs, not one whole-runtime verification claim.

The first full proof run was correctly rejected because a new negative emitted
`1 verified, 1 errors` without the required named failure surface. All eight
new negative fixtures were aligned with the existing named mutated-spec pattern
and repinned. The exact single-failure diagnostic checks were not weakened.
The rejected attempt is retained separately from the final run.

## Reproduction

Run from the repository root with the pinned Rust toolchain and offline cache:

```sh
cargo test --offline --no-fail-fast -p fe2o3-completion -p fe2o3-runtime-model -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets
cargo test --offline --no-fail-fast -p fe2o3-completion -p fe2o3-runtime-model -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets --target x86_64-unknown-linux-musl
cargo test --offline --no-fail-fast -p fe2o3-completion -p fe2o3-runtime-model -p fe2o3-kfd -p fe2o3-runtime --all-features --doc
cargo test --offline --no-fail-fast -p fe2o3-completion -p fe2o3-runtime-model -p fe2o3-kfd -p fe2o3-runtime --all-features --doc --target x86_64-unknown-linux-musl
cargo clippy --offline -p fe2o3-completion -p fe2o3-runtime-model -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo clippy --offline --no-default-features -p fe2o3-completion -p fe2o3-runtime-model -p fe2o3-kfd -p fe2o3-runtime --lib -- -D warnings
cargo fmt --all -- --check
```

Build/test invocations used `CARGO_BUILD_JOBS=4`. From
`benchmarks/runtime_gfx942`, run:

```sh
python3 -m unittest test_run_r26_inplace_runner test_check_r40_striped test_run_r40_striped_runner test_run_r60_pipeline test_check_r60_pipeline test_run_r61_owner test_run_r62_control test_run_r63_graph test_run_r65_drain_versions
```

The dependency audit consumes offline locked Cargo metadata filtered for
`x86_64-unknown-linux-musl` with default features disabled, using
`scripts/runtime_pure_rust_audit.py metadata --input <metadata> --root fe2o3-runtime`.
The authenticated proof runner is
`crates/fe2o3-runtime-model/verus/verify-verus.sh`, with the pinned Verus
`0.2026.08.09` release and `VERUS_TIMEOUT_SECONDS=120`. The final raw proof log
records the authenticated source/toolchain/negative/transcript checks.

## Hardware And Publication Limits

`ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x true` failed to resolve
`sharkmi300x-1`. No remote staging, builds, GPU jobs, resets, cache changes or
cleanup operations occurred. The next native assignment is the signed
OVL-QUAL-1 harness, followed by separately scheduled live qualification.
No R66 performance measurements exist and no speedup is inferred from these
tests. A1/A2 and the broader runtime goal remain open.
