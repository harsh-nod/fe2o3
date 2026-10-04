# Independent Application Proof Manager V1

This increment connects the broker's original published application owner to the
fixed application proof controller. It does not grant a remote proof lease,
conditional native invocation authority, settlement, or ordinary multi-GPU execution.

## Ownership And Transport

The compiler coordinator remains filtered. A separate keyless root executable,
`/usr/libexec/fe2o3/fe2o3-proof-manager`, owns controller launch and proof-tree
custody. Its systemd unit has an independent delegated cgroup and no dependency
that tears it down when the coordinator exits. The coordinator's 14-descriptor
activation ABI and issuer/application confinement are unchanged.

`/etc/fe2o3/proof-custodian/manager-deployment-v1` is a canonical 160-byte root-owned
0444 record pinning both static installed images and the application-controller
deployment identity. Parent directories and original leaf objects remain retained
and revalidated. The two process images must be different. A byte-identical copy
at another inode is not the independently installed process image.

The fixed SEQPACKET socket is `/run/fe2o3-proof-custodian/control.sock`, root-owned
0600 inside a root-owned 0700 directory. Each role sends its original self-pidfd;
the receiver binds it to socket/per-message credentials and independently measures
the actual running image. No peer pidfd is reopened using its numeric PID. PID,
user, time and network namespaces must match. Fresh two-sided nonces and monotonic
message sequences bind the resulting connection. There is no reconnect adoption.

Begin transfers the original app/Cargo pidfds and canonical binding; Attach
transfers the exact observed proof counterpart and accepted transcript. The
bounded receiver mints `ReceivedPublishedApplicationV1` only after both messages
authenticate. The public controller launcher now consumes this owner. Its former
raw tuple entry exists only as a private qualification helper under `cfg(test)`.

Both sides reserve at most 16 registrations. The absolute original registration
deadline crosses the compatible monotonic clock namespaces; it is not restarted
on transfer, retry or controller startup. The controller's aggregate proof budget
remains separate. Bootstrap and application progress are cooperative in the
coordinator reactor. An initially absent or not-yet-listening fixed socket is
retried only within the original bootstrap and registration deadlines. Other
admission failures remain terminal; an admitted connection is never replaced.

## Deployment Status

The standard compiler-execution bundle does not yet include this manager, its
approval record, the application controller, or the analyzer/Verus resource
closure. The new manager unit alone does not install or activate those resources.
Do not enable the production custodian route until bundle generation, manifest
verification, provisioning and service activation include the complete closure.
Both services must be activated; the manager's `Before=` is ordering only, not
an activation dependency. Its lifetime must remain independent of coordinator
shutdown. The isolated campaign installs the exact resources privately and does
not establish these deployment postconditions.

## Ready And Failure

1. The manager stages the fixed controller and waits for actual resource Ready.
2. Before offering its session and original controller pidfd, the physical owner
   permanently enters ReadyOffered custody.
3. The coordinator verifies the approved deployment, original controller parent,
   exact transcript and original retained observation before app CustodianReady.
4. A possibly delivered app Ready disables pending application containment. Exact
   success plain-closes the coordinator peer alias; it never calls shutdown on
   the shared application socket. Only then may Activate be sent.
5. Exact activation acknowledgment advances diagnostics, not proof authority.

Pre-offer failures use pollable containment. Cancellation failure retains owners.
After Ready could escape, coordinator EOF, startup timeout and activation errors
retain controllers in quarantine. Manager channel failure plain-closes only that
control channel so the coordinator learns failure promptly. There is no automatic
release, slot reuse, or inference of GPU settlement from process/socket death.

## Qualification Boundary

The opt-in `root_fixed_manager_handoff_campaign` executes a measured static broker
test image at the coordinator's fixed path and the actual fixed manager binary.
It uses the real cross-UID observation, handshake and publication path, followed
by fixed-controller resource admission and Ready/Activate. Its C application and
compiler records remain fixtures, not an ordinary compiled Rust application.
The campaign sends no proof Request and runs no GPU work. Cleanup uses an external
fresh qualification cgroup, never a fabricated settlement operation.

The [isolated manager campaign](evidence/dev-proof-manager-handoff-2026-10-04/README.md)
passes activation, disconnect after application Ready but before Activate,
post-activation disconnect, absent/stopped-manager cooperative bootstrap, and rejection
of a byte-identical coordinator on another inode. This is not qualification of
the production coordinator under its systemd policy.

CPU checks cover canonical records, mutation rejection, frame shapes, nonces,
deadline conversion and opaque ownership. These Rust transport checks are not a
new machine-refined formal proof. Existing Worker/Verus proof qualification is a
separate evidence boundary.

## Remaining Multi-GPU Gates

The host and production supervisor still select the legacy registration route.
Switching them requires the complete deployment above. The
[consuming CustodianReady/proof client and host remote artifact](runtime-application-proof-client-v1.md)
are now implemented, but their composition with this observed manager, actual proof
and production FD195 audit remains unqualified. Complete that campaign and the
per-device native invocation join, then qualify ordinary
two-GPU compute, upload, PUBLIC XGMI and guarded readback in both directions.
Manager proof probing, settlement-driven release/reuse, actual systemd deployment
qualification and full ordinary-application proof composition remain open.
