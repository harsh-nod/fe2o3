# Checked physical-entry source and inert handoff (V20)

The fixed gfx942 Wave64 physical-entry profile can continue from actual Rust
through authenticated MIR37, exact KIR20, mandatory ranked analysis, the
combined output/compiler-kernarg memory report, canonical LLVM emission and
the normal inert compiler-module handoff. This is a bounded profile, not
arbitrary AMD assembly or a claim that the assembly-authoring milestones are
complete.

The [authored fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/physical_entry_v20.rs)
owns setup, explicit loads and waits, indexing/address carry, output guard,
store completion, EXEC restoration and termination. Its logical signature is
`(DisjointSlice<u32>, u32, u32, u32, u32) -> ()`; the native ABI has six slots.
The exact source launch is workgroup64 and finite max_grid2. No compiler-owned
executable tail or hidden Rust interpreter is added.

## Public command

Build the extraction wrapper with the repository's installed pinned
`nightly-2026-04-03` toolchain and normal backend prerequisites. Do not substitute
an unreviewed rustc or auto-install a different nightly. The command uses the
ordinary Cargo-selected wrapper, not private callback code.

```sh
cargo +nightly-2026-04-03 build --locked --offline -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract
RUSTC=/absolute/path/to/nightly-2026-04-03/bin/rustc \
  node scripts/physical-entry-checked-v20.mjs one /absolute/new/physical-checked-one
```

Use `FE2O3_PHYSICAL_ENTRY_BIN_DIR_V20` if the wrapper and backend shared library
are outside the default target/debug directory. The output directory must not
exist. `diamond` and `registers` select the genuine selector diamond and a
physical output-register edit. The five exact rejection cases are
`wrong-launch`, `foreign-input`, `undefined-merge`, `missing-wait` and
`wrong-carry`.

The command runs separate LLVM and handoff Cargo sessions and retains
`canonical.ll`, `handoff-v2.bin`, source/binary hashes, logs and a receipt.
Negative cases require the precise source refusal and no output artifact; a
signal or unrelated compiler failure is not accepted. The script does not run
CPU simulation, a native LLVM worker, a GPU, a finalizer or host admission.
Hashes and serialized handoff bytes do not export compiler/source authority.

The separate [pre-ranked diagnostic command](physical-entry-source-v20.md)
remains available; its lower-level diagnostic owner is not silently promoted
to this checked path.

## What is checked, and what remains conditional

The source owner is retained through actual SSA/CFG materialization, fixed
ranked checks, exact canonical/source replay, canonical emission and descriptor
association. The combined memory report records each authored kernarg read's
actual source occurrence, block/operation, offset, width, SSA results and
explicit wait occurrence, alongside the real guarded output store.

Compiler kernarg memory is not a fabricated user allocation. Its 32-byte
explicit prefix, alignment8 and exact slot reads are checked. Two conditions
remain **runtime obligations**: kernarg must be immutable throughout execution,
and output must not overlap kernarg. The output still has the conservative
512-byte minimum for the 128-invocation launch envelope. CPU tail-canary tests
on shorter logical lengths do not relax that formal allocation requirement.

These conditions remain in a typed, move-only preparation through descriptor
construction. The only supported conversion explicitly demotes it to inert
LLVM/handoff data. There is no protected-finalizer, native-output or host-launch
admission API for this preparation, and no claim that the runtime conditions
have been satisfied. Generic and complete-body-only capability paths still
reject physical-entry capability text.

## Reproducible qualification boundary

The opt-in backend test
`production_rustc_driver_v1::gfx942_physical_entry_production_v20_tests::actual_physical_entry_production_ladder`
requires `FE2O3_TEST_PHYSICAL_PRODUCTION_OUTPUT_V20` naming a fresh absolute
directory and `RUSTC` naming the installed pinned compiler. It prepares actual
dependencies and runs 24 isolated rustc sessions: eight source fixtures across
owner observation, public LLVM and public handoff extraction.

Expected successful qualification is 576 existing-simulator CPU cases, 15
precise invalid-source refusals and 54 actual-owner ABI mutation refusals
(18 for each positive source). The latter cover missing/dropped/reordered read
records, incorrect source/wait sites, foreign source/canonical/descriptor
identities, dropping either runtime condition, slot mismatch and replacing the
exclusive output slice with an unrestricted pointer descriptor, or dropping
the retained validated source launch. The test
compares public outputs exactly with those derived from the same retained
source owner; it does not import source authority from the files. Twelve additional
denial-only controls check missing owner/ABI storage receipts, one-short storage
and one-short work budgets without admitting anything on a replacement ledger.

These counts describe the required test, not a retained execution result.
Publication evidence must separately identify the implementation commit,
command, source/receipt hashes and observed results. No native functional
execution, hardware register capture or milestone-completion claim is implied.
