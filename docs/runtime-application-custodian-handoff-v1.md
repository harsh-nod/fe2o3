# Published Application Custodian Handoff

The broker now distinguishes observation-only registration from mandatory
proof-custodian registration. This is authenticated pending transport custody,
not deployed proof custody or an ordinary multi-GPU launch.

## Ownership

`register_custodian_application` uses distinct authenticated registry request and
response kinds (9/10). Its supervisor-side owner cannot convert into the legacy
registration owner. The selected route is retained through attachment, issuer
binding, the observation gate and exact publication record plus terminal EOF.

Only then can `take_published_application_custodian` move the complete original
application session into `PublishedApplicationCustodianHandoffV1`. No registration
fields or observations are reconstructed. The original application/Cargo pidfds,
root proof peer, binding, transcript, retained observation and absolute broker
startup deadline remain together. It exposes no raw descriptors or Ready sender.

The registry retains a fixed identity reservation after extraction. Both its
16-application capacity bound and original process/session reuse rejection include
these reservations. Reservations conservatively last for the registry's lifetime;
they are not process custody and do not prevent registry shutdown from draining.
A future independently bounded manager must supply any less conservative lifetime
accounting. Restart or process death must never be interpreted as GPU settlement.

The custodian route never enters legacy `Registered` and never sends legacy Ready.
Once extracted, compiler issuer exit, registry cancellation and registry Drop
cannot receive from, shut down or contain the moved application session. Dropping
the pending handoff itself still contains the exact original application, because
no custodian Ready or proof/native authority has been granted.

Revalidation checks the retained observation and the exact original proof
counterpart, not just Cargo creator credentials. It never reopens the historical
ACK slot. Extraction preserves the broker's existing 120-second deadline; the
host's separate 30-second startup deadline is not extended or equated with it.

## Wire Preparation

`CustodianReady` is a distinct inert registration frame (kind 5, 304 bytes),
carrying a canonical 192-byte proof session and requiring one controller pidfd.
The inner session transcript must exactly match the outer handshake transcript.
Legacy Ready remains kind 4, 112 bytes, zero rights. Decoding either frame grants
no controller approval, retained proof, native invocation or settlement authority.

## Remaining Critical Path

The independently approved manager, authenticated coordinator-manager transfer,
application-side mandatory custodian admission and consuming controller join are
still missing. This increment deliberately supplies no public raw-parts escape
or ability to send Ready from an unapproved tuple.

The join must retain the original observation through final pre-Ready checks,
respect the remaining startup deadline, make Ready delivery irreversible before
Activate, and plain-close the coordinator's peer without `shutdown`. After that,
no coordinator or registry path may read the peer. The existing controller's
activation-only cancellation guard must be extended to include Ready delivery.

Then implement the remote conditional invocation owner and qualify the ordinary
two-GPU fill/output/upload/PUBLIC-XGMI/guarded-readback path. The pending handoff
and controller component campaigns are not substitutes for that application run.
