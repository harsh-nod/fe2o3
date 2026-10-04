# Reviewed Macro Admission

Source: `cf004d54ff68c40006d5e0cee2bf478e6848e591`.

The workspace-local macro allowlist now matches the independently reviewed tree.
This removes the two inherited Cargo source-pin failures; it does not implement
authenticated application registration, remote proof custody, or native authority.

## Review And Changes

Read-only agent review covered the nine macro commits since the previous pin.
The production delta adds owned runtime arguments and ordinary Context arguments,
without new proc-macro dependencies or I/O. This checkpoint changes no production
macro emission. It adds an independent complete four-item adapter oracle for an
eight-argument mixed signature: exact 104-byte ABI, access/alias/type identities,
full-signature ordinals, distinct mapped indices, ordered accounting, supplied
budget, consuming construction/binding and both unsafe trait impls.

Nine post-generation AST mutation controls exercise omissions, duplication,
argument/mapping substitution, added unsafe/Clone code, removed layout fields and
a nonliteral doc attribute. These are oracle sensitivity tests, not compiled
production-source mutants or new formal proofs. Four mixed unsupported signatures
must emit no adapter. Existing downstream tests cover the separately emitted
borrowed argument struct and typed ownership failures.

Offline metadata refreshed the standalone generic fixture lock: six existing
workspace dependency edges, no new packages, versions, sources or checksums.
Only the workspace-local source pin changed. Its qualified canonical digest is
`294470f966254a81cf2c1032294a850ee834087d973fba1350313e8e61b84ba8`.
The independent external-source pin is unchanged.

## Qualification

| Check | Result |
| --- | --- |
| Macro library | 67 passed |
| Typed and renamed downstream fixture groups | 11 passed |
| Host generated-runtime selection | 109 passed, 1 hardware test ignored |
| Cargo main binary | 400 passed, 5 deployment/hardware tests ignored |
| Rustc wrapper and linker proxy | 43 passed |
| Macro and Cargo all-target strict package Clippy | Passed with `--no-deps -- -D warnings` |
| Formatting, whitespace, source/binary audit and signature | Passed |

The downstream harness also requires all five public Context runtime fixture tests
to pass. Both exact-local/git identity and complete-workspace-pin tests now pass.
Sources were unchanged during each qualification stage; the only interstage source
change was the reviewed local pin. The archive includes exact commands, snapshots,
binary hashes, review scope and logs. An initial Clippy run found a needless Box
replacement in a mutation test; it was corrected and the full sequence rerun.

Runs used nightly-2026-04-03, offline locked dependencies, four build jobs, one test
thread, disabled HIP discovery/incremental/debug info, test optimization 1 and enabled
debug/overflow checks. No GPU, production-service deployment, or performance run
was performed. This pin does not newly authenticate host/runtime implementations.
The four owned RAM-backed fixture caches and their links were removed and verified;
normal build caches and unrelated user files were preserved. Scratch is removed
after packaging and publication.

[Evidence archive](evidence.tar.gz) SHA-256:
`fdf8fa713437fe5e34186b2ce4d350eb4db5f95c1757a84571a025efe92d8a55`.
Source patch SHA-256:
`0333cd0f11109aa91f56e2eb467576505f52e43e8d87f5c7d6227a918f5d8dc9`.
The archive manifest and archive-to-source comparison passed.
