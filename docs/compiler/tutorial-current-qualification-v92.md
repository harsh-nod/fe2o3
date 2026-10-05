# Current Tutorial Qualification

[`qualify-tutorial-current-simulation-v92.py`](../../scripts/qualify-tutorial-current-simulation-v92.py)
combines actual default production compilation, simulation of the exact captured
V18 graphs, and same-request CPU-reference comparison. It is the current
end-to-end runner, not evidence that qualification has already succeeded.

## Prerequisites

- An exact compiler checkout and its validated tutorial manifest.
- Its built `cargo-fe2o3`, `fe2o3-kir-sim`, and tutorial CPU-reference executables.
- The real protected compiler configuration, measured Worker, live services,
  and admitted proof runtime required by `authority release`.
- Both the manifest-pinned tutorial site checkout and the current site checkout,
  with Node and the dependencies required by the existing site export script.
- Fresh owned output and Cargo cache directories. A cached Cargo success is not
  a substitute for a same-invocation production census.

The runner does not install services, provision a runtime, waive proof checks,
or grant artifact/load/launch authority. Run it inside the qualified environment
with the existing protected Cargo configuration and environment.

## Run

```sh
python3 -I -B "$SOURCE/scripts/qualify-tutorial-current-simulation-v92.py" \
  --repo-root "$SOURCE" \
  --manifest "$SOURCE/config/tutorial-kernel-manifest-v1.json" \
  --cargo-fe2o3 "$CARGO_FE2O3" \
  --simulator "$KIR_SIM" \
  --reference "$CPU_REFERENCE" \
  --node "$NODE" \
  --site-pinned "$PINNED_SITE" \
  --site-current "$CURRENT_SITE" \
  --site-current-commit "$CURRENT_SITE_COMMIT" \
  --target-dir "$OWNED_CARGO_CACHE" \
  --output "$NEW_QUALIFICATION_DIRECTORY"
```

V92 requires the entire registered corpus: 64 invocations across gfx942 and
gfx950, 61 positive invocations, three expected negatives, and 45 positive kernel
names. It refuses a changed corpus until its versioned contract is updated.
There is no target-subset success mode. Defaults are 600 seconds per compilation
and 120 seconds per simulation; timeout or incomplete output cannot pass.

Each positive invocation uses `cargo-fe2o3 authority release build` with its
manifest-declared features, ordinary production backend, and live V89/V90
compilation census. Simulation consumes the captured graph belonging to that
same census, not checked-in IR or a separately generated approximation. The
reference executable generates the request used by both implementations, and
the comparison retains output bytes and read-only/padding observations.

Each negative must fail normally with its registered diagnostic, unchanged
source, no successful production census, and no compiler or simulation artifact
in its isolated output directory. An arbitrary Cargo failure is not a passing
negative test.

## Interpret Results

The atomically checkpointed `report.json` distinguishes completion from success.
Only exit zero with `complete: true` and
`currentCompilerSimulationPassed: true` establishes this runner's full scoped
result. Source/tool identities, each case, command outcomes, and comparisons
remain part of the evidence. A report from a different compiler revision cannot
qualify the candidate being published.

`qualified` remains false intentionally: this runner does not reinterpret the
older BundleV7/KIR12 simulation contracts, prove LLVM or hardware equivalence,
or execute GPU hardware. It also does not turn the 64 registered invocations
into coverage of every displayed website kernel. The manifest's pending source
bindings, including `mixed_tile_probe` and `tiled_gemm_lds_slice1`, require their
own general compiler support and execution evidence before an all-site claim.

The [production census](tutorial-production-census-v91.md) remains useful for
compile-only diagnosis. Its nonzero final exit and `qualified: false` are not a
replacement for the V92 result. See the complete
[activation gates](mixed-default-activation-v28.md#remaining-activation-gates)
for proof, deployment, default-route, target execution, and publication checks.

## Harness Tests

```sh
python3 -I -B scripts/tests/tutorial_production_census_v91.py -v
python3 -I -B scripts/tests/tutorial_simulation_v92.py -v
python3 -I -B scripts/tests/tutorial_cpu_reference.py -v
```

These test the harness contracts. Their success is not actual tutorial
compilation, protected proof execution, simulation/reference qualification, or
hardware execution.

The standard `generic-core policy` and `generic` validation paths include these
harness suites. They do not attempt protected runtime or hardware qualification
on generic CI hosts.
