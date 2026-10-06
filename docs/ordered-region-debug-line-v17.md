# Opt-in ordered-region LLVM line attribution (V17)

Status: implemented bounded M5 slice, with model/backend tests and an actual
genuine-source LLVM export, real one-frame macro observation and actual O0/O3
linked-DWARF acceptance. The production default and existing lowering API
remain metadata-free. The new optional API with None delegates to the old entry
and must return exactly the old bytes. No transport or execution authority is added.

## Scope and joins

OrderedProgramDebugLineV17::try_new borrows the actual immutable canonical V17
owner and one source file. It checks the supplied canonical identity, assembly
source identity, exact function/block/operation roster coordinates, one matching
source site, one span, one matching file, source byte bounds and LLVM's 16-bit
column bound. Missing, stale, edited and ambiguous selections refuse. A separate
equal-byte owner cannot reuse the borrowed selection.

The public model API checks relationships, not provenance. Only the private
test-only OrderedProgramObservationOwnerV32::diagnostic_llvm_line_v17 uses the
existing source-first correspondence producer and same-session captured files.
That adapter retains the original materialized owner and authenticated bindings
through emission. It returns inert LLVM plus the same projection, which the
existing two-session debugger test reuses; no replacement compiler owner is
decoded from a source-map document. The existing diagnostic report schema stays
unchanged. It does not claim linked line-table acceptance.

The emitter puts exactly one !dbg location on the indivisible ordered inline-asm
call. Its enclosing kernel has a distinct subprogram; its declaration line is
unknown (zero), not invented from the region. The file spelling is a diagnostic
display path, escaped bytewise. The file identity is NOT a source-byte checksum
and is not emitted as one. Only line tables are requested; variables, scopes,
macro expansion frames and instruction microsteps are not asserted. An LLVM
line row may cover a coarser address range after optimization; consumers must not
infer that it identifies each authored instruction.

