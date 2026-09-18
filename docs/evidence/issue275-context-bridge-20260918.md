# Issue 275: Authenticated Context Bridge

## Executed Candidate

- Implementation: `509a4a5148adaa57d962966ff32bae6c46b70916`.
- Published integration base: `98ec3935544c45f03f37384a98c83f41cc6e4738`.
- Before/after source fingerprint:
  `b4b7cb097d157a1d11b446b3d50aef575867613472200515c08ae6eabefc8818`.
- This evidence-only child does not change the executed implementation.
- Host: `mi300x`; pinned `nightly-2026-04-03`, offline/locked Cargo,
  jobs 1, test threads 1, incremental disabled, shared build lock.
- The existing 8 GiB target cap and disk/memory reserves were not raised.

## Integrated Scope

The reviewed backend subset of source234
(`db132062461ff1579caedfbb93e49e99578b607a`) and preserved-provider source235
(`61f42b63160289d040b07c3901408bf1bf9bf5ee`) supplies authenticated context
imports, receiver/closure transport, same-owner root custody and the checked
root handoff. It preserves the ordinary Defined identity of
`ExecutionWithWorkgroup` / `fe2o3_device_with_workgroup_v1`.
Unsafe callback traversal and forged-provider/marker rejection remain enforced.

The historical borrowed-aggregate lowerer stack was not imported. All lowerer
files are byte-identical to the public integration base. The normal merge
retains the published actual-output numerical oracles and Result/moved-owner
admission. The two newly introduced backend lint causes were fixed separately.

**The V29 materialization gate remains closed.** Root/source observations do
not execute workgroup callbacks or tiles. There is no new scoped materializer,
affine availability proof, lifecycle discharge, artifact/launch authority,
GPU result or qualified tutorial pair in this checkpoint.

## Validation

Counts below are separate categories; parent/child observations and retries
are not added together as new independent tests.

| Gate | Result |
| --- | --- |
| Backend library | 852 passed, 10 default ignores |
| Backend doctests | 2 passed |
| Explicit callback/context source parents | 40 callback and 16 callback-free observations |
| Ordinary fill/vecadd/GEMM/Result source parents | Passed; 45 numerical scenarios, each simulated twice |
| Ordinary RustCall closure/slice simulation parent | Passed on both target profiles |
| Forged marker and reachable-unsafe source parents | Passed |
| Closed generative-provider source parent | Passed; refusals remain refusals |
| Device API UI parent | Passed, 183 internal cases |
| Unsafe source policy | 5 passed, 1 explicit maintenance ignore |
| Lowerer library | 721 passed |
| Lowerer five integration targets | 136 + 5 + 2 + 3 + 2 passed |
| Lowerer doctests | 54 passed |
| Total lowerer Rust tests/doctests | 923 passed, zero failed/ignored |
| CPU reference / manifest / occurrences / identities | 47 + 61 + 12 + 15 passed |
| Formatting, dependency/Pliron policies, standalone locks | Passed; 32 lockfiles |
| CI dispatch, matrix shell, hygiene, DCO | Passed; 3 implementation commits signed off |

Ordinary numerical parents simulate the actual admitted optimized output and
emit LLVM text; they do not execute LLVM, protected proofs or GPU artifacts.
The compile-matrix shell test uses mocked tools, not real hardware validation.
The manifest still reports `qualified=false`, pending semantic oracles/policy
verification and unresolved curriculum source bindings.

Strict all-targets Clippy **fails with exit 101** for both backend and lowerer.
Warning-mode runs complete successfully. Independent source comparison attributes
43 unique backend sites to inherited causes (36 in unchanged files, seven
unchanged causes in modified files), and 12 lowerer sites to 11 files
byte-identical to the public base. This is not a new strict-green baseline run.

## Retained Evidence

Directory on `mi300x`:
`/home/harsh/work/fe2o3-issue275-paired-20260917.sp9wFtRZ/validation/`.

The selected receipts are `context-bridge-` plus:
`t-a-backend`, `t-a-lint`, `t-b-callback-source`, `t-c-context-source`,
`t-d-fill-vecadd`, `t-e-gemm`, `t-e-result-wrapper`,
`u-e-backend-docs`, `u-g-marker-safety`, `u-h-source-safety`,
`v-j-rust-call`, `v-k-closed-provider`, `v-m-device-ui`,
`w-o-unsafe-policy` and `y-q-final`.

Every selected receipt has exit 0, the exact executed HEAD, unchanged source
fingerprint and retained command. The lint wrappers' exit 0 records successful
collection; their nested strict-lint exit 101 is retained explicitly.

| File in evidence directory | SHA256 |
| --- | --- |
| `context-bridge-y-evidence.log` | `3013e55b40b276708e2e5a9602d9db14236b93768bf44d28ee392c2d2e7bcab5` |
| `context-bridge-y-q-final.log` | `5a675d961d59f466f9f71bdab09f4f599a5205875802f3184894475ecfbaadee` |
| `context-bridge-t-backend-clippy-strict.log` | `4ba109c1f2d8910631d7d19bc933deaff8c71a61fc33c79f2e7f649d57619533` |
| `context-bridge-t-backend-clippy-observed.log` | `b6f1ba1202df6dd5c85d6f85886af2b2d34a9ee480d46588ec5d48c16af67a83` |
| `context-bridge-y-q-clippy-strict.log` | `054e8a181935ce124fef6a192e0a9d339a0452792c800413d7832f9163c5b556` |
| `context-bridge-y-q-clippy-observed.log` | `e9f584d6da8907d6a2f77a69d65350e0c3ee71f4fd88ff1a01afac791d9cf9de` |
| `run-scoped-materialization-final-20260918.sh` | `ec5cbfe9a83806e2c369a55c7429dbf7c0a244f99b7ae049d07c58ea6e26adf9` |
| `run-slice-final-policies-20260916.sh` | `88a966abcc7a8ed212b81911abcb3f1fe942982299f08b6b8386162096467d08` |
| `verify-context-bridge-evidence-20260918.sh` | `b51f8dcee5ba4ce8b8502d5fdc62103a50047c659d5819c74cfda1526fadda30` |

The verifier records all 15 individual receipt hashes and checks exact source
guards and 40/16 source-observation counts. Independent review checked the
recorded hashes against the files. Three resource-stopped attempts remain at exit
75 and are excluded; owned cache/test cleanup permitted retries under the same
cap. Final attempts `w-q-final` and `x-q-final` remain at exit 1. Their failures
were inherited runner environments in mocked shell tests: `LD_LIBRARY_PATH`
changed a command golden, then `ROCR_VISIBLE_DEVICES=-1` conflicted with a
compile-only ablation's selector. Only the external wrapper changed: clear the
loader variable for the CI self-test and visibility variables for the mocked
matrix test. Production source and test assertions were not weakened.

## Remaining Work

M0 remains partial; M1-M6 remain incomplete. Fully qualified pairs remain zero
and the exhaustive required denominator remains unresolved. The website is
unchanged. Next: authenticated provider-call/derive/exit custody, integration
of instance-qualified availability, independently replayable scoped lowering,
lifecycle verification/discharge, executable tile scheduling, then per-pair
CPU/oracle and target-matched KFD qualification.
