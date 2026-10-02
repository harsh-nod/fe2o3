# Local gfx942 Integer Qualification

Status: independent CPU, arithmetic and LLVM-MC qualification audit accepted
on 2026-10-02 UTC. Baseline:
`05457de5db1ec371cfe3d893e925a04f052d8295`.

## Implemented Scope

The [component](../../gfx942-local-integer-refinement-v1.md) adds a closed
encoded `S_ADD_U32` model, a shared executable arithmetic body and a borrowed
conditional MIR/KIR/machine obligation. Unsupported encodings, trace effects,
operand types, literal mismatches and ambiguous definitions reject. KIR inputs
must be unique u32 block parameters dominating the checked addition; the exact
constant-plus-checked-add statement span remains required.

The obligation retains both original owners and explicitly leaves semantic
local-to-SSA and SSA-to-register equality unresolved. Its source receipt does
not authenticate compiler origin. It grants no load, launch, compiler-refinement
or application authority.

## Qualification

The independent audit checks all 6,091 current source files and 45 executable
identities: 40 test harnesses and five fixture binaries. The complete selected
kernel-analysis/verifier CPU roster has 823 tests: 811 pass, 12 are explicitly
ignored, and none fail or are filtered. The increment adds 18 ordinary tests
and one explicitly selected LLVM-MC test. All 30 compile-fail doctests pass.

The native test passes once with Ubuntu LLVM 18.1.3 and once with AMD LLVM
22.0.0git. Each run assembles and disassembles eight cases, checking actual
instruction encodings, opcode names and displayed operands. Both commands use
the same feature-unified library executables as the ordinary test campaign;
the verifier library has zero matching tests. The two assembler executable
hashes and version output are unchanged before and after these runs.

These are native CPU-tool compatibility checks, not GPU execution. Trace
envelope flags and SCC metadata remain synthetic. The public integration tests
authenticate execution of a test worker, not the production LLVM analyzer.
Neither observation supplies native analyzer or loader-closure authority.

Strict Clippy passes for both production libraries, binaries and the new public
integration test. The no-default kernel-analysis check and all-feature host
integration check pass. All 32 source-control workflow commands, the local CI
dispatch test, scoped Rust formatting and whitespace checks pass. Existing
formatting in the shared machine-worker fixture is the sole explicit formatter
exception; unrelated formatting was not rewritten.

The final selected captures use suffix `04`, with CI retry `final-ci-04-rg`,
source campaign `final-source-controls-04` and proof output `proof-final-04`.
All selected source snapshots match each other and the audited working tree.

## Arithmetic Proof

For all pairs of u32 inputs, the identical Rust/Verus macro proves wrapping
addition, exact SCC overflow and reconstruction of the mathematical sum from
result plus carry. Both positive runs verify one function with zero errors.
All three required executable-body mutants fail their intended postconditions:
discarded carry, incorrect result width and substituted result. Compilation
errors and timeouts are not accepted mutant outcomes.

Five runner controls pass. Both release measurements match the pinned Verus
closure: 190 files and 129,019,839 bytes. Every solver stage has a terminal
receipt and absent owned process group. This proves arithmetic only, not the
decoder, alias/frame behavior, ISA correspondence, CFG, ABI, memory effects,
compiler pipeline, whole kernel or runtime adapter.

The proof is wired into the runtime-model workflow and local Verus gate. This
record does not claim a new run of the entire pre-existing runtime proof suite.

## Diagnostics And Corrections

- Earlier check/build captures completed during source changes and are not
  final-source acceptance evidence.
- The first test run exposed a same-block assumption: lowering legitimately
  reuses a dominating loop-header parameter. The corrected checker verifies
  actual dominance without assuming a semantic-local equality. Its loop
  ambiguity fixture now includes a reachable conditional exit and asserts the
  real LiveIn plus loop-carried definitions before testing rejection.
- The next run exposed an existing debug-section test that equated debug
  assertions with emitted debug information. The test now permits absence of
  the optional fixed marker while retaining checkout-path rejection and every
  canonical-section check. Explicit marker-present/absent controls remain.
- Historical all-target Clippy fails on an unchanged `option_env!().expect()`
  helper. Shared-fixture dead-code warnings also remain in old harnesses. No
  blanket lint suppression or all-target Clippy pass is claimed.
- A standalone native diagnostic exhausted its compile timeout after feature
  unification differed from the main build. Final native commands select both
  packages and the audit requires their exact already-qualified executables.
- The initial CI attempt lacked `rg` in the clean PATH. Its retained retry adds
  the installed, version-specific tool directory for that command only.
- Development proof attempts retain the initial arithmetic expression and
  solver-result classifier failures. Final campaigns use the explicit modulo
  body and strict observed result schema, with unchanged theorem predicates.

The verifier's new kernel-analysis dependency adds one Cargo.lock edge. The
source-control metadata refresh changes six SHA literals and one manifest row
across seven files. Independent baseline comparison confirms all predicates,
expected counts, 22 executable proof-closure files and 2,091 runtime-domain
files remain unchanged. A metadata refresh is not new proof evidence.

## Boundaries And Next Work

No GPU or MI300X resources were used in this increment. There is no new native
multi-GPU, fault-isolation, overlap, scaling or HIP/HSA performance result.
A3 and issue #182 remain incomplete, and A1/A2 remain parked.

The next practical multi-GPU priority is native checked peer subranges for
initialized PUBLIC allocations, preserving whole-owner custody. The retained
`next-multigpu-step.md` specifies lower/runtime boundaries and full-buffer
sentinel qualification; it is a plan, not implemented functionality. Broader
application authority and native failure isolation remain separate open gates.

## Evidence

The [raw bundle](raw.tar.xz), [member manifest](raw-manifest.json) and
[SHA256SUMS](SHA256SUMS) retain commands, environments, source/executable hashes,
rosters, outputs, exits, diagnostics and the independent audit.
The ISA reference URL/hash is recorded; the AMD PDF is not redistributed.

| Record | SHA-256 |
| --- | --- |
| Independent auditor | `17e33db83061f484c70c25d20584517975758da901f4e197255b7f427dcb2c1a` |
| Selection | `5019eda122157ad19fd64cf0cbfeaf8648a08d71c5a8b8f3f1cc6ca24c8de2c2` |
| Accepted audit | `b94167c15359deefdba8f7e2bf027d9ec6ef4bd7cb60297f7c7ee10c498d63b0` |

Run the retained auditor with `python3 -I -B audit.py`. It requires the recorded
baseline workspace, candidate sources, toolchain and retained executables; this
is an evidence record, not a portable build environment or audit of later HEADs.
