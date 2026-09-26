# Synchronous Device Account And Native Supervisor

Date: 2026-09-26. Compiler implementation/test snapshot:
`68c15e89856054e25465f8ce2aea425f56909ed0`.
Supervisor handoff implementation:
`6de7f89717099763a863bc39e25fbaac6f52a3fb`.
Fixture-coverage follow-up and final integrated-check snapshot:
`cadac3716f7c2e632f2f25104ce9fd9c97587390`.
This continues the [native client checkpoint](conditional-inherited-completion-20260926.md).
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7
remain open. No protected activation, machine-refinement, GPU-launch or 47/47
completion is claimed.

## Compiler Ownership

The normal backend now constructs one original TARGET account around its
synchronous device phase: execution admission, context capture, monomorphization,
authenticated collection, lowering and publication. The account borrow ends
before LLVM codegen starts. Host-only compilation constructs no device account.
SOURCE retains its separate original account.

Compiler stage owners retain the genuine protected invocation; the early-admitted
execution session stays outside those owners and is consumed explicitly at
publication. This removes the self-borrow obstacle to native client integration.
It does not activate that client: normal execution admission is still V1 and
conditional publication still refuses. Existing admission, source, proof,
invocation revalidation, receipt and transport checks remain in place. V1
operations have not acquired new TARGET metering from this scope change.

Direct helper tests exercise success, error and unwind with one callback. They
check original work-account identity, retained work/storage and first denial
during owner drop, preservation of the panic payload, and the host-only
no-account path. These are accounting/lifetime tests, not socket-closure or
protected execution evidence.

## Supervisor Admission

`5db1e3d53` adds nominal V3 program and authority admission using the actual
sealed PolicyV3 and SigningKeyV3 capabilities. The existing native V2 bodies are
shared privately, preserving their admission order, logical accounting and
error precedence. No decoded V2 authority is converted into V3. Runtime/image,
policy/key, credential, protected root and external-anchor joins remain checked.

Program/authority admission alone does not implement V3 launch,
process ownership, deployment or listener activation. The shared static runtime
measurement is a policy-neutral runtime contract, not a policy-family conversion.

The handoff follow-up adds `AcceptedCompilerExecutionHandoffV3`, using actual
FrameV3/ManifestV3 owners and the independently admitted V3 supervisor. Admission
and revalidation share V2's original body; V2 launch-transfer helpers remain
V2-only. The unchanged finite receiver drains ancillary rights into RAII owners.
The wire layout is shared, so a real V2-policy-bound frame is rejected at the
V3 policy-identity join, not at an invented version tag. Accepted handoff custody
does not establish readiness, process ancestry, execution or GPU authority.

## Validation

Pinned nightly-2026-04-03, locked/offline Cargo, one job, 12 GiB VM ceiling,
20-minute command bounds, disabled GPU visibility, and fixed source per build.

| Check | Result |
| --- | --- |
| Focused backend pipeline, account and source-wiring tests | 22 passed; 0 failed/ignored |
| Conditional preparation/refusal regressions | 5 passed; 0 failed/ignored |
| Supervisor program filter | 10 passed; 1 failed |
| V3 authority tests | 2 passed; 2 ignored opt-in container roles |
| V2/V3 handoff tests | 5 passed; 2 failed; 3 ignored opt-in container roles |
| Final supervisor documentation tests | 71 passed; 0 failed/ignored |
| Client/issuer/supervisor/broker/Cargo/backend all-target check | Passed; Cargo target kinds, not GPU targets |
| Changed-file formatting, whitespace and hygiene | Passed |
| DCO through the fixture follow-up | 65 commits signed off |

The program filter passed all four new V3 cases and all six existing native V2
program cases. Its incidental legacy
`tests::key_from_another_policy_cannot_bind_to_the_program` case failed at
service-peer socket-domain inspection with EPERM, before the intended key
assertion. This is not an all-tests-green result. Protected container roles and
GPU runs were not executed. Independent source review found no concrete defect
after requesting the now-added outer-phase success/error/unwind tests.

Both handoff socket-shape cases fail at `control_shape` with EPERM. The passing
handoff cases exercise each family's ordered CLOEXEC transfer, malformed packet
cleanup and deadline behavior, plus the V3 policy join using actual V2 wire.
Distinct-UID admitted-supervisor success/quota/role fixtures require their
explicit disposable-root environment and were not run here. No failure was
changed into an automatic skip and no production guard was weakened.

Independent handoff review found no actionable source bug. Its descriptor-leak
coverage finding led to a process-wide FD census before requests and after
success/refusal, alongside the control-identity assertions. This detects net
installed-FD leaks, not descriptor substitutions, queued kernel references or
unwind inside admission. The follow-up compiled; its isolated-root assertions
remain unexecuted. More credential, wrong/dead-pidfd, client/anchor-PID and
simultaneous-fault precedence cases also remain to be added to the V3 matrix.

Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`.

| Log | SHA-256 |
| --- | --- |
| conditional-device-scope-tests-r2.log | `af57350425ade2a8ac8aeb70c0712fecb2778cd57f8df316dc5b33032dbf24a6` |
| conditional-device-scope-preparation-tests-r1.log | `f028983e5575b99a5423e1e1dd20799a0d953d585404d9b9d4b883fff4c63dd2` |
| conditional-supervisor-program-tests-r1.log | `c4f2557799e44c323b0c060386102faaeacf0442c36a5a8d7af5bd3fc42174b1` |
| conditional-supervisor-authority-tests-r1.log | `7dac4d62c0657703c627f56cdb8ebc8f9d7bbd112098c693cb58d8ad5a7f0904` |
| conditional-supervisor-doctests-r1.log | `10f507f1e77d46b9e17a0511d1be70e81246f0fde5a1667b1afe4cf68f81864d` |
| conditional-device-scope-integrated-check-r2.log | `37d362715ce6ad55031db7ac6e547283807b85e034fdb298abb8c298ad441311` |
| conditional-supervisor-handoff-tests-r1.log | `d2ad4b6ecf6da99443c1c7841b3009ee19e5a310b9736d1d385d862b1da3c687` |
| conditional-supervisor-handoff-tests-r2.log | `1f7c4fa66976c9946b7e6b4b9f13729fbb22519022807bf77a0472a914195387` |
| conditional-supervisor-handoff-docs-r1.log | `6b62b9c7b87d357e560dad3f24b6dec821bc59492e042e922a19bb9842ea6e1d` |
| conditional-device-scope-integrated-check-r3.log | `500a9fe2e4e03dfa7e55791926d8f609d8c088f3f479659b20c12b653c909d56` |
| conditional-device-scope-integrated-check-r4.log | `48e507f5cbfbd0236e2edc43f443961947c2f476a103ee041545c49f5ffeb46f` |

## Remaining Work

1. Extend and execute the isolated V3 handoff matrix, complete V3 supervisor
   launch/process/deployment, and pin the policy family, profile and readiness
   independently at the parent boundary.
2. Admit V3 execution in this synchronous scope and consume genuine conditional
   preparation through postchecks, invocation finish/revalidation, V5
   publication, SubjectV3 acquisition and receipt transport.
3. Complete trusted parent history/policy intake, locked V5 recovery before
   mapping, exact occurrence consumption and concrete machine refinement.
4. Run protected validation and the complete target-matched kernel matrix before
   changing tutorial completion claims or release gates.

Normal fetches from both remotes still fail DNS; current public main must be
fetched and merged before successful normal synchronization. GPU SSH aliases
also fail DNS here. No new remote jobs or scratch directories were created;
no source worktrees, reports or unrelated caches were removed.
