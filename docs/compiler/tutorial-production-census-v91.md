# Tutorial Production Census V91

The default tutorial driver now requests the distinct `production-census-v91`
observation kind in a production V2 build configuration. The existing summary
and characteristic observers retain their meanings. V91 is not a receipt codec,
does not issue compiler or launch authority, and does not reinterpret a V53
descriptor, V50 receipt, BundleV7, or KIR12 artifact.

## Live Observation

`cargo-fe2o3` retains the existing authenticated broker connection for each exact
selected compilation unit. Fresh and recovered publication enter the ordinary
V89 finalizer and receipt-bearing load-envelope constructor. Ready recovery
still requires the paid V89 artifact inspection and exact compiler subject and
profile checks. The observer snapshots the retained envelope while it is live,
but submits only after durable readiness, publication-intent retirement, and
successful build-attempt completion. An observer failure cannot repair or
change any production decision; it leaves the external census incomplete.

The snapshot inspects the actual finalized V89 artifact, exact V90 middle-end
preimages and typed signed receipt, and all descriptor roots. It joins source,
original and final graph identities, retains all four graph identities, records
Policy11, all six compiler image identities, protected compiler policy/receipt,
proof runtime/tool identities, artifact digest and size, and the complete kernel
name/contract census. Input, record, aggregate, name and kernel counts are
bounded. There is no legacy-decoder retry.

The broker checks configuration, unit, session, invocation, generation and
finalization joins, rejects conflicting duplicates, and exports only a complete
selected-unit collection. The text export remains an observation of the pinned
driver's real invocation, not a portable attestation or a new authorization API.

## Driver

```sh
python3 scripts/qualify-tutorial-default-cargo.py \
  --repo-root "$SOURCE" \
  --manifest "$SOURCE/config/tutorial-kernel-manifest-v1.json" \
  --cargo-fe2o3 "$PINNED_CURRENT_DRIVER" \
  --target-dir "$OWNED_CACHE" \
  --output "$NEW_REPORT_DIRECTORY"
```

The existing protected Cargo environment and installed proof runtime are still
required. Supply exactly one existing production V1 or V2 configuration as a
template. For each registered invocation the harness preserves its worker,
provider and resource policy, changing only the exact manifest-derived library
selector and explicit V91 observation kind. It independently derives the V2
transitive configuration and unit identities, sets their expected binding, and
pins all compiler images and configuration inputs before and after Cargo.
Workspace selectors are derived from the manifest's exact lockfile location.

The real command remains `cargo-fe2o3 authority release build --locked --offline
--release --manifest-path ... --lib`, with exactly the declared features and
`FE2O3_TARGET`. No kernel filter or alternate backend is introduced. Existing
per-command time, log, process-group cleanup and direct-child reap requirements
are unchanged. A cached zero-exit build without a same-invocation census fails.
Use a fresh owned cache when a genuine invocation is required.

`--legacy-compile-census` retains the old compile/refusal diagnostic explicitly;
it cannot satisfy the V91 compilation gate.

## Remaining Qualification

`production-compile-census-pass` is scoped compilation evidence, not overall
tutorial qualification. The manifest still requires CPU references, exact
negative diagnostics with absent output artifacts, and semantic simulations.
The declared `scripts/run-tutorial-compiler-fixture-simulation.py` adapter is
absent from the tracked tree and history; current simulation contracts request
BundleV7/KIR12, not the compiler's V18 graphs. This batch does not substitute the
V18 diagnostic simulator or change those contracts. These requirements remain
explicit failures, `qualified` remains false, and the driver returns nonzero
until the complete qualification path exists. Hardware execution is separate.

Tests in `scripts/tests/tutorial_production_census_v91.py` are synthetic parser,
identity, complete-roster, input-custody and failure controls. They provide no
actual compiler, protected proof, simulator, deployment or hardware credit.
