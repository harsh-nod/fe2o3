# Experimental Checked-Load Grouping V1

The explicit
`lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_checked_load_grouping_v1`
entry point accepts the same verified ordinary compiler module as the existing
gfx950 entry point. Existing entry points remain unchanged and default off.
There is no environment-variable switch and no runtime or device API change.

Within a basic block, the planner groups two through eight independent,
nonvolatile, read-only global scalar `GuardedLoad` operations. It examines at
most 256 operations from the first load. Intervening operations must be total
integer constants/arithmetic, checked arithmetic, integer bounds comparisons,
integer/pointer selection or casts, slice data/length, or non-inbounds address
calculation. An earlier loaded value may not be an operand of any intervening
operation, later pointer, later predicate or later fallback. The planner stops
at every other operation and never crosses a basic-block boundary.

Independent address/bounds operations are emitted before an `and` of the
original predicates. The all-valid branch issues the loads in original order
in one straight-line block. The other branch retains each original predicate,
load and fallback in individually guarded diamonds. Each result reconverges
before any consumer. The final join keeps the last original guarded-load merge
label, so successor and backedge PHI predecessor names remain correct.

No floating arithmetic or reduction is moved or reassociated. The pass rejects
volatile and writable-pointer loads, ordinary unguarded loads, stores, calls,
divides, remainders, shifts, barriers, collectives, atomics, semantic markers,
allocation and assembly as intervening operations. Anchored, ordered,
debug-line, physical and native BF16 contexts cannot enable this pass. A BF16
bitcast consuming a just-loaded U16 ends the region; this is not an MFMA
prefill optimization.

## Qualification

Source implementation and tests are not an ISA or performance result. Run on
the approved remote build host, never the local workspace:

```text
cargo test -p fe2o3-amdgcn-model --test checked_load_grouping_v1
```

The test uses `llvm-as`, or `FE2O3_CHECKED_LOAD_LLVM_AS`, to verify representative
emitted CFGs. A deliberately closed interpreter executes the emitted
branch/load/phi subset for all 256 masks and distinct fallbacks, with no memory
entry for false-guard pointers. This checks emitted control flow, not native
hardware or general LLVM semantics. Root integration owns running these tests.

On October 7, 2026, mi300x-2 passed all 19 focused tests, including seven
LLVM assembler fixtures, all 256 guard masks, checked overflow/nonzero GEP
execution and the nontermination negative test. The source base was fe2o3
`1736eff451d445f1f194abe145af242cf51ec322`; qualification used a path-preserving
workspace projection with an audited dependency-subset lockfile. A separate
baseline suite completed 112 tests with 11 provider/device cases explicitly
ignored. Its clean outer result was observed; the subsequent SSH outage
prevented retrieval of that suite's full raw logs for independent retention.

Strict library Clippy failed on eight diagnostics in unchanged code. No lint
was suppressed or unrelated source repaired. No device image or native timing
has been produced with grouping enabled. The physical/anchored engineering
emission path still needs a separate, identity-bound experimental profile;
the ordinary-module API must not replace it or discard its physical checks.

Before any promotion, retain before/after compiler identities and actual LLVM,
ISA, register and spill evidence. Require multiple memory instructions before
the first wait/use, native component parity (including inactive tails), then
paired component timings and same-worker full-model token parity/TTFT/TPOT.
The additional fallback code and longer register lifetimes can regress code
size, occupancy or latency. No gain is predicted.

The relevant Ferric source is
`device/qwen3-tp-gemv-prefetch-kernels-v20/src/projection.rs`:
`ferric_qwen3_tp_wave_gemv_prefetch4_bf16_v20` places four raw input/weight pairs
before conversion/arithmetic, with independent checked bounds between reads.
Its partial-FP32 sibling has separate tail-control-flow regions and therefore
can group only each two-load pair. The separate Ferric endpoint-loador
experiment is needed to expose all eight loads in one partial-kernel block;
do not silently change its tail or floating-operation semantics.
