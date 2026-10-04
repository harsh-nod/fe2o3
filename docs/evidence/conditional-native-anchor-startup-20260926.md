# Native Anchor Startup Checkpoint

Date: 2026-09-26. Continuation of the
[peer checkpoint](conditional-native-anchor-peer-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open. This
checkpoint implements native daemon startup and validates a static V3 image;
it does not establish successful protected startup, compiler semantic equivalence,
numerical refinement or GPU execution.

Base: `03f4b58e8aa2fde664a324927a14a421c4f47d43`.
Integrated code: `97eea83729b500e5dfb03b476f240502c436398a`.

## Implementation

- `b6f448ab8`: native metered lifecycle admission, exact parent identity,
  revalidation and close-only shared-lock custody, using the existing filesystem
  engine. No public V1 upgrade, descriptor accessor or unchecked native path.
- `97eea8372`: native V2/V3 inherited startup and dedicated binaries. Actual
  policy, supervisor, deployment and key owners stay in their family. Cleanup
  precedes descriptor-owning admission; the complete source table is checked
  before duplication. Native process/namespace, running-image and lifecycle
  checks compose with existing native durable-state and peer operations on the
  original caller ledger. Startup opens existing state only.
- The same commit adds bounded running-image admission, explicit static-build
  family selection, startup cleanup/refusal tests, and the lifecycle test-harness
  readiness framing correction. The script still defaults to V1; existing helper
  and coordinator launch paths remain V1. No legacy fallback is added to native
  startup, and no raw signing key is extracted by it.

See the [startup contract](../compiler-execution-native-anchor-state.md#native-inherited-startup)
for the unsafe isolated-process boundary, exact descriptor table, accounting,
finite-attempt I/O and late-failure limits. Quotas do not bound RSS, syscall
latency or idle service lifetime.

A native worker implemented lifecycle custody in a private worktree and reviewed
startup, packaging and the next helper boundary. The primary integrated startup
and tests. Review found a retrying standard-library running-image open; it is now
a direct, single-attempt syscall with an EINTR regression test. Cleanup tests now
check all retained object identities, including low-numbered private duplicates,
not just the fixed slots. Follow-up source review found no remaining concrete
defect in those corrections or packaging. The worker ran no builds, tests, SSH or
network commands and is closed.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, one bounded Cargo job at
a time, HIP disabled, no incremental compilation, production sources unchanged
during builds. Logs and the preserved executable are in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Initial focused compilation | Two failures: tuple-struct alias construction, then unavailable rustix errno accessor; corrected |
| Focused R3 | Lifecycle 14 passed; service 44 passed, 6 socket failures; other tests filtered |
| Initial host binaries | All three service binaries built; before the final running-open correction |
| Full R1 | Lifecycle 19 passed, 1 readiness failure, 1 existing ignored subprocess role; service 72 passed, 15 failed; executable 14 passed |
| Isolated lifecycle crash-order retry | Passed after framing the readiness sentinel on a fresh line |
| Final full R2 | Lifecycle 20 passed, 1 existing ignored subprocess role; service 72 passed, 15 failed; executable 14 passed; no filtering; exit 101 |
| Concurrent startup groups, four test threads | Both families passed all 10 cases each; 2 tests passed, 85 filtered |
| Three-crate doctests | 7 positive and 25 compile-fail passed |
| Thirteen-package all-target check | Passed with existing warnings |
| Static V3 release R1 | Timed out after 20 minutes before linking; exit 124 |
| Static V3 release R2 | Passed from the same sources and cached dependencies in 12m 58s |
| Exact V3 ELF and production image parser | Passed; explicit ignored integration test executed, 1 passed |
| Exact V3 empty-environment, missing-contract smoke | Exit 1, no output |
| Changed Rust formatting, whitespace and code hygiene delta | Passed |
| DCO through integrated code | 108 signed-off commits from `9350f2f6b73da5e249a56a297097c4919378b363`; no exceptions |

The final full unit run totals 106 passes and 15 failures, plus the existing
subprocess-role ignore. Focused and concurrent runs overlap that suite. New
startup tests exercise 20 refusal/accounting cases with actual native contexts,
keys and durable state but private rootless process/image/lifecycle hooks. They
cover profile rejection, the private pre-serve checkpoint, unwind, work/floor/
scratch refusal, substituted policy, absent state, invalid running image and a
missing peer slot. They assert unchanged state, closed input/private descriptors,
restored storage and preserved original-ledger/denial history. The private
checkpoint is not protected readiness or a successful socket exchange.

The 15 service failures are the 13 recorded in the peer checkpoint plus two new,
unignored native startup socket tests. Both new cases fail socket-domain
inspection with `EPERM`, after native rootless admission. Their cleanup,
duplicate-close and accounting assertions pass. Existing failures include socket
send/domain inspection, response-direction shutdown, wrong-endpoint categories
and poisoned-mutex cascades. The full suite is not green; no production checks
were weakened and no failing socket tests were skipped.

The cross-package check covers supervisor, issuer, client, compiler coordinator,
closure capability, anchor coordinator/service/provisioner, host, cargo-fe2o3,
backend, lifecycle and protected executable. Cargo target kinds are not GPU
architectures. Prior protected supervisor transport failures remain unresolved.

## Static Image

The preserved source ELF is `conditional-native-anchor-startup-v3-97eea83729b5.elf`:
6,856,384 bytes, SHA-256
`c0f373460fc2a8bd680a475c13cb0af795e871bc6e4122d21477495ede847517`.
It is ELF64 EXEC with entry `0x6923b0` equal to `fe2o3_secure_start_v1`, a
non-executable stack, no dynamic-loader dependency and no undefined symbols.
The shared production sealed-static image parser accepts its bytes.

The build used `cargo rustc --release --target x86_64-unknown-linux-musl` with
`+crt-static`, static relocation, `-static`, `-no-pie`, and the secure entry symbol,
matching the script's build options. Its image/parser/smoke stages were run
explicitly with the existing host test cache; the whole packaging script was not
run with its separate profile-test cache. Script syntax and unknown-family
rejection were checked. V2 was host-checked but not separately built as a static
release image in this checkpoint.

This is a source ELF, not an admitted sealed runtime capability. The smoke test
proves refusal with missing startup prerequisites, not which individual check
caused refusal. Root provisioning, actual protected profiles, helper-to-daemon
exec, protected readiness/recovery and GPU launches were not executed.

## Evidence Digests

SHA-256 of files in the evidence directory:

```text
24b623b4a5a52a6c75fd1260c6d1fa224ab6002d3fc2dbca4fc3548446245ca3  conditional-native-anchor-startup-binaries-r1.log
5d0d4849c4ed6c9306d2501ecec90b35a788ca44a249fc28702f9b82dbc4947b  conditional-native-anchor-startup-dco.log
833eab56b8ad619fd74058a0a7c5ab30fbe52044f400794d5f1b6e8843ccc7cb  conditional-native-anchor-startup-doctests.log
646eb5e282691a994b5db9939334837ddbfa013878d7836be8ca53cde125c463  conditional-native-anchor-startup-executable-r1.log
3bd943b4b076ea61803705edaad30b72eff93c1563e2c7b476fd399ff3aa6a3f  conditional-native-anchor-startup-focused-r1.log
062e38c4570cdf67fde438c0e23b83ce6a26a7cfc09721d095b25b799a96d9ab  conditional-native-anchor-startup-focused-r2.log
a3523f5a9ed611a9141ef1646de4cffc4e83d4943d3750c36fe6ecde910f2843  conditional-native-anchor-startup-focused-r3.log
940109e188c31ab589a89a00721b1ebfeb556044e543a9f1808636b3c50517ab  conditional-native-anchor-startup-full-r1.log
cc9e2f21ff0dfb9478b92de291518b8c415997d956c64da21006b09475f653d3  conditional-native-anchor-startup-full-r2.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-anchor-startup-hygiene.log
6281a5b9cef98de627089c28141cac8a2aee9ca80b3ea3c0abe51bf167db433d  conditional-native-anchor-startup-integrated-check.log
4e5caae11519b1fc70c2e85880212f954d1f684cead84085ac0d48af83217dfa  conditional-native-anchor-startup-lifecycle-crash-r2.log
3e344d25b0ecd367da228492bab1578839847d8f46201489b1c9be93b6aa5e06  conditional-native-anchor-startup-parallel.log
eb64edcd4a397642c7f4d9c37a29cc3829986580759ecfff1efc71c2c0b003ae  conditional-native-anchor-startup-static-v3-elf-check.log
0009efb861bfdcaa4befe75dc6c0f5577f67c9b90bbc1235cc1fe5395ddc3b23  conditional-native-anchor-startup-static-v3-profile.log
b68ea91c357d14320fee8399ad44dfe1d0f5066ebd34b86e2c6d720f3b7dfc47  conditional-native-anchor-startup-static-v3-r1.log
7aae20b931cf3fa646b73271606495eb402be26b2a30f6a78a04c5ce1131d2ef  conditional-native-anchor-startup-static-v3-r2.log
bd478641a5dd187a5a9ecec2d50a668abefd3ae22ee84f3f3884c0a00f55ba8d  conditional-native-anchor-startup-static-v3-smoke.log
1363b74d38f8c6ca9fbbf2af9f84147ce13c9eef9b0c982e559f8ee4e87a9ba1  conditional-native-anchor-startup-static-v3.readelf.txt
```

## Remaining Gates

1. Migrate protected helper admission/preparation and terminal exec to actual
   native policy, supervisor, deployment, provisioning and key owners. Preserve
   native lifecycle custody and all same-family context slots through staging.
   Reissue the native key and use native open-or-initialize without extracting a
   V1 raw key or introducing another persistence engine.
2. Integrate native root coordinator preparation/launch with explicit per-attempt
   accounting for wait/release/reap paths. Validate exact readiness descriptors,
   exec EOF, live pidfd and protected endpoint identity. Execute genuine protected
   reissue, startup/readiness/recovery and cleanup on a suitable host.
3. Complete native compiler admission/preparation, postchecks, invocation finish,
   revalidation, V5 publication and SubjectV3 transport in the single production
   pipeline. Backend receipt admission remains V1; conditional finalization still
   refuses. This startup implementation does not close those gaps.
4. Complete source/machine/numerical proofs, generic non-AMD validation and all
   47 target-matched GPU runs. Numerical differences require explicit
   compiler-proved bounds, not unchecked tolerances.

Fresh SSH probes still failed DNS for mi350, mi350-2 and mi300x. No remote job or
scratch directory was created. Shared source worktrees and reports were preserved.
Normal pushes to both remotes and an issue-status update remain required; this
checkpoint does not credit either remote main or the tutorial site as updated.