The metadata follows LLVM's documented compile-unit, file, subprogram and location
relationships: [LangRef debug metadata](https://llvm.org/docs/LangRef.html#specialized-metadata-nodes)
and [source-level debugging](https://llvm.org/docs/SourceLevelDebugging.html).
LLVM's [location implementation](https://llvm.org/docs/doxygen/DebugInfoMetadata_8cpp_source.html)
stores columns in sixteen bits; the adapter refuses overflow instead of letting
LLVM silently make the column unknown. No macro chain is relabeled as inlinedAt.

## Bounded resources

Construction allocates nothing and admits at most 16 files, 4096 sites and 8192
spans. Emission uses a new narrower 4 MiB opt-in ceiling on the existing compiler-module
writer (the legacy 16 MiB ceiling is unchanged); path bytes are
already bounded to 4096 and are streamed with at most 3 output bytes per input
byte. Existing lowering allocations remain under their existing policy, not an
RSS claim. The two-session private debug test prepays 8 MiB for two additional
LLVM strings inside its unchanged 128 MiB cumulative logical envelope and
reuses, rather than duplicates, each source projection.

## Reproducible CPU checks

Use the pinned Rust toolchain and normal bounded runner; retain failure output.
The model suite and focused backend tests passed; a full backend run timed out
and is not counted as successful. See the implementation status for exact counts.

    cargo test -p fe2o3-amdgcn-model debug_line_tests -- --test-threads=1
    cargo test -p fe2o3-amdgcn-model --lib
    cargo test -p rustc-codegen-fe2o3 --lib

Then run the existing actual-source two-session debugger qualification under its
existing prepared source/dependency invocation and parent supervisor. The same
source_candidate_debug_join_v17_tests::observe callback now performs opt-in
line emission before consuming that same projection as a debugger catalog.
Require both genuine default/edited callbacks, all old refusal/capture checks,
and the unchanged bounded output report. This does not emit a portable map.

## Actual O0/O3 linked-DWARF recipe (CPU only)

Do not run a GPU kernel, GDB, a production Worker service or a public finalizer.
Use the already reviewed LLVM22/LLD ordinary ordered-program native build route
and its exact SDK/tool closure, serial build, RAM/disk and outer process-group
limits. Build only the new additive test target:

    cmake --build ABS_REVIEWED_NATIVE_BUILD --target fe2o3-worker-ordered-debug-line-observation --parallel 1

First produce a synthetic deterministic model smoke input from the ignored
test (it prints exactly one BEGIN/END pair); bounded extraction must preserve
only the bytes between those markers without adding/removing bytes. This input
has display file src/kernel.rs, line 27, column 9, three ordered steps, used
result and fixed registers v32/v33/v34-v36. It is not genuine Rust source evidence.

    cargo test -p fe2o3-amdgcn-model print_synthetic_ordered_debug_line_llvm -- --ignored --nocapture --test-threads=1
    ABS_REVIEWED_NATIVE_BUILD/fe2o3-worker-ordered-debug-line-observation ABS_EXACT_LLVM ABS_FRESH_OUTPUT_DIRECTORY

The existing native reader admits at most 64 KiB of input LLVM; this is narrower
than the model debug-line emitter ceiling and is not raised by this change.
The observer invokes the unchanged ordinary V2 in-process compile/link helper at
O0 and O3 with the existing non-stripping options, verifies the same whole
three-instruction sequence and final descriptor capacities, and exclusively
writes O0.hsaco and O3.hsaco (each <=1 MiB). It rechecks the full LLVM bytes
after both compilations. Its JSON deliberately leaves line_table_verified=false.

For EACH exact final payload, under the selected trusted LLVM tool closure:

    ABS_LLVM_DWARFDUMP --verify ABS_FRESH_OUTPUT_DIRECTORY/O0.hsaco
    ABS_LLVM_DWARFDUMP --debug-line ABS_FRESH_OUTPUT_DIRECTORY/O0.hsaco
    ABS_LLVM_DWARFDUMP --verify ABS_FRESH_OUTPUT_DIRECTORY/O3.hsaco
    ABS_LLVM_DWARFDUMP --debug-line ABS_FRESH_OUTPUT_DIRECTORY/O3.hsaco

Require normal exits and no verification errors, bounded complete stdout/stderr,
actual .debug_line data, the exact file/line/column, and rows whose address
intervals cover the independently decoded ordered region in the same final ELF.
Translate decoded file offsets using the actual ELF section file-offset/address
mapping; do not compare file offsets directly with DWARF virtual addresses.
Require O0 and O3 separately; do not infer one from the other or accept section
presence alone. Repeat with legacy None input as a negative: it must be rejected
by the new observer's metadata precondition, while old native tests still pass.
Any tool diagnostic, missing line, ambiguous attribution or cap failure remains
a failed observation, not a reason to add guessed metadata or waive an old guard.

Proposed outer runtime caps: child 90 s; outer <=120 s including cleanup;
stdout/stderr <=64 KiB each; two final payloads <=2 MiB total; final DWARF command
10 s and <=1 MiB bounded output each. No cap is authority to execute: the root
selects a fresh finite CPU request and authenticates all actual inputs/tools.

The private actual_source_candidate_line_export_ladder now exports the actual
adapter-returned LLVM and expected source span while the original owner remains
alive. Its default and edited callbacks passed with full source rechecks and
stale-source/KIR refusals. The source export does not itself establish linked
DWARF acceptance by itself: the exact edited export subsequently passed the
native/DWARF checks above at both O0 and O3. The selected line-row intervals
cover the full twelve-byte region at line 7, column 20. LLVM22 output includes
OpIndex: the checker requires it to be zero with max_ops_per_inst=1. All 38
parser controls and the separate final input-bound acceptance passed. A separate ordinary-helper fixture checks that surrounding calls
are not falsely assigned the ordered region's location.

## Explicit remaining work

This slice does not complete M5 by itself. Still separate: nested macro and
LLVM inline-stack coverage, macro-frame viewer integration, finer
per-authored-instruction attribution where representable, allocator/expansion/
variant trace publication, general final optimized instruction-range joining
and consumer UX. The genuine three-step O0/O3 region is now joined, not a
general arbitrary-kernel guarantee. Existing KIR maps, CPU debugger/storage observations and
native whole-region matching remain valid evidence in their own scopes; they
must neither be discarded nor promoted to those missing observations.

## Diagnostic macro frames

The opt-in compiler producer follows actual expansion callsites and records
macro names and expansion/call/definition origins from the same compiler session.
Missing definition sites remain unavailable. It rejects cycles, stale canonical
identity and inconsistent depth/site joins. Limits are 32 frames, 256 name bytes
and a 64-KiB report, within the existing cumulative logical capture budget.
The default origin path is unchanged. These diagnostic frames are not LLVM
inlinedAt, a physical allocator trace, authenticated portable source, or
per-authored-instruction microsteps. Pure tests and the separate real-source
macro campaign passed. The actual one-frame fixture joins the unchanged
canonical baseline and actual call/definition/expansion spans, with source
rechecks and stale canonical/callsite/false-inline refusals. Nested actual
expansions remain unqualified.
