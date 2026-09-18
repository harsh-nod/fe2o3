# Issue 275: Context Bridge And Checked-Output Composition

## Executed Candidate

- Candidate: `784a303aecba4afa9adcb095629ad9a1ee4c4fce`.
- Public parent: `a1672a9fcca237d314193add170413718b61bf58`.
- Before/after source fingerprint:
  `e3152e71c0780d430a1be772990c3ed764ff2074b4fc7ca8b09005d4573c6044`.
- This evidence-only child does not change the executed implementation.
- Host: `mi300x`; pinned `nightly-2026-04-03`, offline/locked Cargo,
  one build job/test thread, incremental disabled and shared build lock.
- Existing 8 GiB target cap and disk/memory reserves were retained.

This normal merge combines the authenticated context bridge with the published
silent-helper/private-memory checked-output path. Original semantic, ranked,
collector and context custody remain attached to the checked-output route.
Lowerer/verifier files are byte-identical to the public parent. The historical
509 qualification remains separately recorded in
[the bridge note](issue275-context-bridge-20260918.md); those results are not
relabeled as qualification of this candidate.

**No new scope or tile execution is enabled.** V29 materialization remains
closed. Source237/238 availability/CFG work is not imported here. Backend
provider-custody preparation is outside the tested tree and not implemented
by this commit.

## Fresh Combined Validation

| Gate | Result |
| --- | --- |
| Backend library / doctests | 884 passed, 11 default ignores / 2 passed |
| Explicit callback / callback-free source observations | 40 / 16 |
| Fill and vecadd actual-output oracle | 18 scenarios, each simulated twice |
| Scalar GEMM actual-output oracle | 11 scenarios, each simulated twice |
| Result-wrapper actual-output oracle | 16 scenarios across two configurations, each simulated twice |
| Private Unit helper actual-output oracle | 8 scenarios in one configuration, each simulated twice |
| Ordinary RustCall closure/slice source parent | Passed |
| Forged-marker / reachable-unsafe source parents | Passed |
| Closed-provider source parent | Passed; required refusals remain refusals |
| Device API UI | 183 cases: 26 positive, 157 compile-fail |
| Unsafe source policy | 5 passed, 1 maintenance ignore |
| Verifier library / doctests | 138 passed, 4 explicit ignores / 29 passed |
| Lowerer library | 757 passed |
| Lowerer five integration targets | 136 + 5 + 2 + 3 + 2 passed |
| Lowerer doctests | 61 passed: separate groups of 2 and 59 |
| Total lowerer Rust tests/doctests | 966 passed, zero failed/ignored |
| CPU reference / manifest / occurrences / identities | 47 + 61 + 12 + 15 passed |
| Formatting, dependency/Pliron policies, standalone locks | Passed; 32 lockfiles |
| CI dispatch, mocked compile matrix, hygiene, DCO | Passed; five commits signed off |

The private-helper source parent observed `SilentUnitLocal` on its first,
default-O3 configuration. Its retained-MIR fallback did not run and is not
counted. These four ordinary-output groups total 53 scenarios / 106 simulator
executions, not GPU observations. They consume actual admitted optimized
output and emit LLVM text/descriptors, not executed LLVM or HSACO.

The callback matrix spans gfx942/gfx950 and O0/O3 x MIR0/MIR2. Its observations
reach the checked root consumer and subsequent refusal; they do not execute
callbacks. Parent, child, repeat and UI counts are separate categories.

Strict Clippy remains **exit 101** for backend, verifier and lowerer.
Warning-mode runs complete. Independent source attribution found 43 inherited
backend sites, five verifier sites (four Clippy plus one rustc dead-code site),
and 12 lowerer sites. Verifier and lowerer warning files are byte-identical to
the public parent; backend moved spans were checked against unchanged causes.
This is not a strict-green baseline rerun or a full-workspace/hosted-CI claim.

## Retained Evidence

Directory on `mi300x`:
`/home/harsh/work/fe2o3-issue275-paired-20260917.sp9wFtRZ/validation/`.

The 19 selected `context-bridge-z-` receipts are:
`a-backend`, `a-lint`, `b-callback-source`, `c-context-source`,
`d-fill-vecadd`, `e-gemm`, `e-result-wrapper`, `e-private-helper`,
`e-backend-docs`, `g-marker-safety`, `h-source-safety`, `j-rust-call`,
`k-closed-provider`, `m-device-ui`, `o-unsafe-policy`,
`q-verifier`, `q-verifier-docs`, `q-verifier-lint` and `q-final`.

The evidence verifier checked exit status, exact HEAD, before/after fingerprint,
40/16 source counts, 183 UI cases and the private-helper route. It records
each selected log's SHA256; independent review recomputed terminal hashes.
Lint wrapper success does not replace the retained strict-lint failures.

| File | SHA256 |
| --- | --- |
| `context-bridge-z-evidence.log` | `74ec809740b5d85a256e42cd3757b16e20acfc2ecf704d41abe7b4bed1436b4c` |
| `context-bridge-z-q-final.log` | `131b0b7a484d5b9a189f46df033acd97192393ae37d4572767bde22778aac7db` |
| `context-bridge-z-backend-clippy-strict.log` | `47acbd1c090cedd7bcead701adfeef4960072c1c92223e0a3847976d53b3a2c0` |
| `context-bridge-z-verifier-clippy-strict.log` | `3ba3a351ceb37c4148ea03c5272604d2db6e7f83e8422636804c83f438a44b4d` |
| `context-bridge-z-q-clippy-strict.log` | `57820b98cc2fd31cda66a3fdd24b9fbc621560618342d54d74e46ed1db18074c` |
| `run-context-bridge-gates-20260918.sh` | `c33d359ce6e68f6a7bfeb4e57ef17088e89f7cd2b2ab73f0821f0ecdc0409dfb` |
| `verify-context-composition-evidence-20260918.sh` | `fef81b92dfacd13b14b6e2c7b7fecaa91184c9155461bb5a06c1a173786d5d7a` |

All combined implementation gates terminated before this document was added.
Owned completed test executables were removed between package batches under
the shared lock; retained evidence, reusable dependencies and peer work remain.
The clean unused preparation worktree and 1,041,348,142 bytes of independently
audited superseded source-cache pairs were removed before this combined run.

## Remaining Acceptance

M0 remains partial; M1-M6 remain incomplete. Qualified SIMT/tile pairs remain
zero, the exhaustive required denominator is unresolved, and the website is
unchanged. The manifest retains `qualified=false` and explicit missing source
bindings. Scoped source custody/replay, affine/CFG activation, lifecycle
checking/discharge, generic tile schedules, approved protected-proof/runtime
qualification, generated-host/artifact admission and per-pair target-matched
KFD correctness remain required. No acceptance boundary was waived.
