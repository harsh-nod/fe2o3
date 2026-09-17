# Diagnostic Realtime Observations (V32)

`fe2o3_device::diagnostics::realtime64() -> u64` is a checked diagnostic
observation for the exact `gfx950:xnack-` target. The importer requires the
reviewed device provider, diagnostic item/DefId, definition path and exact
zero-argument unsigned 64-bit signature. Spelling a local function `realtime64`
does not grant counter authority.

## Wire Boundary

Semantic V32 extends the admitted V28 grammar with `Realtime64`, intrinsic tag
87. It does not reopen the inert V29 execution-capability grammar. V32 rejects
the retired and capability tags 69 through 86; V29 capability types also remain
outside V32. Exact older decoders reject V32, and older encoders reject the new
operation. Modules without the new observation keep their existing production
version selection and bytes. The reviewed original git and Cargo-vendor device
closures remain accepted alongside the two exact new source closures.

The independent importer and planner terminal-identity encoders use tag 122 in
their current CombinedV4 schema. Existing tags, including collective-context
tag 111, are unchanged. Exhaustive terminal-roster tests check both encoders.

## Observation Contract

The KIR operation is an ordinary, conservatively effectful diagnostic call with
a full `u64` result, not a referentially pure expression. Repeated observations
must not be commoned or deleted even when their results are unused. The call
bridge preserves occurrences; an empty Rust-memory-effect list is not a purity
claim. No memory read, write, alias exclusion, synchronization or happens-before
authority is fabricated for the counter.

The combined V12 effect summary retains `OrderedExecution`, including through
ordinary internal calls. It is therefore not complete-and-pure under the
existing helper policy. Neither that helper gate nor generic unsupported-order
checks are relaxed to admit the observation.

The gfx950 emitter uses `llvm.amdgcn.s.memrealtime`, without truncation or added
pure/speculatable attributes. LLVM 22.1.2's intrinsic definition includes
`IntrHasSideEffects` and excludes speculation. This retains observations but
does not fence independent arithmetic or memory operations around them.

The counter is not a memory-completion wait, a workgroup barrier, or a timestamp
of a completed operation. Instrumented code must establish required completion
and execution placement separately. A pair of counter reads around source text
alone does not prove that all work in that text completed between the samples.

AMD's CDNA4 ISA section 8.2.5 specifies the hardware realtime counter and a nominal
100 MHz rate independent of core power state. This API returns opaque ticks:
there is no measured frequency conversion, cross-CU/XCC epoch agreement, host
clock correlation, or equivalence to KFD `GET_CLOCK_COUNTERS` in its contract.
Those require separate device qualification. Host spans must not be presented as
GPU overlap, and these observations alone are not calibrated GPU task spans.

## Primary Sources

- [LLVM 22.1.2 AMDGPU intrinsic definitions](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.2/llvm/include/llvm/IR/IntrinsicsAMDGPU.td)
- [AMD CDNA4 Instruction Set Architecture, section 8.2.5](https://www.amd.com/content/dam/amd/en/documents/instinct-tech-docs/instruction-set-architectures/amd-instinct-cdna4-instruction-set-architecture.pdf)

## Verification Scope

Focused schema tests cover the exact new tag/signature, canonical round trip,
older-version rejection, unchanged no-clock bytes, and continued V29 rejection.
Target tests cover two full-width observations and hostile target/argument/type
mutants. An explicitly enabled CPU LLVM test checks O2 retention and native
`s_memrealtime` emission. Ignored source tests exercise ordinary checked Rust
extraction, target rejection, and a spelling-only negative control.

These tests do not claim GPU execution, calibration, phase completion, measured
overlap, protected runtime admission, or nondeterministic functional correctness.
