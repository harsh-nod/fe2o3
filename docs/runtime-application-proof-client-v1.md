# Consuming Application Proof Client V1

This increment implements application-side remote custody and its host artifact
join. It does not enable ordinary native execution or complete multi-GPU admission.

## Ownership

The application consumes its original inherited proof endpoint. The explicit
custodian registration path accepts only root CustodianReady with the exact
registration transcript and one original controller pidfd. Legacy Ready is not a
fallback. The authenticated root route has already admitted the installed manager
and controller deployment; session bytes alone cannot construct this client.

After handoff, the original controller pidfd and exact packet credentials replace
root liveness as the continuing premise. The original application occurrence,
Cargo parent and endpoint object remain checked. Coordinator exit therefore need
not invalidate a surviving controller.

After descriptor ACK, the client requires Active, sends exactly two distinct sealed
read-only input memfds, and requires Proved followed by an authenticated Probe /
Retained round trip before returning a proof owner. Every later probe checks the
same subject, session, credentials and strictly increasing sequence. A failed or
possibly delivered transaction cannot reconnect or retry from a fresh owner.
Drop closes the local endpoint only: no Release or GPU settlement is inferred.

The execution budget starts before registration and is capped at 300 seconds.
Probes are capped at 30 seconds. Caller deadlines only shorten these limits.
Preparation and cleanup are not hard-preempted. The memfd reader uses openat, which
the existing application allowlist admits; no sandbox syscall was added.

## Host Join

The explicit custodian handoff returns RegisteredWorkerV3CustodianApplicationV1,
holding the same publication token acquired before registration and ACK. Neither
the recovered admission nor the controller can be extracted publicly.

Its consuming conditional-fill constructor checks original compiler inputs, target
lineage, singleton descriptor, derived host contract and finalizer before admitting
FD195. The one-use production compiler audit runs immediately, before the longer
proof request, with its timeout capped by the remaining caller budget. It retains
the independently installed compiler deployment provenance.

Only the original publication's exact payload and original envelope enter the proof
request. The returned subject must match the local lineage, deterministic request
challenge and conditional-fill boundary. Obligation, receipt and key identities
remain authenticated matching data, not a reconstructed local executed proof.

RemoteConditionalFillArtifactV1 retains that entire chain and can revalidate it.
The one evolving client stays inside the original inherited descriptors. Its
mutex is never held across publication callbacks; contention rejects immediately
instead of extending an absolute deadline. Existing recovered and local-pending
owners remain Send + Sync. No unconditional executable or native permit is minted.

## Qualification Boundary

Cross-UID root transport campaigns use the consuming Rust client with a synthetic
controller and inert input/subject bytes. They exercise exact sequencing, original
pidfds, root loss after handoff, controller loss, replay, substitutions, sealed
files, deadlines and terminal failures. A separate component campaign installs
Cargo's unmodified syscall allowlist after entering the test helper. It does not
claim production pre-exec inheritance or initial-exec supervision.

Host CPU checks cover subject matching, existing admission regressions and opaque
ownership. The new host constructor still needs a composed positive campaign with
the observed manager handoff, actual controller proof and production FD195 audit.
These Rust transport checks are not a new formal proof.

## Next Multi-GPU Work

1. Qualify that complete application/manager/controller/auditor composition.
2. Join the remote artifact to conditional charged argument preparation and the
   existing mandatory per-device native fill constraint, retaining shared custody
   through both invocations' settlement or quarantine.
3. Package the complete proof deployment and switch ordinary host/supervisor routing.
4. Run ordinary N=65/G=128 fill on two free MI300X GPUs, then completed-output
   staging, settled upload, PUBLIC XGMI and guarded readback in both directions.
