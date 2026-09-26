# Native Deployment Transport Checkpoint

Date: 2026-09-26. Continuation of the
[controller/provisioning checkpoint](conditional-native-provisioning-20260926.md),
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and the 47/47 production-to-safe-GPU-launch matrix remain incomplete.
No protected execution, semantic equivalence, numerical refinement or GPU credit
is granted by these transport APIs or local tests.

Base: `cf98106d17b4808dac78f6572ab9a60317409f45`.
Integrated code: `d6a2f03665ad6bcd0b403cc22c3fc75035097123`.

## Implementation

- `01ed40fad`: the existing sealed-record transport now accepts a private borrowed
  decode context. Existing policy/profile/launch APIs and quotas are unchanged.
  Native supervisor deployment recovery requires the actual same-family policy,
  checks its full storage floor before I/O scratch can mask underpayment, and
  reserves decoded ownership on the original ledger. Consumed File recovery
  returns only growth; inherited recovery returns full private duplicate custody.
- `a4bde3f26`: native V2/V3 anchor deployment records bind the actual supervisor,
  actual policy, exact anchor credentials/key, and a bounded executable digest
  and length. Distinct 168-byte frames and hash domains reject cross-family
  substitution. The nested supervisor-policy check uses the original ledger.
- `d6a2f0366`: sealed anchor deployment capabilities reuse the same transport.
  Recovery requires both actual context owners, not identities or a V1 upgrade.
  Revalidation and transfer require the exact object, metadata, seals and bytes.
  Anchor I/O charges 38152 logical work units; full contextual admission charges
  49432. These are logical quotas, not instruction, elapsed-time or RSS bounds.

These records are immutable configuration, not authenticated provisioning.
Trusted callers must independently pin configuration origin and executable
measurements. A valid rehashed configuration or an immutable descriptor cannot
establish those facts. See the
[capability contract](../compiler-execution-capabilities-v2.md#native-deployment-transport)
for the input/output accounting obligations.

A native worker implemented the anchor records and transport in a private
worktree. The primary integrated the commits and owned every build. A subsequent
read-only worker review found no concrete defect or material missing regression
in the shared context transport; the primary found no concrete defect in the
anchor codec/adapter. Workers ran no Cargo, SSH, remote jobs or permission prompts.

## Validation

Pinned `nightly-2026-04-03`, frozen offline dependencies, one bounded Cargo job at
a time, no incremental compilation, HIP execution disabled, and primary source
frozen during builds. Logs reside under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Source checkpoint | Result |
| --- | --- | --- |
| Supervisor transport integration | `01ed40fad` | 16 passed |
| Anchor deployment protocol integration | `a4bde3f26` | 28 passed |
| Full protocol package | `a4bde3f26` | 300 unit/integration tests; 7 positive and 142 compile-fail doctests passed |
| Anchor transport integration | `d6a2f0366` | 22 passed |
| Full capability package | `d6a2f0366` | 205 unit/integration tests; 6 positive and 92 compile-fail doctests passed |
| Eleven-package all-target check | `d6a2f0366` | Passed, with existing warnings |
| Native supervisor regression | `d6a2f0366` | 125 passed, 3 failed, 32 ignored, 154 filtered; exit 101 |
| Host-independent doctor CLI tests | `d6a2f0366` | 3 passed; hardware-gated test filtered out |
| Changed Rust formatting and whitespace | `d6a2f0366` | Passed |
| Hygiene delta against the base above | `d6a2f0366` | Passed |
| DCO from `9350f2f6b73da5e249a56a297097c4919378b363` | `d6a2f0366` | 94 signed-off commits; no inherited exceptions |

Focused tests overlap package suites and are not independent additional coverage.
Protocol source did not change after its full package run. The capability suite
includes decoder unwind, arithmetic/resource limits, first-denial history,
substituted context, cross-family frames, descriptor retirement and exact-object
transfer tests. Positive doctests are API checks, not protected execution.

The all-target check covers supervisor, issuer, client, coordinator, closure
capability, anchor coordinator/service/provisioner, host, cargo-fe2o3 and
rustc-codegen-fe2o3. It checks Cargo target kinds, not GPU architectures.

The same three supervisor failures as the prior checkpoint remain:

1. `handoff_v2::tests::shared_socket_failures_preserve_legacy_and_native_categories`
2. `handoff_v3::tests::shared_socket_failures_preserve_legacy_and_native_categories`
3. `listener::native_accept::tests::named_socket_accepts_exact_control_and_rejects_path_drift`

They fail with `EPERM` at `handoff_native_io_tests.rs:77` and
`listener_native_accept_tests.rs:127`, before their intended socket assertions.
They were neither skipped nor converted into passes. No listener scratch
directory remained after the run. The 32 ignored protected fixtures were not
executed. CLI success confirms diagnostics and rejection of `--require-execution`
at the unwired Worker V3 application route, not application readiness.

## Evidence Digests

SHA-256 for files in the evidence directory above:

```text
6594c5eca848356126d2786f8a494e77f08a8f4774b0b5112331c9c29d2fa302  conditional-native-deployment-transport-tests-r1.log
3746158ea941edee3e825dabc67884d8d8b63f6a259e0847dd67b653614a0cd9  conditional-native-anchor-deployment-tests-r1.log
a7e04acce81dac74f83c3cae295f78565ca7a59db949b833835219e5f64df3ff  conditional-native-anchor-deployment-protocol-suite-r1.log
ea2237686ae44e3ffe8f8644d4318876b6f79c8d8dfc5ec7ef5957acf86041dc  conditional-native-anchor-transport-tests-r1.log
e5cc94ec899e20ac4236f3256d1d8941c7949c5864b8962480fc3bd02198036b  conditional-native-anchor-transport-capability-suite-r1.log
6722623eaf96175bb4eb5f0af5a8943ae7f49857701782e62205198e13942c2c  conditional-native-deployment-transport-integrated-check-r1.log
1cf148752aee396b655e01a163c06523651e5e9bed3806227be23c3e18d3ee3e  conditional-native-deployment-transport-supervisor-regression-r1.log
265114508aee18bc12cbedaeecb095ead2941574a7c271c2703407f7d6709973  conditional-native-deployment-transport-doctor-r1.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-deployment-transport-hygiene-r2.log
758ea95d30c76e626eeb1faa6e2611ee3ace9f88e7793a417bc12402c96cbcbd  conditional-native-deployment-transport-dco-r1.log
```

## Remaining Gates

1. Implement native anchor signing-key and provisioning ownership, preserving
   role/deployment binding, guarded seed erasure and original-account charging.
   Integrate those owners with the root coordinator and inherited startup. The
   current anchor coordinator and protected service entry points still use V1.
2. Execute genuine protected-root reissue, listener/session/issuer, readiness and
   recovery cases in the isolated protected environment. Retain exact executable,
   credential, parent, key and persistent-cleanup bindings.
3. Wire native admission and conditional preparation through the existing normal
   compiler transaction, including postchecks, invocation finish, revalidation,
   V5 publication and SubjectV3 acquisition/transport. Backend receipt admission
   still uses V1 and conditional production finalization explicitly refuses.
4. Complete exact source/machine/numerical refinement and target-matched runs for
   all 47 kernels, with generic non-AMD coverage. Numerical differences require
   explicit compiler-proved limits, not unchecked comparison tolerances.

GitHub fetches and SSH probes of mi350, mi350-2 and mi300x failed DNS resolution
from this environment. No remote job or scratch directory was created. Remote
main state remains unverified; both remotes require normal fast-forward pushes.
No shared cache or unrelated source/report cleanup was performed in this checkpoint.
