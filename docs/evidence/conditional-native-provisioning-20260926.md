# Native Controller and Provisioning Checkpoint

Date: 2026-09-26. Continuation of the
[native listener checkpoint](conditional-native-listener-20260926.md), for
issue #272. M0-M7 and the 47/47 production-to-safe-GPU-launch matrix remain
incomplete. This work grants no protected-proof, numerical-refinement,
root-provisioned execution or GPU credit.

Follow-up: [native deployment transport](conditional-native-deployment-transport-20260926.md)
adds contextual sealed supervisor/anchor transport and native anchor records.
It does not activate protected startup or the production compiler route.

Base: `1f8c213f9d7c7c113276f28e2868141401380bbb`.
Integrated code: `d5a47e272708d8b175ce29d58ed0da9e8875543a`.

## Implementation

- `3cff85fb0`: native V2/V3 services expose finite `run_turns`, using the original
  request budget and separately funded persistent cleanup. Every dispatch outcome
  is followed by a cleanup attempt. Full pump funding is required before another
  dispatch. Reports preserve dispatch and cleanup failures separately and return
  only inert observations. A turn limit does not establish terminal cleanup.
  Controller tests use synthetic dispatch outcomes and an actual isolated cleanup
  account, not a launched native issuer.
- `72407f03e`: distinct 184-byte V2/V3 supervisor deployment records bind service
  and anchor credentials, exact supervisor/launcher measurements, and the complete
  native policy identity. Decoding requires the actual same-family policy and
  original resource ledger. These are inert configuration, not authenticated
  provisioning or executable capabilities.
- `5e473f649`: distinct 88-byte native readiness records require the expected
  child PID and exact native deployment during decoding. Correctly rehashed
  records for another context are rejected. Readiness bytes alone do not prove
  process liveness, trusted bootstrap provenance, or clean channel EOF.
- `cf73e722f` and `3d9a0eca2`: native signing-key reissue consumes a prepaid,
  root-owned 0:0, sealed anonymous mode-0400 read-only CLOEXEC template. It binds
  the actual policy and current nonroot deployment credentials, derives the key
  once, creates fresh service-owned custody, and checks both images again before
  returning. Seed staging is guarded before resource admission; returned storage
  is only growth over the consumed File. The caller must independently pin
  deployment provenance. Closing a memfd does not prove kernel-page erasure.
- `d5a47e272`: the same reissue scope now has deterministic late-failure/unwind
  coverage through a private observation point. Fresh descriptor flag removal,
  template permission mutation, and panic after allocation all preserve seed
  wiping, descriptor retirement, entry storage, cumulative work, peak storage,
  ledger identity, and first-denial history. Production uses a no-op observation.

A native worker implemented deployment records and key reissue in a private
worktree. Read-only controller/bootstrap review found no concrete defect. An
independent key review found a missing late-cleanup test, not a demonstrated leak;
the final commit addresses that gap with shared tests for both families. The
primary integrated the changes and owned all builds. No worker ran Cargo, SSH,
remote jobs, or permission requests.

Rootless reissue tests use a private expected-owner helper; the public API rejects
the service-owned test templates. These tests assert a nonroot process instead of
silently passing when their prerequisite is absent. They are not successful
root-owned-template handoff evidence. The inherited production entry remains V1.

## Validation

Pinned `nightly-2026-04-03`, frozen offline dependencies, one Cargo job at a time,
no incremental compilation, bounded processes and a frozen primary source tree
during each build. Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Source checkpoint | Observed result |
| --- | --- | --- |
| Focused controller tests, R2 | `3cff85fb0` | 22 passed |
| Focused deployment tests | `72407f03e` | 22 passed |
| Focused bootstrap tests, R2 | `5e473f649` candidate | 12 passed |
| Complete protocol package | `5e473f649` | 272 unit/integration tests; 7 positive and 124 compile-fail doctests passed |
| Focused reissue tests, including late cleanup | `d5a47e272` candidate | 20 passed |
| Complete capability package, R2 | `d5a47e272` | 165 library tests; 2 positive and 54 compile-fail doctests passed |
| Native supervisor regression | `3d9a0eca2` | 125 passed, 3 failed, 32 ignored, 154 filtered out; exit 101 |
| Supervisor doctests | `3d9a0eca2` | 6 positive and 123 compile-fail examples passed |
| Eight-package all-target check, R2 | `d5a47e272` | Passed |
| Changed Rust formatting and whitespace | `d5a47e272` | Passed |
| Hygiene delta against the base above | `d5a47e272` | Passed |
| DCO from `9350f2f6b73da5e249a56a297097c4919378b363` | `d5a47e272` | 90 signed-off commits; no inherited exceptions |

