# Genuine Rust Checked-U32 Prefix Extraction

Baseline: `f1788c950cfb03404841f47521db75f3fa0b3672`.
This checkpoint connects ordinary safe Rust compilation to the same retained
semantic MIR/V8 KIR owner and existing entry-prefix checker. It changes neither
native multi-GPU transport nor production kernel-launch admission.

## Implemented Scope

The explicitly selected wrapper mode preserves existing metadata, overflow and
passthrough rules and rejects competing extraction modes. Inside the live rustc
callback, a pre-SSA transaction selects one actual helper-entry checked add and
moves the original semantic owner once into capture-aware lowering. Existing
root reachability, type/profile, replay and exact-V8 checks remain in force.
No guessed function IDs, detached correspondence map, serialized owner or
reference-policy source annotation supplies the capture.

The report records actual source/KIR identities, argument bindings, source/SSA/
KIR coordinates and conditional value/overflow samples. It explicitly disclaims
normalization proof, compiler authentication, continuation and launch authority.
The owner is dropped before its retained capture-storage charge is released.

## Qualification

MI300X was used only for CPU compilation and tests with four build jobs at nice
priority 15. Toolchain: `nightly-2026-04-03`, rustc commit
`55e86c996809902e8bbad512cfb4d2c18be446d9`, LLVM 22.1.2.

- Compiler library: 517 passed. Extractor wrapper: 18 passed, including three
  new mode/isolation tests. The normal integration invocation reports ten
  explicit compile-environment ignores; it is not counted as executing them.
- The exact new ignored integration test passes all six real compiler cases.
  The first accepted helper retains two copy statements and resolves to argument
  0; the second has an empty prefix and resolves to argument 1. Both shapes are
  required by the test, as are distinct source/KIR hashes and carry-boundary
  results. Missing site, ambiguous site, multiple roots and unsupported prefix
  each fail with their expected diagnostic and no accepted report.
- Existing default extraction and unsafe-source rejection integration tests
  pass separately. The latter retains all three existing source-safety cases.
- Strict compiler-library/wrapper Clippy, changed-module formatting, both shell
  syntax checks and CI dispatch regression pass. The existing ROCm compile lane
  selects the exact module-qualified ignored test. GitHub Actions execution,
  all-target Clippy and a workspace-wide suite are not claimed.

`qualification.tar.xz` retains full outputs, exact commands/environment, source
inventories, compiled artifact/dynamic-library identities, source patch and the
receipt auditor. All accepted runs use the same final source cohort. Earlier
attempts remain excluded diagnostics: an incorrect hash-formatting API call and
a dispatch-test environment mismatch caused by the compiler loader override.
The latter passes with that override removed; neither failure led to weaker
admission checks. Earlier positive runs are not substituted for final-source runs.

No new solver execution is claimed. The shared fold/add proof sources and
lowerer/checker remain unchanged from the baseline; genuine compiler extraction
is tested here, not newly proved. The two actual prefixes do not establish
general Rust semantics, machine entry, ABI/EXEC, memory, continuation or protected
per-invocation compiler/verifier/finalizer custody.

The owned remote build/scratch directory and its copied files were removed after
log retrieval and process-absence checks. No GPU execution, reset, fault injection,
performance measurement or cleanup of unrelated work occurred.

## Next Gate

Refine the actual normalization adapters into the shared fold contract, followed
by semantic-to-machine and protected invocation admission. A3, issue #182 and
HIP/HSA parity remain incomplete. Already-admitted native copy and finite-compute
multi-GPU workloads remain usable independently of this diagnostic mode.
