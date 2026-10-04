# Shared Compiler-Execution Slot Admission

Date: 2026-09-27. Base: `68ed9284fc4b450e272777e59f93e64b9ade3e71`.
Follow-up to [broker-first admission](broker-first-configuration-admission-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**This fixes the selected production compiler admission. It does not activate
native V3 execution, complete an M0-M7 milestone or add GPU qualification.**

## Ownership Invariant

Previously the selected entry duplicated the policy before checking the service
input. With lower descriptors occupied and service slot 195 missing, the policy
duplicate could land in that slot. Service admission would then consume a
descriptor still owned by the policy, creating two independent closers.

Both policy slot 202 and service slot 195 must now be live and non-CLOEXEC before
any duplication. A shared guard consumes both inputs on refusal or unwind,
disarms each closer before transferring ownership, and never retries close.
It does not fabricate a Rust descriptor owner for a potentially absent input.
The selected V1 and native V3 entries share this guard; duplicated native cleanup
code is removed. Native accounting still charges the original budget before
descriptor inspection, with unchanged storage and work-denial behavior.

The regression safely demonstrates that the old order allocates the policy
duplicate in slot 195, drops that duplicate, then exercises the actual fixed
entry. Other cases cover absent inputs, unexpected CLOEXEC, invalid policy,
valid-policy/client refusal, unwind cleanup, slot reuse and transferred ownership.
Protocol slots are manipulated only in isolated subprocesses with bounded reaping.
Fixture public keys and sealed files are inert, not protected provenance; the
client-refusal case intentionally supplies a non-socket, not an execution service.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded
Cargo command at a time, with compiled source frozen during each command:

| Command / Filter | Result |
| --- | --- |
| Backend library: `protected_compiler_execution` | 4 passed |
| Backend library: `production_target_account::tests` and selected driver wiring | 4 passed |
| Four-package `--all-targets` check | Passed; warnings remain |
| Cargo integration: `unsafe_source_policy` | 4 passed, 1 failed, 1 maintenance test ignored |

The all-target check covered `cargo-fe2o3`, `rustc-codegen-fe2o3`,
`fe2o3-hsaco-finalize` and `fe2o3-compiler-execution-client`. The driver wiring
assertion is a source-order check, not positive production execution evidence.
The first admission build failed because the test helper's direct rustix
dependency was missing. Adding the existing pinned workspace dependency only to
dev-dependencies fixed that build; the final expanded regression run passed.

The unsafe gate remains red for **54 preexisting per-file inventory mismatches**,
all outside the changed Rust files. Only this patch's three reviewed test blocks
were registered; the gate and unrelated allowances were not changed. The native
production and test files now contain no unsafe blocks. See the explicit
[safety review](../unsafe-code-policy.md#shared-compiler-execution-input-slots).
The ignored test is the existing explicit inventory-refresh command, not a
skipped validation case. No inventory failure is counted as a pass.

Rustfmt and whitespace checks passed. All test/build processes terminated and
private `/tmp/fe272slots.NA4RZxjh` scratch was removed. No remote job or file was
created. Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
60953186dd0d508d6e65cc51277339acd3c2be9e07867c9f03beb161c2659630  shared-compiler-slot-admission-tests-r1-20260927.log
140f620d8dfb3b0e74c0ce73f4a65e315853477c9e749b0babfbe3914cbf2077  shared-compiler-slot-admission-tests-r2-20260927.log
e78e66ed2a547fa55e54d6b56c34f4105d4de1114cc0af411c3978b0e69d0981  shared-compiler-slot-admission-tests-r3-20260927.log
7a59d6476311e065f57ac862f332d1b9ed0cc4b24ddaa15861f0cfe972e0334b  shared-compiler-slot-driver-account-tests-20260927.log
45b9f3329fd55232d8f84d7fc95db325aa4604c316061d87c9258367f6dc010f  shared-compiler-slot-all-target-check-20260927.log
33c3e0f588a325f5815d73f2ab466cdc27fe17741a22f8cf6225b23b36883470  shared-compiler-slot-unsafe-inventory-r1-20260927.log
4fd4d57fb9d135f676b4fd2cae04d936d7688ac8c2aeac7619fdeb07e69c398c  shared-compiler-slot-unsafe-inventory-r2-20260927.log
```

## Remaining Work

Native parent/profile/broker admission, original account custody, bounded live
invocation capture, driver publication and artifact/host consumption must still
be integrated together. Positive protected continuation, publication/recovery,
machine and numerical refinement, the target-matched 47/47 matrix and release
gates remain open. The unsafe inventory needs its own source-by-source review.

Native agent spawning failed at the thread limit; implementation and review
were local. All three SSH aliases and Git fetch failed DNS; the GitHub issue
read also failed connectivity. Publication outcomes are reported separately.