Focused counts overlap the package/regression suites; they must not be added as
independent coverage. Protocol source was unchanged after its complete package
run. The final capability suite and integrated check include the late-cleanup
change. Positive doctests are API checks, not protected launch evidence.

The all-target check covered supervisor, issuer, client, coordinator, closure
capability, host, cargo-fe2o3 and rustc-codegen-fe2o3. It means Cargo target kinds,
not GPU architectures. HIP execution was disabled. Existing dead-code and unused
import warnings remain; this was not a warning-free build or hardware run.

The supervisor failures were:

1. `handoff_v2::tests::shared_socket_failures_preserve_legacy_and_native_categories`
2. `handoff_v3::tests::shared_socket_failures_preserve_legacy_and_native_categories`
3. `listener::native_accept::tests::named_socket_accepts_exact_control_and_rejects_path_drift`

The first two encountered `EPERM` at `handoff_native_io_tests.rs:77`; the third
encountered `EPERM` at socket bind in `listener_native_accept_tests.rs:127`.
They were not skipped or converted into passes. The same three failures appeared
in the earlier controller regression. The failed listener's private temporary
directory was cleaned up. No ignored protected-environment fixture was executed.

## Evidence Digests

SHA-256 for files in the evidence directory above:

```text
57d6b832853625fe1ab1333bab6ddb1578588fbdf7cabdd6ffd5d7a89443873e  conditional-native-controller-tests-r2.log
2d69d74e6bbc702eb262a992db6862841e8e91879da04ab1299eb124a949d2e1  conditional-native-deployment-tests-r1.log
9321067d04d668284ab35bd79afa3a95c1961a348267834fa57a17d65c73e545  conditional-native-bootstrap-tests-r2.log
cf132e66dd0e5111cb203c1960d0b0d2b08267fb1aedc5a6a8739dcb2762131a  conditional-native-bootstrap-protocol-suite-r1.log
c31288efb4930644b3318718eb056496e134e40bc5f113249559bb4aeaf99792  conditional-native-key-late-cleanup-tests-r1.log
3297f1d53ba51ee28e208f2a57e3623596f4f06a7abe06e57746d64db65a31d7  conditional-native-provisioning-capability-suite-r2.log
d8ae1133d8af8a47f3e84b6e9220ca99d4a6ba1d7e6022344de94a8858e80b56  conditional-native-provisioning-supervisor-regression-r1.log
9a21597950ac6f64d27a78fb29c149f96a59b688770d787777aaf1ed398931f4  conditional-native-provisioning-supervisor-doctests-r1.log
32d56e82b49e485f39fb60ebe7fdc279fed5ce359cb69ca1946d28b893b5abdf  conditional-native-provisioning-integrated-check-r2.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-provisioning-hygiene-r2.log
c31cc80b4e0b8806ae997900b5bd655622be9b172b606051710cb5890e0bf6a4  conditional-native-provisioning-dco-r2.log
```

## Remaining Gates

1. Carry native deployment records through sealed transport with actual policy
   context, extending the existing capability machinery rather than introducing
   a second transport path. Provisioning origin must be independently pinned.
2. Integrate native anchor/deployment configuration, root coordinator admission,
   inherited startup, readiness and recovery. Keep exact parent, supervisor,
   launcher, runtime and key bindings, plus persistent cleanup ownership.
3. Execute protected-root key reissue and the genuine native listener/session/
   issuer cases in the isolated protected profile. Rootless helpers and compiled
   static fixtures cannot substitute for these runs.
4. Wire native admission, conditional preparation, postchecks, invocation finish,
   revalidation, V5 publication and SubjectV3 acquisition/transport through the
   normal compiler entry point. Backend admission remains V1 and conditional
   publication still refuses; library availability does not activate that path.
5. Complete source/machine/numerical refinement and all 47 target-matched
   gfx942/gfx950 runs, plus generic non-AMD coverage. Numerical differences need
   explicit compiler-proved limits, not unchecked comparison tolerances.

Both GitHub fetches and probes of mi350, mi350-2 and mi300x failed DNS resolution
during this checkpoint. No remote job or scratch directory was created. Remote
main state remains unverified; synchronization requires normal fast-forward
pushes, not force updates. Local headroom was recovered by removing 192 MiB of
our inactive host-profile release cache while Cargo was idle. Source worktrees,
reports, active debug/musl caches and prior validated static artifacts were kept.
