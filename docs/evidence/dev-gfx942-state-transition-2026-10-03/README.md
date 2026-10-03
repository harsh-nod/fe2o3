# Shared gfx942 Register-State Transition

Qualified on 2026-10-03 UTC over baseline
`050fb4e7128f3104bd7e4d9ce223d642d4fbe713`.

## Scope

The actual `Gfx942SAddU32V1::execute` and Verus use one shared executable body.
For valid instruction register indices, it proves old-state operand reads,
including aliases; modular result and exact carry; the returned result; exact
destination update; SCC replacement; and preservation of all other registers.
The checker binds both models' exact 102-SGPR/SCC, source, instruction and
result schemas, includes and forwarding bodies. The decoder and public
authority boundaries are unchanged.

This is a prerequisite for application-kernel admission, not an application
proof. Decoder validity, ISA correspondence, source/SSA/register entry
equalities, CFG, ABI, EXEC masks, memory effects and whole-kernel refinement
remain outside this theorem. The existing checked-add verifier records a
conditional instruction obligation; it does not yet compose modeled execution.
No new production Worker V3 proof provider or launch authority is introduced.

## Qualification

- The complete selected kernel-analysis/verifier roster passes 813 tests with
  12 unchanged explicit ignores, zero failures and zero filtered tests.
  Its 825 names are exactly the previous roster plus two alias/frame tests.
- All 30 compile-fail doctests pass. Both new CPU tests exercise the production
  execute method, including every ordinary-register destination and incoming
  SCC values, using independent `overflowing_add` and full-register oracles.
- LLVM-MC 18.1.3 and AMD LLVM 22.0.0git each pass the explicitly selected
  assembly/disassembly compatibility test. Tool hashes and version output
  match before and after; both use the already-qualified library executables.
- Strict Clippy passes for both libraries, binaries and the checked-add public
  integration test. Kernel-analysis no-default and all-feature host checks,
  all 32 source-control commands, local CI dispatch, scoped formatting and
  whitespace checks pass. This does not claim all-target Clippy coverage.
- Two whole-crate Verus runs each verify four obligations with zero errors:
  arithmetic, execute, the constant initializer and the derived source Clone.
  Three arithmetic and five state mutants fail genuine selected postconditions.
  State mutants cover early alias writes, wrong destinations, unrelated-register
  clobbering, stale SCC and wrong returned values. Parse failures, overflow,
  timeouts and unrelated diagnostics are not accepted negative controls.
- All 12 runner controls pass. The pinned 190-file, 129,019,839-byte Verus
  release closure matches before and after. All 13 campaign process groups
  close, and production, retained and staged proof sources remain unchanged.

The independent auditor binds the complete qualified source snapshots, exact
commands/environments, all 45 built executable identities (40 test harnesses),
the exact CPU roster and ignores, proof diagnostics, tool measurements and the
staged source-only candidate patch. No prior solver result substitutes for this
fresh integrated campaign.

## Diagnostics And Resources

The warm diagnostic build succeeded while proof files were still changing; its
capture correctly exits 125 and is excluded from acceptance. All selected final
captures use suffix `01`. An initial staging check ran before `git add` finished,
rejected unstaged source and wrote no candidate artifacts; candidate capture
succeeded only after staging completed. No failed test or proof was relabeled.

After all Cargo commands completed, 25 measured obsolete `.rlib` cache files
were removed from this checkout, reclaiming 336,322,560 allocated bytes.
Current artifact rosters, hashes and file identities were checked first;
current binaries, historical KFD executables and evidence were preserved.

No MI300X resources were used. LLVM-MC is a native CPU tool, not GPU execution;
its trace envelope is synthetic. Existing public tests authenticate a test
worker, not a production compiler/analyzer. Earlier multi-GPU evidence remains
historical, not a new qualification of this whole tree. No performance,
physical-overlap, native-fault or HIP/HSA parity claim follows from this work.
A3 and issue #182 remain incomplete; A1/A2 remain parked.

Next: a closed MOV model and bounded contiguous MOV-prefix/ADD composition,
while keeping source-to-span-entry equalities explicit. Native application
authority still requires the broader compiler, ABI and memory-effect chain.

## Evidence

The [raw archive](raw.tar.xz), [member manifest](raw-manifest.json) and
[checksums](SHA256SUMS) retain the controllers, exact source patch, commands,
outputs, diagnostics and independent audit. `audit.py` is read-only and requires
the recorded baseline workspace, staged candidate and retained executables; it
does not audit arbitrary later HEADs. The archive redistributes no toolchain,
GPU executable or vendor ISA document.
