# Native Compiler Child Preparation

Date: 2026-09-27. Base: `357156022bab6b6c9e8c9461b3d5aa2ff6e87b95`.
Follow-up to [native configuration admission](conditional-native-configuration-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**No complete M0-M7 milestone, native production activation, positive protected
continuation or additional end-to-end GPU kernel is claimed.**

## Changes

The native prepared-child owner consumes the admitted V3 profile and native
recipe, checks the recipe's original account address/ledger/storage floor, and
retains an exclusive borrow of that account. It derives and seals the exact
native issuer policy, installs the policy and existing child-channel hooks, and
keeps profile, policy, recipe and pending channel together. Completion uses the
actual child endpoint and existing V3 supervisor readiness exchange under one
absolute deadline. Its ready owner carries the recipe and readiness into the
existing V5 finalization continuation without exposing a detached account or
recipe. It does not add another worker, parser, proof engine or workload selector.

The shared descriptor preflight checks both service FD 195 and policy FD 202
before either installer mutates the command. The currently selected V1 compiler
boundary also uses this preflight. Occupied slots return distinct typed errors;
preflight neither replaces nor reserves descriptors. Each installer still must
reserve its exact slot without replacement, including after a concurrent change.
The application-verifier preparation retains its separate policy-in-parent rule
and does not require FD 202 to be vacant.

The native owner prepays fixed Rust owner/channel/hook state and retains its
reservations on error, unwind and success. The schedule is logical work/storage,
not CPU instructions, allocator capacity or RSS. Command arguments/environment,
pinned executable custody, spawning, retries and process supervision are separate
caller-owned accounting domains. Command is borrowed, not owned: the caller must
drop it on refusal or after its one intended spawn before retiring hook/alias
charges. Later installer failures do not promise rollback of earlier hooks.
The enclosing supervisor must establish compiler success before finalization;
the new finalization method does not itself wait for the child or prove success.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time and compiled source frozen during each command:

| Command / Filter | Result |
| --- | --- |
| Cargo binary: `compiler_execution_boundary::native` | 8 passed |
| Cargo binary: `compiler_execution_boundary::`, exclusions below | 13 passed |
| Client `child_channel`: paired-FD preflight test | 1 passed |
| Client `child_channel`, exclusions below | 5 passed |
| Four-package `--all-targets` check | Passed; warnings remain |

The 8-test and 1-test runs are subsets, not additional independent coverage.
The all-target check covered `cargo-fe2o3`, `rustc-codegen-fe2o3`,
`fe2o3-hsaco-finalize` and `fe2o3-compiler-execution-client`.

Coverage includes exact policy bytes, descriptor identity and CLOEXEC preservation,
command behavior after occupied-slot refusal, parent-only application policy,
exact/one-short work and storage limits, original ledger and terminal storage,
reservation substitution, release on owner drop, invalid PID and expired deadline.
Fixtures use a measured `/bin/true` image for configuration pinning. They neither
execute a production LLVM worker nor authenticate a protected service boot.

The first native run had 7 passes and one failed test assertion: resource denial
was wrapped in the capability error rather than the top-level resource variant.
The corrected assertion inspects typed nested causes and still requires exactly
work denial or storage denial for the corresponding one-short case. No production
check was relaxed.

The Cargo command explicitly excluded
`preparation_installs_exact_policy_and_child_created_service_channel` and
`application_verifier_gets_service_channel_without_policy_capability`. The client
command explicitly excluded `child_creates_exact_pid_bound_service_channel`,
`child_exit_before_admission_fails_closed` and
`later_child_callback_cannot_remove_the_installed_client_peer`. The previously
observed sandbox `getsockopt(SO_TYPE)` denial prevents full endpoint validation;
these exclusions are not passes. Positive native `finish` and full continuation
execution remain unvalidated.

Rustfmt and whitespace checks passed. All commands terminated; the private
`/tmp/fe272boundary.QPgBvXSh` directory was removed. No remote job or scratch was
created. Logs under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
f87ceba9e3f8d9ed5abb97facab3ce02184b66e87923a79532325eaa8132bb9c  cargo-native-child-preparation-tests-r1-20260927.log
5b2da4f47c9efd15b84c6545e33ee939e17464cc5e30a3dbf0851d80795af9a5  cargo-native-child-preparation-tests-r2-20260927.log
aabbc27cb0d630bec894b0cc0e8f44de172ebf586155db6c520faf8177c02cc1  cargo-child-boundary-regression-tests-20260927.log
45dd5248171fb145451b306e1c2da3e71ea9bbf13f758768678c89dac99a81b5  client-paired-fd-preflight-tests-20260927.log
2b98e8a363add1830a448fa52efc12e6f73c35f5a08100c8383da53c5e85a8d7  client-child-channel-preparation-tests-20260927.log
3588b4c331a0b0f6395d9a6d24123fc291c4f1c7a1ccc26405ae78098a2f06c4  native-child-boundary-all-target-check-20260927.log
```

## Remaining Gates

The production broker, authority release and binding wrapper still use V1
execution profiles. The new native owner is compiled but not selected by them.
The next integration must authenticate profile version and configuration
schema/identity together, retain the original attempt account through broker
transport and child preparation, and preserve the existing invocation permit
and exact descriptor checks. Broker wire V3 is not execution-profile V3.

Bounded live argv/cwd/current-environment admission and complete Command/spawn
accounting remain open. Do not substitute initial `/proc/self/environ` bytes for
the live process environment or reset budgets per broker request. Protected
positive continuation, conditional artifact publication/recovery, sealed
authority, generated safe host consumption, machine/numerical refinement,
target-matched 47/47 qualification, migration/removal and release CI remain open.

Native agent creation again hit the service thread limit; this checkpoint was
implemented and reviewed locally, without Qwen delegation. All three SSH aliases
and origin fetch failed DNS. Push and issue-publication outcomes are separate
from these local results.
