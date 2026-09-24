# Native Finalization And Replay Checkpoint

This is an implementation and CPU-validation checkpoint for #272, not milestone
completion or 47/47 end-to-end qualification. No GPU kernel was launched and no
new protected semantic proof was executed in this batch.

## Implemented Boundary

- Native Worker evidence retains the complete V4 source and lowering owner
  through the shared HSACO lineage, ELF/AMDHSA, descriptor and ABI checks.
  Legacy Worker identities and descriptor-version separation are preserved.
- Compact native replay binds the original occurrence, source, Worker evidence
  and finalization under separate domains. Recovery rederives the inert V4
  receipt, reconstructs both exchanges and repeats structural finalization.
  Fresh consumption and recovered replay remain distinct custody states.
- Durable native records reuse the existing journal and artifact storage.
  They do not convert into legacy publication authority or authorize loading.
- Native root/socket/pidfd service admission and V2 service codecs retain
  cumulative resource accounting without enabling a production issuer.
- The fill tutorial has an independent CPU reference and an optional,
  nondefault `reference-proof` feature. Constant point-output joins preserve
  `u32` versus `f32` types and exact representation bits. Default onboarding
  remains available without the protected runtime.

The native ledger does not subsume the inherited bounded artifact parser,
LLVM/process allocations or RSS. Resource tests establish the represented
budgets, not a whole-process memory bound. Public-key fixtures, deterministic
replay and internally consistent HSACO bytes are not execution authority or
semantic-to-machine proofs.

## Validation

Builds used nightly `2026-04-03`, locked offline dependencies, one Cargo job,
disabled incremental compilation, a 12 GiB per-process virtual-memory ceiling,
nice 10, and a 1200-second outer timeout. Guard records measured source and
tool hashes before and after each build; all completed validation records had
stable inputs. This is scoped validation, not a complete workspace test run.

| Check | Result |
| --- | --- |
| Artifact transaction library | 264 passed |
| Broker authority library | 251 passed; 6 environment-specific tests skipped locally |
| Execution protocol library | 54 passed |
| HSACO finalizer library | 176 passed |
| Explicit native finalization/replay integration matrix | 15 passed, including all four Direct/Erased and gfx942/gfx950 source owners |
| Legacy first-build and finalizer admission regressions | 44 passed; 3 real-Worker tests skipped |
| Doctests across those four libraries | 129 passed |
| Compiler conditional-reference unit tests | 11 passed |
| Fill harness unit tests | 3 passed; 6 explicitly gated integrations not selected by this unit run |
| Binding-aware fill CPU tests | 2 passed in each feature mode |
| Actual `quickstart.sh no-gpu` | Passed exact output and canary comparison |
| Default fill source regressions | 2 passed, including quickstart cleanup and eight boundary-length simulations |
| Pre-proof source/output preparation | Passed integer and float cases on gfx942 and gfx950 |
| Missing-runtime source regression | Passed; absence was never counted as proof success |
| All-target check of 11 production packages | Passed |
| Unsafe-source policy | 5 passed; explicit baseline-refresh command skipped |
| Manifest source contracts | 3 fill tests and original-47 obligation preservation passed |
| Formatting and dependency policy | Passed; 140 members, 8 layers, 503 internal dependency declarations |

The all-target check covered the compiler backend, Cargo CLI, finalizer, runtime,
execution issuer/client/deployment/supervisor, broker, host and debugger CLI.
It retains the backend's unused-code warnings; it is not a warnings-free claim.
The earlier full Python manifest suite timed out and is not a recorded pass.

Protocol and broker libraries passed `clippy --lib --no-deps -- -D warnings`.
Strict transaction linting exposed three existing `too_many_arguments` findings
in the unchanged `compiler_module_handoff.rs`; its rerun passed with that one
command-line allowance. Finalizer linting passed with the existing
`large_enum_variant` and `needless_question_mark` allowances after fixing a new
doc-list formatting error. No new source-level lint suppression was added in
response to these checks.

The four initial library suites used the pre-reconciliation snapshot. Main
commit `ed69ea8c845a5d13adbaceedf1b6b3321217d6ed` was then fast-forwarded without
overwriting this batch. The source/CLI checks, legacy regressions, doctests,
downstream checks and policies ran on the combined tree. Native integration was
rerun after the comment-only lint correction: all 15 cases passed again. Final
documentation edits followed the frozen build/test runs.

The final native matrix's measured source snapshot was
`f2d0601d320e418c2a49f12b595bd51b8569cc0b877ea0d9c8113522fb4013de`.
The guard hashes sorted tracked and nonignored untracked paths with their
content digests; this is not a Git tree ID. The matrix covers exact and one-short
budgets, full returned storage charges, fresh/restart equality, checksum-valid
coordinate substitutions, changed artifacts/descriptors/ABIs, foreign producers
and undeclared provider attachments. Its Worker and signed inputs are synthetic
test fixtures, not the production LLVM Worker or protected proof results.

## Shared-Host Execution

One explicitly selected distinct-UID broker-admission test passed on `mi350`.
It used an isolated Ubuntu 24.04 container with no network or GPU devices,
read-only root and test executable, private namespaces, default seccomp,
no-new-privileges, only the required UID-management/test capabilities, one CPU,
1 GiB memory and 64 PIDs. Container exit was zero, with no OOM. The exact owned
container and scratch directory were removed and their absence checked.

The tested broker executable SHA-256 was
`2146dce77a0127c794d6b5a8bf7c5fb3dd425c0e75d6906cb47bd45f728f3b5b`.
This checks OS admission and revalidation only, not signing, deployment,
protected Verus execution, artifact authorization or a GPU launch.

The separate pinned-toolchain transfer for the actual fill proof test timed out
after 900 seconds before a proof container started. Its partial private scratch
directory was removed after confirming the transfer had stopped. No shared
runtime files or volumes were modified. The protected positive, mutation and
post-bind tests therefore remain unexecuted, not passed or semantically failed.

## Still Required

Normal production still needs the protected native issuer, signing and durable
ACK/currentness joins; source-bound conditional ownership continuation; required
aggregate proof and semantic-to-machine refinement; and authorized Cargo,
finalizer, runtime and generated-host consumers. The actual-source positive and
wrong-store proof tests must run against the admitted protected runtime. See the
[fill proof prerequisite](manifest-fill-source-proof-20260924.md) for the exact
test boundary and reproduction commands.

These components must integrate into the one production pipeline for all kernel
families. Library tests, simulation and synthetic Worker transactions do not
replace the required 47 production proof-to-safe-GPU-launch runs.
