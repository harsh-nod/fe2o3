# Broker-First Configuration Admission

Date: 2026-09-27. Base: `45f0ece1922312695ba7f74dbe06e3d2732c60db`.
Follow-up to [native child preparation](conditional-native-child-preparation-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**The selected wrapper ordering changed. Native V3 execution profiles remain
unselected; no complete M0-M7 milestone or additional GPU qualification is claimed.**

## Production Change

The compile-shaped wrapper previously opened the configuration, pinned the
worker and read providers before authenticating its broker. It now reads only
the routing claim first, validates the retained compiler/library identities and
receives authenticated capabilities before loading the configuration. The
existing peer/executable checks, one-use invocation permit, challenge response,
descriptor count/type checks and compiler-closure revalidation remain in place.
Reading the route alone is explicitly not authentication.

After parsing, the exact transitive configuration identity must equal the
authenticated broker binding. Both identity domains already bind their schema,
canonical manifest, worker and provider inputs. Environment namespace/identity
checks remain additional requirements, not substitutes for broker agreement.
No broker wire format, request/response authentication scheme or schema changed.

Two duplicated capability-intake methods were replaced by one method consuming
the already authenticated transfer. Ordinary and nonselected observer units
still release invocation authority before managed-attempt creation; selected
source/ISA units retain it until their exact config/unit/attempt release. A
configuration or selection failure drops the transferred owners and stream
without creating a managed attempt. Existing broker deadlines still apply;
this change does not grant unbounded time for configuration preparation.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded
Cargo command at a time, with compiled inputs frozen during each command:

| Command / Filter | Result |
| --- | --- |
| Cargo binary: `binding_wrapper::lifecycle_tests` | 7 passed |
| Cargo binary: `build_config::tests` | 16 passed |
| Cargo binary: `capability_broker::platform::tests` | 9 passed, 4 failed |
| Cargo integration: `production_build_config` | 19 passed, 1 failed |
| Four-package `--all-targets` check | Passed; warnings remain |

No tests in these runs were ignored or excluded. The all-target check covered
`cargo-fe2o3`, `rustc-codegen-fe2o3`, `fe2o3-hsaco-finalize` and
`fe2o3-compiler-execution-client`.

New behavioral coverage enters the actual wrapper in an isolated subprocess
with a sealed `/bin/true` compiler fixture, a malformed broker route and a
missing manifest. It requires the broker diagnostic, not a manifest-read error.
The fixture is never executed and establishes no protected provenance. Other
new tests reject omitted, changed and cross-schema broker identities despite
matching environment declarations, and verify configuration presence/identity
changes both request and response authentication bytes. A source-order test
supplements these checks; it is not positive end-to-end execution evidence.

The first lifecycle build failed on two missing test trait imports; the next
run exposed an unsealed fixture rejected by the existing compiler-image guard.
The fixture now uses the existing sealed-image API. The final lifecycle run
passed without relaxing production checks.

Four broker tests failed during Unix-socket I/O: the two invocation-dispatch
tests, `observer_sink_is_returned_only_after_exact_server_preparation`, and
`v1_release_waits_for_the_frozen_prepared_ack_and_rejects_substitution`.
An independent socket-pair probe confirmed `EPERM` for `send`, `shutdown`, and
`getsockopt(SO_TYPE)`. Pair creation succeeded. The integration failure remains
`production_runner_rejects_no_envelope_marker`: supervisor socket inspection
fails before the expected envelope diagnostic. These five failures are not
counted as passes or as successful broker/protected-runtime execution.

Rustfmt and whitespace checks passed. All commands terminated and private
`/tmp/fe272broker.gYIoRI6q` scratch was removed; no remote jobs/files were created.
Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
437a9ffb3a6c64841854e0b448d81a1338dbdd9a419066881b95faf458f34087  broker-first-wrapper-lifecycle-tests-r1-20260927.log
4d2b8fb0392f8a8a874f097b156c268f2d4c7a2498435b616e6a32436e2acce4  broker-first-wrapper-lifecycle-tests-r2-20260927.log
79ce5677625bcd486abd40f2f21e9cac60e2901719f3e38683ad25285e503e93  broker-first-wrapper-lifecycle-tests-r3-20260927.log
4e7bd6ae04d348981e0c98278ab95581fd4e73d6536db95f184879cc268eb769  broker-first-configuration-tests-20260927.log
e8ac2b637812995973b099ebabf973ac36150cdd6c22d351385bdd5dee99c91e  broker-first-broker-tests-20260927.log
0675892967c1d25c0900dc6831425d505ab70f1e212f29cc46ea3e365cae2379  broker-first-production-config-integration-20260927.log
9591f301dc9c966657f110796b1c7877910de539f2f714c437404d21a8d4cf78  broker-first-all-target-check-20260927.log
```

## Remaining Integration

The active wrapper still constructs the existing unmetered configuration and
uses V1 execution profiles. Authenticated configuration ordering is a
prerequisite for selecting native recipe admission, not its completion. Native
profile transport, authority-release admission, original attempt-account
ownership, bounded live invocation capture and Command/spawn accounting still
need integration. No fresh budget or legacy conversion was added here.

Positive protected continuation, conditional publication/recovery, sealed
authority and generated safe host consumption, machine/numerical refinement,
target-matched 47/47 qualification, migration/removal and release CI remain open.

The native agent service again reported its thread limit, so implementation and
review were local. All three SSH aliases failed DNS. The GitHub API reported both
remote main branches at `28dc7433e8375c8d55b12971e698f059b3d260b5`, absent from the
local object store; Git fetch failed DNS. Publication outcomes must be reported
separately from this local validation record.
