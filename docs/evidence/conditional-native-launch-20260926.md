# Native V3 Prepared And Consuming Launch

Date: 2026-09-26. This continues the
[device-account and native handoff checkpoint](conditional-device-scope-20260926.md).
Prepared-custody implementation: `0af046d1fc5b1bcdc12b97d4fa3cd706b8781dff`.
Consuming-process implementation: `5ec24affd371e360c3804cc54e0906d86338c1a9`.
Initial validated snapshot, including readiness error inference:
`135fce609e2bbb23c8d622bb21507414463cbe21`.
Drop-order regression follow-up and final compile/test snapshot:
`21ed1f138e4fdd9aa2f1831456e55f4ce38b1980`.
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7
and 47/47 end-to-end completion remain open. This is library custody, not
production deployment, protected proof execution or GPU validation.

## Implemented

`ProtectedIssuerSupervisorV3::prepare_launch` consumes a genuine V3 accepted
handoff, retains V3 sealed authority and the exact native descriptor transfers,
and prepares the existing static launch manifest. Preparation, finite manifest
I/O, transfer checks and error mechanics are shared with V2 without converting
admitted owners between families. The isolated V3 preparation fixture checks
quota boundaries, manifest mutation, retained storage and installed descriptor
restoration; it is present but was not executed here.

`ProtectedIssuerSupervisorV3::launch` now consumes that prepared owner into
nominal launched, ready, serving and exited states. One shared lifecycle body
preserves the V2 profile checks, private child-report protocol, gated static
exec, pidfd ownership, finite waits, readiness EOF, publication and terminal
reaping. Cleanup/profile/static ABI machinery is policy-neutral and shared;
it does not admit policy authority. Every request operation retains the original
mutable resource-account borrow through the final exited owner. Pending cleanup
uses the existing separately funded persistent pool.

Readiness must join the exact child PID, manifest and independently admitted
policy. Tests construct real V2 policy-bound wire, decode it using the shared V3
framing, then require rejection at the V3 identity join. This tests the actual
private helper called during both readiness admission and revalidation, not a
manufactured wire-version mismatch. Input, work and scratch refusals preserve
the original account, floor, error categories and first denial.

The public V3 state types cannot be cloned, expose no descriptor, cannot accept
V2 prepared/wait/readiness values, and cannot outlive or release their original
borrowed request account. The supervisor README and existing preparation and
consuming-launch contracts now distinguish V2 runtime evidence from V3 local
checks. No existing V2 MI350 result is counted as a V3 result.

## Validation

Pinned nightly-2026-04-03, locked/offline Cargo, one job, disabled GPU visibility,
12 GiB VM ceiling, 20-minute command bounds and frozen source during each build.

| Check at the initial validated snapshot | Result |
| --- | --- |
| Shared V2/V3 process tests and V3 readiness joins | 46 passed |
| Preparation and handoff filters | 17 passed; 2 failed; 5 ignored |
| Supervisor documentation tests | 100 passed: 1 positive compile-only, 99 compile-fail |
| Client/issuer/supervisor/broker/Cargo/backend all-target check | Passed; Cargo target kinds, not GPU architectures |

The first process build failed at the extracted readiness scope because its
closure's error type was no longer inferable. Explicitly specifying the existing
launch error type fixed compilation; no guard or result classification changed.

Both handoff failures are the known V2/V3 socket-shape fixtures failing with
EPERM at `handoff_native_io_tests.rs:77`, before their intended assertions.
The five ignored cases require an explicit disposable-root environment. None
was converted to an automatic skip. This is not an all-tests-green result.

Independent source review found no new correctness defect in the process
extraction: V2 charge/floor/check order, wait boundaries, field destruction order
and cleanup behavior were preserved. It identified missing explicit drop-order
regression coverage and the still-missing real V3 consuming-process fixture.

The follow-up adds ordered probes around the same private `Funded` declaration,
with an actual `RequestFunding` owner. They assert owner destruction precedes
funding retirement during normal drop, owner panic and nested unwind, while
preserving the original account, work, storage, denial history and panic payload.
Production `Session` remains bound to the concrete original request funding;
there are no production probe hooks or unsafe test aliases.

| Final follow-up check | Result |
| --- | --- |
| Shared process tests and exact V3 readiness joins | 48 passed; 0 failed/ignored |
| Supervisor documentation tests | 100 passed; 0 failed/ignored |
| Six-package all-target check | Passed |
| Changed Rust files: pinned formatting and whitespace | Passed |
| Source-policy check through `4dbf9721f` | Passed after the test-helper annotation |

The first source-policy check classified the extracted test-support file's
intentional panic as production code: it does not follow the parent's external
`cfg(test)` module declaration. `4dbf9721f` adds the documented fixture-only
panic annotation; the panic and runtime behavior are unchanged.

Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`.

| Log | SHA-256 |
| --- | --- |
| conditional-native-launch-process-tests-r1.log | `ea0f9976cb00f6d5278af3e84cf100e44e3e6fb7b8bcc16141a520b70b79fab2` |
| conditional-native-launch-process-tests-r2.log | `e9f95efa85ada2bd3f9677982f57658f966372b56d22c244baaa03b1eba65fe8` |
| conditional-native-launch-preparation-tests-r1.log | `801986efdf025d43e791ff2d4ea8a2b43f55ef621c008cd2102a6809a8cacbb0` |
| conditional-native-launch-doctests-r1.log | `6f25f8506f2a94be279ddf34ade01a2d359cc986f4d890bff47032d1783f990d` |
| conditional-native-launch-integrated-check-r1.log | `bf2ab17947cf09e7fdb1bb5abd586cbb85cd2b4a5d17ff314bc2c22ab8723f9d` |
| conditional-native-launch-process-tests-r3.log | `7e0e46d22cba6aee91a3661974aa402e2777d5b65c578fbad3fec6ea6c52bbec` |
| conditional-native-launch-doctests-r2.log | `7fe2ad463f15159cc1caafeed84ee1ae9f7d73500de34c570dedda7297b10b4c` |
| conditional-native-launch-integrated-check-r2.log | `ce04ff11a3125b178fe38b7e7e2db397df02b899b1e0539e812ca8785854581e` |

## Remaining Gates

1. Extend the actual static readiness fixture and distinct-UID consuming
   coordinator to V3, then run positive publication/exit and refusal/cleanup
   cases. Execute and expand isolated V3 preparation/handoff coverage.
2. Integrate native issuer, listener, session, provisioning and recovery custody;
   independently pin the family, profile and readiness at the trusted parent.
3. Connect the normal backend's original TARGET account to genuine conditional
   preparation, invocation finish/revalidation, V5 publication, SubjectV3 receipt
   acquisition and transport. Current normal execution remains V1; conditional
   publication still refuses.
4. Complete locked recovery and occurrence intake, machine/numerical refinement,
   protected validation and the full target-matched kernel matrix before any
   tutorial or milestone completion claim.

Both GitHub remotes and the three GPU SSH aliases fail DNS in this environment.
No remote job or scratch directory was created. Current public main must be
fetched and integrated before successful normal synchronization; no force push,
privilege workaround or unrelated cleanup is part of this checkpoint.
Posting the status comment to #272 also failed to connect to `api.github.com`;
its body is preserved with the local evidence logs.
