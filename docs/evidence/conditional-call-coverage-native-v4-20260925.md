# Conditional Call Coverage and Native V4

Follow-up to [slice extents and Worker V4](conditional-slice-extents-worker-v4-20260925.md)
for #272. **No milestone closes and no kernel gains end-to-end qualification.**
Actual shared-body Vecadd now passes CPU-oracle and conditional proof-preparation
checks on both gfx942 and gfx950. Preparation deliberately stops before protected
proof execution. The default, unannotated manifest selection is unchanged.

## Canonical Coverage

The real source's shared bounds-failure block contains a target diagnostic call
followed by `Unreachable`. Coverage previously rejected that call before deriving
the input-read premises which exclude the block.

The existing two-pass analysis now defers call rejection until its second pass.
The first pass still discovers read and address domains without using input guards
as assumptions. The second applies only exact, retained read-bound premises and
rejects every call on a potentially executed path. There is no callee-name,
target, tutorial or source-text allowlist. Reachable abnormal exits, unsupported
operations, cycles and incorrect write counts remain refusals.

The six new tests cover excluded calls, reachable-call mutations, two input
guards sharing a failure block, unrelated-slice substitution, an input guard
before the output guard, launch-wide reads, and exact resource boundaries.
In particular a guarded read cannot hide a failure outside the output domain,
including an empty output. Input guards cannot narrow their own read premises.
A separate read-only review found no concrete soundness or accounting defect.

The actual-source observer now reports the precise unsupported-coverage reason.
Its budget checks measure scratch peaks instead of assuming no scratch is needed;
exact and one-short work/storage limits and missing owner reservations are tested.
This corrects the test harness, not the production resource schedule. Temporary
canonical graph prints used for diagnosis are not retained in the code.

## Native V4 Structure

Distinct move-only native V4 artifact owners and identity domains share the strict
native finalization and independent replay implementation. Exact contract/ABI,
source, transaction, Worker lineage, artifact and resource joins are preserved.
Fresh consumption and recovered transcript custody remain distinct. Private
storage and checked variant extraction prevent legacy wrapping.

This remains a structural prerequisite. Upstream native semantic recovery still
requires descriptor V1; a public recovered native source carrying descriptor V4
cannot yet be obtained. No V4 publication/load bridge, conditional proof receipt
or launch authority is added. The existing publication gates are unchanged.
See the [finalizer contract](../../crates/fe2o3-hsaco-finalize/README.md).

## Validation

The nightly `2026-04-03` guard used locked offline dependencies, one Cargo job/test
thread, disabled HIP, hidden GPUs, a 12 GiB virtual-memory ceiling and a
1,200-second deadline. Source and tool hashes remained stable in every listed run.

