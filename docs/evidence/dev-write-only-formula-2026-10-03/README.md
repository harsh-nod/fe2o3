# Write Only Fill Formula Qualification

This checkpoint advances the first verified multi-GPU Rust fill workflow. It
enables actual per-output formula verification, but does not complete aggregate
source admission, machine refinement, native application admission, or HIP/HSA
parity. The complete V9 handoff acceptance test still fails closed at dynamic
launch coverage.

## Implementation

The compiler recognizes the authenticated `WriteOnlyDisjointSlice` output type
in reference signatures without treating it as readable initial memory. For the
closed Index1d write profile, the expression projector reads the actual call's
value argument and tracks the invocation coordinate through authenticated index
intrinsics. It checks exact allocation origin, output-argument custody, unique
definitions, address escapes, and definition dominance at each use. Call results
must dominate through their successful return edge. Index and receiver checks
also apply when the stored value is constant. Unannotated kernels do not acquire
this additional reference-expression walk.

The protected proof controller previously slept 2 ms whenever a resumed tracee
had not immediately stopped again. Live observation showed thousands of advancing
ptrace stops, including non-executable `mprotect(..., 4096, PROT_READ|PROT_WRITE)`
requests. Adaptive polling now starts at 50 microseconds and doubles to the
existing 2 ms idle cap, resetting after each observed event. Exact-TID waits,
syscall checks, descriptor and executable checks, deadlines, output bounds and
rejection cleanup are unchanged.

Actual execution also exposed an unconditional uninterpreted IEEE declaration
that the pinned verifier rejected under `--no-cheating`, even for integer proofs.
Integer formulas now omit it. IEEE operator-congruence lemmas instead quantify
an arbitrary `spec_fn` parameter; aggregate replay threads the same parameter
through affected lemmas. This proves congruence for every interpretation, not
IEEE target-value semantics. Unused native stubs were removed from generated
library proof sources. The fixture campaign now also uses `--no-cheating`.
Unexpected proof results retain bounded, escaped stdout/stderr prefixes while
success still requires an exact nonzero verified count and empty stderr.

## Qualification

Parent commit: `d11f11d28483ddb506b580233a4aa9c7711d6a42`.
Tests use `nightly-2026-04-03`, four Cargo jobs, no incremental compilation,
test opt-level 1, enabled debug assertions and overflow checks, and serial tests.

- Compiler library: 520 tests passed, including value projection and mutation
  cases covering dominance bypass, changed origins, redefinitions, escapes,
  shifted indices, types and predicates.
- Verifier library: 137 passed, 6 ignored. Hostile process-tree checks still
  reject descriptor leaks, unexpected descendants, executable-map substitutions
  and deadline overruns.
- Protected generated-formula test: passed in 63.93 seconds. It executes three
  positive/negative pairs: wrapping integers, constant IEEE expressions and
  symbolic IEEE expressions. Every negative requires an actual assertion failure,
  not a parser, runtime-admission or no-cheating-policy rejection. The transcript
  records the combined test, not six independently exported proof receipts.
- Genuine AMD extraction: both proof-gate tests passed in 82.10 seconds. The
  positive fill reaches the separate dynamic coverage rejection; the changed
  reference value fails an actual Verus assertion. Neither emits a handoff.
- Strict library Clippy passed for the compiler and verifier with warnings denied.
- Four AMD reference-binding regression tests passed. These include the actual
  write-only output-read rejection; existing positive binding cases deliberately
  stop at the unavailable runtime on the ordinary host, not at a proved result.
- The pinned aggregate fixture campaign passed with `--no-cheating`: single-
  and two-output positives (4 and 5 verified obligations) and three expected
  proof failures. Targeted Rust formatting and whitespace checks passed.
- Protected trivial proof: 85.51 seconds before adaptive polling, 10.27 seconds
  afterward. These are single-run observations on the same local host, not a
  statistical benchmark or a GPU/HIP/HSA performance comparison.
- Earlier genuine fill attempts are failures, not qualification successes: first
  a 60-second proof timeout, then the forbidden uninterpreted declaration, then
  `FE2O3-OWN-002` after the per-output formula succeeded. No attempt emitted an
  admitted handoff or granted launch authority.

[Qualification archive](qualification.tar.xz), SHA256
`654ff50fa368d46b4c32dc69b4e0002ecb16a963c041b62893234d4de9e3344a`,
contains the candidate code patch, test transcripts, host-check commands and exact
isolated-runtime harness. The harness records this host's paths, not a portable
deployment installer. The earlier timing observations and failure history above
are development notes; their complete raw transcripts are not in this archive.

## Exact Runtime Environment

The original production runtime manifest remains unchanged, SHA256
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
Its pinned Verus distribution and Rust 1.97.1 toolchain were reconstructed under
a private root-owned directory and exposed at the required `/opt` path only in
an isolated mount/PID/IPC/UTS/network namespace. The host's installed libraries
and `/opt` were not changed. Proof controllers ran as UID/GID 1000 with no groups,
capabilities or inherited seccomp filter. No GPU devices were exposed.

Additional exact inputs, downloaded and extracted rather than installed:

- [Rustup 1.29.0 archive](https://static.rust-lang.org/rustup/archive/1.29.0/x86_64-unknown-linux-gnu/rustup-init):
  `4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10`.
  This provenance input was not executed.
- [Ubuntu glibc 2.39 0ubuntu8.8 archive](https://snapshot.ubuntu.com/ubuntu/20260801T000000Z/pool/main/g/glibc/libc6_2.39-0ubuntu8.8_amd64.deb):
  `3b8d5391b6b484a4c81fd000b6064885ad967ec3cb966bc57603f3fb3ebf0ed5`.
- [Ubuntu zlib 1.3 3.1ubuntu2.1 archive](https://snapshot.ubuntu.com/ubuntu/20260801T000000Z/pool/main/z/zlib/zlib1g_1.3.dfsg-3.1ubuntu2.1_amd64.deb):
  `7074b6a2f6367a10d280c00a1cb02e74277709180bab4f2491a2f355ab2d6c20`.

The unchanged provisioning script accepted the exact closure. This environment
is development qualification, not a deployed compiler-origin or application
authority provider. Extraction tests retain synthetic invocation metadata and
explicitly grant no runtime authority.

The private root-owned runtime tree was removed after protected tests finished.
Only this task's scratch was cleaned; no shared MI300X files were removed.

## Next Multi GPU Work

1. Prove conditional total coverage from the actual guarded identity-store graph.
   For output length N and authenticated global extent G, N <= G must imply one
   writer for each index below N and no outside writes. Bind this to the exact
   output ABI argument and actual launch. Workgroup size 64 is not grid size;
   unknown dimensions must not be replaced with 64 or declared proved.
2. Check the retained source, neutral KIR and independently replayed optimized
   KIR for the complete guarded fill relation, then join the existing 14-instruction
   machine model and physical dispatch coverage. Stored-value checks alone do
   not establish guard or indexed-address equivalence.
3. Supply the protected production application provider and current-publication
   binding. Reuse existing preparation, completion, staging, tracked upload and
   native peer transfer for GPU A to GPU B readback.
4. Qualify lengths 64, 65 and 4097 in both directions on freshly observed idle
   devices, including destination guards, result credits, peer counters, stale
   publication rejection and cleanup. No MI300X GPU job ran in this checkpoint.
