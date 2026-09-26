# Native Anchor Provisioning Checkpoint

Date: 2026-09-26. Continuation of the
[deployment transport checkpoint](conditional-native-deployment-transport-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open.
No protected execution, semantic equivalence, numerical refinement or GPU credit
is granted by these APIs or local tests.

Base: `fed174dfb8da96c4376cc9a2b1ea33876a8119be`.
Integrated code: `9489ad756d7642263f9c574028dd69b5c9f5fcc1`.

## Implementation

- `d12ba2752`: native V2/V3 anchor key owners bind role, family, full deployment
  identity and the expected verification key. Admission, root-template reissue,
  exact-object transfer and fixed observation signing preserve original-ledger
  accounting and guarded seed erasure. No raw key export or V1 upgrade exists.
  Issuer and anchor owners share fixed secret staging; the existing anchor
  protocol now shares one allocation-free signing/verification transcript.
- `2e270b398`: native 128-byte provisioning records bind the actual anchor
  deployment and bounded helper measurement, with distinct V2/V3 framing and
  identities. Public recovery requires the actual deployment, not a digest.
- `9489ad756`: provisioning capabilities reuse contextual sealed-record transport.
  Consumed File recovery returns growth; inherited recovery preserves its source
  slot and returns full private CLOEXEC custody. All nested charges use the
  original ledger, including the complete borrowed deployment floor.

Records remain inert configuration. Trusted provisioning must independently pin
their origin and measure executables. Signing authenticates a claimed observation,
not durable persistence, currentness or a compiler occurrence. The
[capability contract](../compiler-execution-capabilities-v2.md#external-anchor-provisioning)
documents exact ownership obligations and logical quotas.

A native worker implemented the provisioning protocol in a private worktree.
The primary integrated it and owned all builds. Read-only worker reviews found
no concrete defect in native key custody or provisioning transport. Coverage
limits remain: genuine root-to-service reissue was not executed; adapter-specific
sticky-history, exact-denial and overflow cases rely partly on shared-engine and
protocol tests. Workers ran no Cargo, SSH, remote jobs or permission prompts.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, one bounded Cargo job at
a time, no incremental compilation, HIP disabled, source frozen during builds.
Logs reside under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Initial focused key tests | 20 passed, before two additional reissue tests |
| Key suite R1 | 187 passed, 2 failed; incorrect new peak-history assertions |
| Corrected key suite R2 | Capability: 227 unit/integration + 114 doctests; anchor protocol: 23 + 3; all passed |
| Focused provisioning protocol / transport | 22 / 10 passed |
| Final full capability package | 237 unit/integration; 10 positive + 120 compile-fail doctests passed |
| Final full compiler-execution protocol | 322 unit/integration; 7 positive + 160 compile-fail doctests passed |
| Final full external-anchor protocol | 23 unit/integration; 3 compile-fail doctests passed |
| Eleven-package all-target check | Passed with existing warnings |
| Existing anchor service suite | 17 passed, 9 failed; exit 101 |
| Isolated entrypoint diagnostic run | 5 passed, 21 filtered; excludes failed socket setup |
| Changed Rust formatting / whitespace / hygiene delta | Passed |
| DCO through integrated code above | 98 signed-off commits from `9350f2f6b73da5e249a56a297097c4919378b363`; no exceptions |

Focused and diagnostic runs overlap full suites; do not add them as independent
coverage. Final three-package totals are 582 unit/integration and 300 doctests.
Positive doctests check APIs, not protected execution. The all-target check covers
supervisor, issuer, client, coordinator, closure capability, anchor coordinator,
service, provisioner, host, cargo-fe2o3 and rustc-codegen-fe2o3. Cargo target kinds
are not GPU architectures.

R1's two newly added reissue tests incorrectly expected a fresh peak even after
setup had recorded a larger peak. The corrected assertion takes the maximum of
prior and operation peaks. Production accounting and limits were unchanged;
the failed log is retained.

All 13 durable-state tests passed, including persistence-boundary recovery,
idempotent replay, invalid-state refusal and poisoning after persistence failure.
The full service suite is nevertheless failed: socket send/domain inspection
returned `EPERM`; response-delivery setup's `shutdown` returned -1; wrong-endpoint
testing did not reach its expected error. The first entrypoint failure poisoned
its shared test mutex, causing two further failures. Those two tests passed in
the isolated five-test entrypoint run. No failure was turned into a skip/pass,
and the service's later integration/doctest targets were not reached.
The prior checkpoint's three supervisor socket failures remain unresolved.

## Evidence Digests

SHA-256 for logs in the directory above:

```text
b4fa504a8107a9ef8fb48dabbee432485990dcb4c0e95d1179684eadae3bb459  conditional-native-anchor-key-tests-r1.log
f8ce001b3f382d13cd3f8e3eb3dda37a0e755a13fa331bd6ef97b85efc8d80a2  conditional-native-anchor-key-suite-r1.log
4a705e78585ee26b79c01480eb57dce787245b3745fca2725f37866e3b596728  conditional-native-anchor-key-suite-r2.log
76d342885e53a63070ef59b08578e95e027962f862061a3408a26133c2f1c64d  conditional-native-anchor-provisioning-tests-r1.log
493f9eb8ee890c212ea987ff28daabca939930d21b0eb59b35aa64c70dbb10ae  conditional-native-anchor-provisioning-transport-tests-r1.log
4bef2044ea00c563f2fb4c69ddf21e373d3763aac15125dd3260e158ecb7ffc0  conditional-native-anchor-provisioning-suite-r1.log
1a897583a77b21f70f8e57d92fd5484e9a8165a93e23be97aa007544b2da757e  conditional-native-anchor-provisioning-integrated-check-r1.log
3067541277401ba9ddeed6633e36c5fb56d94b27ea95e96da367649cd605da0c  conditional-native-anchor-provisioning-service-regression-r1.log
f895902664bf222932a20403ead50aab8fe7dcaa425df76393250f5d6754c330  conditional-native-anchor-provisioning-service-entrypoint-isolated-r1.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-anchor-provisioning-hygiene-r1.log
e97c63b46d3c99b36442d703c0d4d67998ff4e4c91e014a2a48186f5a9f68df3  conditional-native-anchor-provisioning-dco-r1.log
```

## Remaining Gates

1. Integrate native owners into the protected coordinator, helper and inherited
   daemon entrypoint. Both helper and service still extract raw V1 signing keys.
   Reuse the durable-state engine; preserve exact persistence-before-signing,
   recovery, lifecycle custody and retained-descriptor cleanup behavior.
2. Execute actual protected-root reissue and startup/readiness/recovery cases.
   Local expected-owner hooks do not establish protected provisioning.
3. Wire native admission, conditional preparation, postchecks, invocation finish,
   revalidation, V5 publication and SubjectV3 acquisition/transport through the
   existing production compiler transaction on its original ledger. Backend
   receipt admission still uses V1; conditional production finalization refuses.
4. Complete source/machine/numerical refinement and target-matched GPU runs for
   all 47 kernels, plus generic non-AMD coverage. Numerical differences require
   explicit compiler-proved bounds, not unchecked comparison tolerances.

Fresh probes of mi350, mi350-2 and mi300x failed DNS resolution. GitHub issue
refresh also failed to connect. No remote job/scratch or shared-cache cleanup
was performed. Remote main state remains unverified; normal pushes to both
remotes are still required. This checkpoint does not update the tutorial site
or claim additional end-to-end kernels.
