# Shared gfx942 MOV/ADD Composition

Qualified on 2026-10-03 UTC over baseline
`f7f360004c223d182a75214322ed17925751fdb4`.

## Scope

The closed `Gfx942SMovB32V1` model decodes ordinary SGPR/integer-source MOVs,
cross-checks their exact retained trace and executes a shared Rust/Verus body.
Under valid-index preconditions, MOV reads the old source, writes exactly its
destination, returns the complete bit pattern and preserves SCC and every
other register.

`Gfx942MovPrefixAddU32V1` accepts a complete contiguous same-block interval of
at most 64 instructions: zero through 63 MOVs followed by one ADD. Unsupported
instructions, split boundaries, cross-block intervals and excess length reject.
The actual shared iterative origin fold and execution fold refine the same
ordered recurrence. The theorem establishes conditional result/carry, SCC and
the complete register frame. Origin calculation is O(102 + n); execution is
O(n). Origins are derived, not stored as caller-supplied claims.

A separate borrowed verifier API retains the exact compiler-input and analyzer
execution owners. It checks the existing MIR/KIR checked-add profile against the
derived entry register and literal, with reaching definitions at the span start.
The old single-ADD API is unchanged. Source-local/KIR equality, KIR/entry-SGPR
equality, reaching the span with those values and continuation beyond the ADD
remain explicit unresolved obligations.

These theorems do not prove decoder validity, ISA conformance, compiler lineage,
ABI, EXEC masks, memory effects or whole-kernel refinement. No production Worker
proof provider, application permission or load/launch authority is introduced.

## Qualification

- The complete selected kernel-analysis/verifier suites pass 832 tests, with
  zero failures and zero filtered tests. The exact 845-name roster adds 20
  tests to the prior checkpoint: 19 ordinary tests and one explicitly ignored
  MOV LLVM-MC test. All 12 previous ignores are preserved, giving 13 total.
- All 30 compile-fail doctests pass. New tests cover every ordinary destination,
  aliases, literals, full register frames, bounded spans, exact boundaries,
  unsupported operations, entry definitions and ambiguous loops. Two public
  tests retain authenticated TEST-worker owners, execute the composed model
  and reject substituted intervals or inconsistent encodings.
- LLVM-MC 18.1.3 and AMD LLVM 22.0.0git each pass both explicitly selected ADD
  and MOV assembly/disassembly tests. Executable hashes and versions match
  before and after; these runs use the already-qualified library executables.
- Strict Clippy passes for both libraries, binaries and the checked-add public
  integration test. No-default kernel-analysis and all-feature host checks,
  all 32 source-control commands, local CI dispatch, scoped formatting and
  whitespace checks pass. This is not an all-target Clippy claim. The shared
  test-worker fixture retains its pre-existing whole-file formatting exception.
- Two whole-crate Verus positives each report 16 verified obligations and zero
  errors. That count includes constants, derived implementations, getters and
  loop obligations, not 16 independent instruction theorems.
- All 21 mutants are rejected at their declared logical obligations: three
  arithmetic, five ADD-state, five MOV-state and eight composition mutants.
  Sixteen fail postconditions and five fail exact loop invariants. Qualified
  function, contract and macro locations are checked; parsing, overflow,
  translation, timeout and unrelated failures do not count as acceptance.
- All 17 runner controls pass. Both measurements match the pinned 190-file,
  129,019,839-byte Verus closure. All 26 campaign process groups close and the
  production, retained and staged proof inputs remain unchanged.

The independent auditor binds exact commands/environments, source snapshots,
45 executable identities and 40 test harnesses, the complete roster and ignore
delta, native tool measurements, staged mutant bytes, proof diagnostics and
the source-only candidate patch. Prior solver runs do not replace this fresh
integrated campaign.

## Diagnostics And Limits

The diagnostic build and CPU command succeeded while proof files were still
changing; their capture exits 125 and they are excluded from acceptance. The
diagnostic Clippy command passed unchanged but is also not selected evidence.
Every accepted final capture uses suffix `01`. No failure was relabeled.

No MI300X resources were used. LLVM-MC is CPU assembly/disassembly, not GPU
execution, and its trace envelope is synthetic. Authenticating a test worker
does not authenticate a production compiler or analyzer. Earlier multi-GPU
results remain historical, not fresh hardware qualification of this tree.
There is no new performance, overlap, native-failure or HIP/HSA parity claim.
A3 remains incomplete and A1/A2 remain parked.

Next: reuse compiler-owned source occurrence and actual KIR emission capture
for an exact borrowed source-local/KIR join, then prove its value invariant.
Physical span-entry equality, ABI/memory/completion composition and production
Worker admission still require their own evidence.

## Evidence

The [raw archive](raw.tar.xz), [member manifest](raw-manifest.json) and
[checksums](SHA256SUMS) retain controllers, candidate patch, commands, outputs,
diagnostics and independent audit. The read-only auditor expects this baseline,
staged candidate and retained executables; it does not audit arbitrary later
HEADs. No toolchain, GPU executable or vendor ISA document is redistributed.