- A: 8,160 files, `9feb6915e03bc7eb229b6f46abf56209c98adc845618da6e7c22845da09c91eb`.
- B: 8,160 files, `a659156caf7afa319a24c1f58cc67b6df85154671b9689cfb5cb6e0a2fa062d9`.
- Guard SHA256: `20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

These are file inventories, not Git tree hashes. B differs from A only in the
test observer's budget checks. Subsequent edits are documentation only.

| Guard | Snapshot | Result | Log SHA256 |
| --- | --- | --- | --- |
| `conditional-dead-call-kernel-ir-r2` | A | 31 coverage tests passed | `8d48e94bbe3f4ce7c178f1b0a24779d9269bec9ef99831f3a4c2074e7fc0bc0b` |
| `conditional-native-v4-tests-r1` | A | Full finalizer suite: 335 tests and 40 compile-fail doctests passed; 26 ignored | `58644670a448ff3f17cab448bc22f2ad0c6413e9e4bac1ae9746cdcb7902d0d9` |
| `conditional-dead-call-actual-source-r2` | B | Actual annotated Vecadd oracle/preparation passed on both targets | `1bc82edf06c7092d6cafbd5961d7ee8372d78c5ff4ca92757de9a01791b71ad9` |
| `conditional-native-v4-fixture-export-r1` | B | Fresh four-case native fixture export passed | `d0b3751fc0c0c9bf30a6b12ca438be1e340f80ee26fe72b4b009eab3b558d669` |
| `conditional-native-v4-explicit-fixtures-r1` | B | All 18 explicitly selected native finalization/replay tests passed | `cc8a162446836adb683c8c75c8f2b1fa38176ee1635e38dad8d13bd697d3a50d` |
| `conditional-dead-call-production-check-r1` | B | Normal backend library check passed without test-only proof features | `af5ddb2c57403a0fe238458bbc3eac54350f52215ce9fda66b46beb439f15c2b` |

The 18 opt-in cases were among the default suite's ignored tests; other ignored
tests receive no pass credit. Native fixtures use public test keys, a synthetic
source invocation and the measured fixture Worker, not protected compiler origin
or real LLVM/GPU execution. Their four members were archived and byte-verified
before temporary-directory removal. Archive SHA256:
`b65940a1a21191dbce56cd01b833a27af5b6c8480e1cd1342d6ab00b5798810a`.

For each target the actual-source oracle observed ten matching scenarios, two
short-input refusals and no counterexamples. Preparation retained output argument
2, canonical length value 19, ranked extent argument 0 and `GlobalLaunch` address
formation. It stopped at the exact test-only pre-proof boundary with no root
artifact, receipt or launch authority. These are sampled CPU observations and
preparation tests, not protected semantic equivalence or hardware qualification.

The initial diagnostic probe identified the real `Call` refusal (log SHA256
`2bd0cec3b46e2594d2da6a54ffcb3e314fc0ed1256e0ec0aa10c60b48b9ed219`).
The first coverage-test build failed on a test-only `Debug` assertion, corrected
before r2. Actual-source r1 then passed canonical coverage but failed the test
observer's zero-scratch assumption (log SHA256
`55b69d5c7850b136bcdbeed11ac1b4ddd3726d2993bcccba62935d1238065a25`).
Actual-source r2 corrected that assumption and passed. Existing unused-code
warnings remain. This is not a workspace-wide test run.

## Reproducible Base

The package-availability blocker is cleared. All 99 unchanged locked packages,
including the 24 missing pins, were recovered from the official Ubuntu snapshot
`20260831T060000Z`. Signed metadata, package identities, SHA256s and the exact
dependency closure were verified. Package archive SHA256:
`df29ed728726d768a3419f32a95a0d25557e9c89f04d0c5ce2e7ae3f73cb6eb5`.

Two executions of the unchanged builder from a private clean checkout of
`de7f415eb768aa3e495842252621b02884d3812b` produced byte-identical bundles.
The repository's two-bundle test passed. Each SquashFS image is 31,657,984 bytes,
SHA256 `79745e545419d9c66ba77c9fcbd57751d39aff3dc1a6b668776edf2a30f51568`.
The verified two-build transport archive SHA256 is
`36abf0ebe0893ce16b492bf2759ff47e71acef0128cd7838434c98bc781ae9f5`.
The embedded source epoch remains `1790350698`; this base was not relabeled as a
later compiler candidate.

Builds used local Ubuntu 24.04 amd64, private APT state, one CPU and bounded
time/storage. An initial TLS failure preceded extraction; a private configuration
then referenced the environment's existing CA bundle with peer/hostname checks
still enabled. Build B's disk watcher failed during a changing-directory scan;
the original builder was not restarted. Its recorded terminal success, absent
process group, output identities and independent reproducibility check were
verified afterward. Both orchestration failures remain in the retained evidence.

All sidecar jobs and recorded process groups ended. Build temporary paths and the
private coding worktree were removed; inputs, outputs and verified evidence remain.
No SSH, root provisioning, host package installation, service, shared runtime or
GPU operation was required. This base is an input for later isolated validation,
not protected-runtime execution or production qualification by itself.

## Remaining Work

Fresh frozen-input export and protected-runtime replay, conditional final-graph custody,
applicable target/machine/numerical refinement, native V4 semantic recovery,
authenticated publication, generated safe host launch and the full 47-kernel
matrix remain. No existing authority gate was weakened to make these checks pass.
