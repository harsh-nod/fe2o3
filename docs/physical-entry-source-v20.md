# Physical-entry source diagnostics (V20)

This experimental `gfx942:xnack-`, Wave64 profile lets the author write the
complete entry body: kernarg loads, global index and address arithmetic, carry,
bounds/EXEC masking, store, waits, EXEC restoration and termination. Rust owns
the typed five-argument source signature; diagnostic LLVM declares the ordinary
six-slot kernel ABI. Native descriptor validation remains a separate stage. There is no compiler-generated body tail.

This increment is **pre-ranked diagnostics only**. The canonical checker
validates the closed physical SSA/CFG profile, including pending loads and
pointer/carry provenance. Mandatory ranked/formal checks, production descriptor
continuation, normal worker handoff and protected artifact admission are still
unavailable. Existing production LLVM and handoff routes explicitly refuse it.
The public command itself claims no native or GPU execution. Separate
[source/native qualification](physical-entry-source-qualification-20260924.md)
records the observed diagnostic continuation and its remaining limits.

## Reproduce actual source diagnostics

Use the repository's pinned `nightly-2026-04-03` installation, with its core
sources already available. Build the backend and public wrapper through the
ordinary repository workflow. Then, from the repository root:

```sh
export RUSTC=/absolute/pinned/nightly/bin/rustc
export FE2O3_PHYSICAL_ENTRY_BIN_DIR_V20=/absolute/build/debug
node scripts/physical-entry-source-v20.mjs one /absolute/new-output
```

The script uses Cargo's actual selected source invocation and the public wrapper;
it does not reconstruct a compiler owner from serialized input. `diamond` and
`registers` are additional positive fixtures. The latter changes the authored
result register from v8 to v22. Source lives in
`crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/physical_entry_v20.rs`.

The public wrapper selection is
`FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_ENTRY_DIRECTORY_V20`. It is mutually exclusive
with all existing output modes and requires a fresh directory. The three outputs
are `canonical-v20.bin`, unchanged `canonical.ll`, and
`native-observation-input-v20.txt`. The sidecar describes the same retained
canonical owner and emission; it is an inert expectation for an independent
native checker, not an admission certificate. No native compiler is invoked.

The public command only exports diagnostics; it does not run CPU simulation.
The source qualification ladder below supplies separate CPU coverage. Raw V20
files are not accepted by the existing debugger CLI, and physical registers,
source-variable maps, hardware register capture and GPU stepping are unavailable.

## Source qualification and controls

Root/operator qualification can run this ignored test in a fresh absolute output
directory, with the same pinned compiler and backend test environment:

```sh
FE2O3_TEST_PHYSICAL_ENTRY_OUTPUT_V20=/absolute/new-ladder \
cargo test -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::gfx942_physical_entry_qualification_v20_tests::actual_physical_entry_source_ladder \
  -- --exact --ignored --nocapture
```

Observed on mi350 on 2026-09-24: 16 isolated Rust sessions, comprising
eight actual source fixtures through both live observation and the public
diagnostic driver; 576 CPU cases for three positive fixtures; ten precise
negative refusals. The CPU matrix covers both selector arms, grids64/128,
lengths0/1/63/64/65/127/128/129, extreme scalar values, view offsets and canaries.
It uses the existing simulator with typed V20 admission, not a second interpreter.
CPU behavior does not discharge any pending production/formal/runtime requirement.

The public script also accepts `wrong-launch`, `foreign-input`,
`undefined-merge`, `missing-wait`, and `wrong-carry`. Each must fail at its named
boundary with no diagnostic output directory. A generic compiler crash, signal
or unrelated rejection is not a passing control.

## Scope and custody

The bounded profile is one root with exactly required/max workgroup64 and
max_grid2, one block or a selector diamond, at most64 native instructions and
73 source marker occurrences. It has no helpers, loops, LDS, barriers, atomics,
global loads, arbitrary assembler text or caller-supplied physical plan.

The move-only source owner retains actual semantic SSA, source launch, exact
canonical graph and occurrence correspondence. Pure Rust transport is recorded
as eliminated; authored labels and terminators map to existing CFG edges. A
source replay reconstructs and compares the full graph, not just hashes.

The added canonical/source/emission storage and work share one cumulative
ledger. Existing bounded rustc/semantic SSA/root-planner accounts remain
separate; the receipt is not a whole-process RSS, allocator-capacity or compiler
query allocation guarantee. Output I/O and test-observation buffers have their
own explicit bounds. Serialized bytes do not export source or compiler custody,
and no data here grants artifact or launch authority.
