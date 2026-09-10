# R67 Owned Arguments, Credits And Coexistence Harness

This is local source/test/proof evidence for restricted MEM-1, GEN-1,
OVL-QUAL-1, DRN-3A and SCALE-3-PROTO. It is not a physical native memory budget,
production generated-launch authority, hardware receipt or HIP/HSA performance
result. The [swarm plan](../../runtime-a1-a2-swarm-plan.md) retains the separate
remaining acceptance gates.

## Source And Scope

Baseline: `4b897b3a806805f3d81cbbfc8809a1577267ea9c`, branch
`codex/r65-runtime-drain-versions`. `source-files.sha256` identifies changed
implementation, model, proof, runner and fixture inputs relative to the repo
root; unchanged inputs come from the baseline. `retained-files.sha256` identifies
the raw local logs, not independent hardware provenance.

The [owned-data and credits contract](../../runtime-owned-arguments-and-credits-v1.md)
documents the implemented boundaries. Context admission measures requested
bytes and allocation records, not resident backing. The
[native accounting inventory](../../runtime-native-resource-accounting-v1.md)
proposes the next lower-level ownership interface; extraction, parent budgets,
native batch/split transactions and aggregate quarantine remain unimplemented.
DRN-3A adds three composed failure/cancellation regressions without changing
production transitions. SCALE-3-PROTO checks matched capture consistency only.

## Local Results

| Gate | Result |
| --- | --- |
| GNU all-feature/all-target runtime tests, four crates | 2,021 passed; 0 failed; 5 ignored; 46 harnesses |
| musl all-feature/all-target runtime tests, four crates | 2,021 passed; 0 failed; 5 ignored; 46 harnesses |
| GNU all-feature doctests, runtime four crates plus host | 75 passed; 5 harnesses |
| musl runtime all-feature doctests | 64 passed; 4 harnesses |
| musl default direct-KFD host doctests | 10 passed |
| Default host library | 90 passed |
| GNU all-feature host library, valid guard environment | 207 passed; 4 existing ignores |
| Generated macro fixture harness | 7 passed |
| R26/R40/R60/R61/R62/R63/R65/R66 and SCALE-3 runner/checker tests | 132 passed |
| Six-crate all-feature/all-target Clippy, warnings denied | Passed |
| Six-crate no-default production-library Clippy, warnings denied | Passed |
| Formatting and whitespace checks | Passed |
| Production Cargo dependency audit | 42 packages; 8 permitted build scripts |
| Authenticated Verus gate | 56 positive sources; 1,330 obligations; 640 expected-negative rejections |

The four runtime crates are `fe2o3-completion`, `fe2o3-runtime-model`,
`fe2o3-kfd` and `fe2o3-runtime`; lint additionally covers `fe2o3-host` and
`fe2o3-macros`. The Python total includes eleven R66 tests and fourteen
SCALE-3-PROTO tests. Separate agent cross-review reran the latter fourteen.

### Additional Attempts

The optional all-feature musl host doctest command failed while building the
legacy `fe2o3-hip-sys` C shim because `x86_64-linux-musl-gcc` is not installed.
That attempt is retained as failed, not reclassified as a passing direct-KFD
gate. The default direct-KFD host musl command and all four runtime musl
commands above pass. No compatibility dependency was added to production.

The initial broader GNU all-feature host library attempt produced 176 passes,
31 failures and four existing ignores. The failures reported `Filesystem`
`ENOENT` in the durable publication helper; a single failing case rerun passed.
The primary had an explicitly set `XDG_RUNTIME_DIR=/run/user/1000/`, but that
directory does not exist. A set-but-missing runtime directory intentionally
rejects before the HOME guard fallback. A controlled single-case rerun with
that explicit stale value reproduces the rejection. Unsetting it for the test
process uses the existing correctly owned `0700` HOME guard directory.
Both primary full-library reruns pass 207 tests with four existing ignores;
the final reproducible command uses `env -u XDG_RUNTIME_DIR`. No production or
test source changed. Worker parallel and serial diagnostics pass all publication
cases but each report two unrelated socket-descriptor `EPERM` failures under
their execution restrictions; those diagnostic failures are retained separately.

The first two lint attempts found an owned-decoder collapsible condition and a
qualification-roster boolean simplification. Both were corrected without lint
suppression. `*-tests-final.log` files rerun GNU/musl against the final production
source; the earlier passing target logs precede the last boolean simplification
and are retained separately.

## Proof Scope

R67 contributes fourteen vector/record-decision obligations and eight named
mutations: partial debit, overflow, duplicate refund, premature refund, stale
owner, quarantine refund/reuse and partial release. The complete authenticated
runner passed exact transcript, pre/post source/inventory and pinned release
closure checks (190 files; 129,019,839 bytes). Proof source, pins and runner
inputs remained unchanged through the final CPU gate; the negative inventory
was independently rechecked.

The aggregate totals are property-specific, not a whole-runtime proof. Mutex
and arena ownership, Context adapters, native cost extraction and physical
disposal, parent/batch/split extensions, the generated async authority path and
whole reference-executor refinement remain open. Reviewed correspondence and
CPU regressions must not be reported as those missing theorems.

## Reproduction And Hardware Boundary

`raw/r67-final-source-gate.json` records exact test commands, working directories,
elapsed times and exit statuses, including failed attempts. Builds use the
repo-pinned Rust toolchain, offline cache and `CARGO_BUILD_JOBS=4`. Run the
Python modules from `benchmarks/runtime_gfx942` with `python3 -B -m unittest`.
The proof gate uses the repo-pinned Verus `0.2026.08.09` and
`VERUS_TIMEOUT_SECONDS=120`; the raw transcript records its authenticated
checks. The production metadata audit uses the no-default musl dependency graph.

The supplemental host environment checks are:

```sh
env -u XDG_RUNTIME_DIR CARGO_BUILD_JOBS=4 cargo test --offline -p fe2o3-host --all-features --lib
env XDG_RUNTIME_DIR=/run/user/1000/ CARGO_BUILD_JOBS=4 cargo test --offline -p fe2o3-host --all-features --lib published_direct_link::tests::exact_published_selection_is_admitted_without_authority -- --exact
```

The first exits zero; the second intentionally reproduces the environmental
rejection and exits 101 while that runtime directory is absent.

The R66 example covers eight R26 compute/disjoint directional-copy cells and
requested-credit saturation/retention/logical-disposal checks. This local
record does not establish successful native publication or physical overlap.
Signed live qualification and actual ELF/census/cleanup audits are separate
evidence. A1/A2 and the broader runtime parity goal remain open.
