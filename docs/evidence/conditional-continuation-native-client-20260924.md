# Conditional Continuation And Native Client

This is a prerequisite checkpoint for #272, not completion of a roadmap
milestone or the 47-kernel end-to-end qualification target. The new conditional
continuation and diagnostic native client do not activate safe GPU launch.

The subsequent [conditional-formula and native-preparation checkpoint](conditional-formula-native-preparation-20260924.md)
records later implementation and its separate validation scope; it does not
retroactively extend the results below.

## Conditional Compiler State

The consuming lower-MIR continuation retains the original canonical source,
ranked graph and work account. It replays source correspondence, derives output
coverage, checks read occurrences, and stages an explicitly conditional
aggregate request. The owner retains its original account; budget-view
substitution is detected and rejected on callback return. The callback cannot
recover a mutable graph or convert conditional evidence into an ordinary clean
report.

The request states concrete premises: a D1 launch, output length within the
launch, writable output, readable input spans, input/output separation, and
representable aligned address arithmetic. Canonical parameter numbers have an
explicit checked correspondence to source arguments; they are not assumed to
be host or CPU-reference ABI ordinals. Address formation outside the output
guard retains the global-launch domain even when the output is empty.

Final-graph checks retain ordinary bounds, trap, effect and value obligations.
A readable-input premise does not suppress a failing input-bounds proof.
Value-less writes and disagreement with the checked store expression reject.
Graph identity and mutation epoch are checked again before aggregate replay.
Accepted work charges are never refunded. Retained input capacities are charged
before source replay and released only after their owners are dropped.
Preexisting source-correlation work and recipe/arena allocations retain their
separate accounting domains; this is not complete allocation or RSS accounting.

The Pliron tests include canonical-IR fill and read-expression fixtures with
independently proved bounds. Lower-MIR tests cover source-root refusals, read
occurrence mutations, and resource limits. These are not a positive
actual-Vecadd end-to-end test.
The backend CPU-reference join, protected conditional aggregate proof consumer,
descriptor premise transport and safe host discharge still need integration.
No aggregate Verus receipt or launch authority is produced by this API.

## Diagnostic Native Client

`CompilerExecutionClientV2` consumes a connected unnamed `SOCK_SEQPACKET` peer
and borrows the original resource account through its terminal exchange. It
supports receipt acquisition and journal-stage recovery, plus authentication
against a fresh currentness challenge. Packet correlation, policy, signatures,
anchor position and exact carriage bytes are checked without a V1 retry or
native-to-legacy owner conversion.
Receipt acquisition does not establish currentness; currentness authentication
is a separate consuming operation.

The durable-directory native API shares the existing descriptor-relative
transaction engine, with metered positional I/O, exact content and namespace
checks, and work and scratch quota checks before I/O. Short transfers and EINTR
fail closed; accounting does not bound syscall duration. The unused private
native-service recovery adapter is not included: no protected native issuer or
Worker journal has been activated. The production service remains V1. Signed test transcripts
do not establish protected key custody, live compiler observation, independent
anchor administration, or commit-before-publication by a real native service.

## Validation Scope

Validation used pinned `nightly-2026-04-03`, locked offline dependencies, one
build job, and no GPU. The final focused and documentation runs used merged code
commit `bc746b5ed177305bac73c687118ed86c64e46833`, including the concurrent
debugger/simulator update `07547cd4b`. Source and tool snapshots remained stable
through each guarded run. Only this evidence note was completed afterward.

| Completed scope | Result |
| --- | --- |
| Canonical kernel-IR library suite | 759 passed |
| Full Pliron library suite, `internal-proof-staging` enabled | 1,568 passed |
| Final merged-tree conditional, consuming-continuation and native-storage tests | 180 passed: 25 kernel-IR, 74 lower-MIR, 78 Pliron, 3 storage |
| Artifact transaction and broker suites | 695 passed; 6 ignored entries (privileged fixtures and subprocess helpers) |
| Client, protocol and issuer suites, including their doctests | 247 passed; 5 ignored entries (isolated/static fixtures and subprocess helpers) |
| Final artifact, broker, kernel-IR, lower-MIR and Pliron doctests | 327 passed |
| Tutorial-manifest Python suite | 79 passed |

The full Pliron run preceded the unrelated debugger/simulator merge; all 78
conditional Pliron tests passed again in the merged-tree run. The full lower-MIR
suite was interrupted while running existing resource-exhaustion tests and is
not counted as a pass. Its 74 selected tests completed successfully. An initial
client run exceeded the Unix socket pathname limit; the complete client,
protocol and issuer rerun used a short private temporary directory and passed.
The counts above do not add partial or repeated runs to completed-suite totals.

The affected compiler/service all-targets check, Rust formatting, workspace
dependency policy and source-hygiene delta check passed. Six-crate all-targets
Clippy completed with warnings; strict `-D warnings` stopped in unchanged
local-frame analysis. This is not a warning-free workspace claim.

The final selected test command was:

```sh
cargo test --locked --offline \
  -p fe2o3-kernel-ir -p fe2o3-lower-mir-kernel -p fe2o3-pliron \
  -p fe2o3-artifact-transaction \
  --features fe2o3-pliron/internal-proof-staging --lib -- \
  conditional consuming_continuation native_record --test-threads=1
```

Full-Pliron log SHA-256:
`d18f8b920fd9ff3d6e099ddc426540b8c51bed7a9e0fb8cd63d1b5077fb94aa6`.
Final selected-test log SHA-256:
`198a9b3da173e3ef4db0f11e7adb652a2a8bca2a9f04ee8f11536706e47eb890`.
Final doctest log SHA-256:
`f2cbf6484085b5be94c769b33f3c89ee0c4ec96c1ffebfb047b73dd5e1968978`.

The separately recorded [actual-source protected fill proof](manifest-fill-source-proof-20260924.md)
ran on its frozen compiler snapshot; its result is not transferred to this
changed compiler. Tutorial input hashes are refreshed without changing the
original 47 obligations or any qualification status. The merged-tree manifest
validator still reports `qualified=false`, pending semantic/policy checks, and
the existing `gemm-proof-plan` source-binding gap. No #272 milestone is closed.
