# Historical Tutorial Default Cargo Census

This page describes the historical V54 compile/refusal observation. Select it
explicitly with `--legacy-compile-census`. The same script now defaults to the
[V91 production census](tutorial-production-census-v91.md); use the
[V92 end-to-end runner](tutorial-current-qualification-v92.md) for current
compilation, simulation, and CPU-reference comparison.

`scripts/qualify-tutorial-default-cargo.py` attempts the registered tutorial
sources through `cargo-fe2o3 authority release build`. It does not invoke the
LLVM extractor, a test callback, an alternate pipeline, or a synthetic receipt.
This is a compile/refusal census, not complete default-pipeline qualification.

## Run

Use an already built absolute `cargo-fe2o3` executable and the genuine protected
compiler/Worker configuration required by `authority release`. The harness
does not manufacture that configuration, remove refused environment variables,
or install a proof runtime. The production build configuration must select the
actual compilation units; unmatched units remain production refusals.

```sh
python3 scripts/qualify-tutorial-default-cargo.py \
  --legacy-compile-census \
  --repo-root "$PWD" \
  --cargo-fe2o3 /absolute/path/to/cargo-fe2o3 \
  --target-dir /absolute/owned/tutorial-cargo-cache \
  --output /absolute/owned/tutorial-cargo-census-new
```

The default includes every invocation returned by the existing validated
tutorial source registry: compiler fixtures and curriculum source-driver cases,
including expected negatives, on both target profiles. `--target gfx942` or
`--target gfx950` records an explicit subset, never an all-tutorial result.
At the implementation baseline there are 64 invocations: 24 gfx942 and 40
gfx950, including three expected negatives. Counts are derived, not hardcoded.

Each invocation preserves its package manifest, exact library target, declared
default features and feature list, source/lock hashes, expected kernel symbols,
and target profile. Cargo owns platform target selection; the harness sets
`FE2O3_TARGET` and never passes `--target`. The manifest features select the
source closure. Expected kernel symbols are retained as obligations, not used
as an invented compiler filter. Builds use `--release --locked --offline`.

The persistent target directory is separated by target profile and may reuse
ordinary Cargo dependencies. No cache hit or old artifact establishes a fresh
compiler invocation or a successful V53 publication. No Rust/Cargo build is
performed while testing this Python harness.

## Results And Limits

The new output directory contains an atomically checkpointed `report.json` and
one bounded combined stdout/stderr log for each attempted invocation. A missing
fixed production Verus runtime or missing protected configuration is recorded
before any Cargo invocation. Runtime presence is not runtime authentication.
The actual protected entry remains responsible for all custody and proof gates.

Failures do not stop later cases. Source snapshots are checked before and after
each attempt. A source/manifest change overrides even a zero Cargo exit. A
timeout or log overflow terminates the owned process group and reaps the direct
child; a truncated log is explicitly marked incomplete. The default bound is
600 seconds and 16 MiB per invocation, with at most 3600 seconds and 16 MiB
accepted. SIGINT, SIGTERM, and SIGHUP at the CLI boundary terminate the owned
process group, reap the direct child, and exit 130. An interrupted invocation
leaves the last incomplete checkpoint; it cannot count as completed or qualified.
The previous signal handlers are restored when the CLI returns. SIGKILL and
machine failure cannot run cleanup and are outside this guarantee.

Statuses distinguish `blocked-prerequisite`, `invalid-source-input`,
`launch-error`, `cargo-failed`, `timeout`, `log-limit`, `source-changed`, and
`cargo-completed-unqualified`. Expected-negative matching remains unknown:
an arbitrary Cargo failure is not evidence of the intended compiler rejection.

The report always preserves `qualified: false`, no compiler-execution
authentication, and no artifact/load/launch authority. A completed census exits
1, including when every Cargo command exits zero; malformed inputs or inability
to create the report exit 2. Full qualification still requires a same-invocation
complete V53 publication/kernel census, authenticated V50 execution, and the
independent native-refinement gates. The existing Float actual-rustc parent is
a useful synthetic source-chain smoke test, not a substitute for these ordinary
tutorial inputs or those gates.

## Harness Tests

```sh
python3 -I -B scripts/tests/tutorial_default_cargo.py -v
```

The `generic-core` CI lane runs this harness test suite, also inherited by the
`generic` lane. CI runs the harness tests, not the protected compiler census.

These tests use the real manifest validator and physical source snapshots, plus
explicit unit-only subprocess/mock outcomes for failure, timeout, output bounds,
signal interruption (including child-launch handoff), and report behavior.
They do not execute Cargo, Verus, Worker, or a GPU and must
not be recorded as tutorial compilation successes.
