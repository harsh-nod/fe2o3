# Staged Application Proof Controller

This is a component on the ordinary two-GPU critical path, not complete application
admission or GPU launch authority. The independently installed application controller
is separate from the root-only conditional-fill controller. Both share the fixed
launcher, original-child custody, cgroup containment and pinned analyzer/runtime opener.

## State And Ownership

1. Root stages a sealed canonical registration capsule, original application and
   Cargo pidfds, the exact transferred proof peer and a separate root control peer.
   The capsule binds process start times, registration transcript and peer object.
2. The fixed application controller enters the dedicated proof UID/GID and opens
   the pinned analyzer and Verus runtime before resource Ready. It does not read
   or peek at application packets in this state.
3. The registering root must deliver authenticated application Ready and plain-close
   all its proof-peer aliases before Activate. `shutdown` would affect the shared
   endpoint and is not a valid ownership transfer.
4. Exact root Activate/Activated binds the fresh session, deployment and actual
   controller process. The controller then sends application Active and accepts one
   credential-checked, session-bound Request with exactly two sealed input FDs.
5. The controller checks application ownership, read-only mode, full seals, bounds
   and declared hashes. The envelope must also match the registration's original
   envelope identity. It executes the retained conditional-fill producer, preserving
   original immutable inputs and the actual proof owner.
6. Application Proved/Probe/Retained and root Probe report recomputed matching data
   from that owner. They do not serialize or transfer proof authority. Application
   EOF, original process death, replay or protocol failure quarantines custody.

The first root probe shares the original aggregate execution deadline because
analysis/proving is synchronous in the worker. After the first retained response,
each root control operation has its own 30-second bound. Nonblocking root polls do
not wait for child progress; filesystem admission/revalidation remains synchronous.
The resource-Ready expectation uses fixed storage, including after child startup.

There is no application or root application-mode Release message. Before Activate
can be delivered, cancellation contains the original process tree. After a possibly
delivered Activate, dropping root custody aborts the manager rather than silently
releasing it. A future deployed manager must provide whole-cgroup death containment.
Qualification-only containment is not GPU settlement or a production release API.

## Remaining Production Joins

`begin_application` is a root staging primitive accepting raw owned descriptors.
Its own capsule is not proof of authenticated registration: the registering broker
must preserve its original pidfds, parent observation and exact slot-4 counterpart
check through a typed handoff. A different socketpair created by the same Cargo
process must not be admitted merely because its creator credentials match.

Still required are the independently measured manager/coordinator deployment,
authenticated Ready handoff to the application, and a private remote proof owner
retaining the original publication token and one FD195 currentness audit. That owner
must join the selected devices' exact conditional native preparation and retain
custody through both invocation settlements. No conversion to an unconditional
executable is provided by this component.

The runtime now has
`KfdMultiDeviceRuntimeBackendV1::open_worker_v3_generated_only_with_native_peer_copy_v1`.
It reuses existing device/route admission and PUBLIC device allocation policy while
leaving generic, atomic and collective launch gates closed. This removes a backend
construction gap; it does not complete any of the application joins above.
