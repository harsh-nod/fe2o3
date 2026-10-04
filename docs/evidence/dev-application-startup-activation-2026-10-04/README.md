# Application Startup Activation Qualification

This checkpoint activates Cargo's application-specific supervisor transfer and
host registration before acknowledgment. It also fixes a readiness-ordering race
that could reject an application which finished immediately. The composed
production startup, retained proof custodian and ordinary two-GPU application
remain unqualified.

Source: `01d51ccab0d35142d85bc369a6d987e13eb3e5a4`.
The signed source audit, exact patch, frozen source inventory, selected binary
hashes, commands and final logs are in [evidence.tar.gz](evidence.tar.gz).

## Implemented Behavior

- Cargo consumes its retained proof peer once, binds the exact four inherited
  inputs to the original compiler handoff, and transfers the application profile's
  four rights using duplicates of the captured original Cargo/application pidfds.
  It accepts only the dedicated 208-byte readiness record and terminal EOF.
  Ordinary compiler transfer remains unchanged.
- One absolute Cargo deadline covers post-spawn child admission, registration
  transfer, supervisor readiness and application ACK/EOF. Invalid transfer inputs
  leave the proof peer retained; successful transfer leaves original child,
  sandbox and reaper cleanup intact. The closed proof-state enum stays inline in
  the existing pre-spawn custody allocation.
- Both production host consumers require four inputs and authenticated root
  registration, including feature-enabled builds. One original publication token
  spans registration and the final descriptor/currentness/session checks before
  ACK. The ACK deadline is checked again after validation. Cargo does not reacquire
  that publication lock until the application exits.
- The returned descriptor or roster retains the registered endpoint and original
  root pidfd through native unload. Session closure by a still-live root fails
  continuity. Envelope-only fixtures use explicitly named test APIs, never a
  fallback in a production consumer.
- Root returns two distinct pipe capabilities within the existing two-right
  registry transport. After authenticating the application, it publishes the
  observation gate but withholds Ready until an exact session-bound reverse
  publication record and EOF arrive. Each registry step performs at most one
  nonblocking publication read.
- The supervisor commits the reverse gate only after sending Cargo readiness.
  No fallible issuer-liveness check follows that commit, so the application can
  ACK, finish its audit, or exit immediately. Neither pipe enters the unchanged
  fourteen-input issuer ABI. A failed reverse publication can leave a readiness
  record queued for Cargo, but cannot release app Ready or complete Cargo's
  separate ACK requirement; original issuer cleanup is retained.

Registration is occurrence/session custody, not proof execution or GPU authority.

## Qualification

The final frozen run passed 1,247 top-level checks, excluding nested helper reruns:

| Selection | Passed | Default ignored |
| --- | ---: | ---: |
| Cargo binaries | 448 | 5 |
| Broker | 201 | 17 |
| Client unit, binary and integration | 56 | 2 |
| Supervisor unit, binary and static-image selection | 63 | 4 |
| Host default unit suite | 226 | 3 |
| Process identity | 23 | 0 |
| Runtime protocol | 45 | 0 |
| Feature-enabled host handoff selection | 9 | 0 |
| Strict static application selection | 18 | 0 |
| Doctests | 153 | 0 |
| Separately enabled isolated root campaigns | 5 | included above |

The 31 default-ignored entries include helper and deployment tests; not all were
enabled. Ordinary tests ran as UID/GID 1000. Existing service-profile fixtures can
skip for identities that cannot represent their required non-root service.

New cases cover single-use transfer, invalid input retaining custody, expired
original deadlines, three-slot rejection without ACK, final validation failure
and expiry without ACK, and live-root session retirement. The static tests run
sealed GNU static applications under Cargo's actual pre-exec filter. Their
positive descriptor/roster cases use explicit envelope-only fixture APIs; both
production consumers reject when root registration is absent. These are not
positive production registration tests.

The supervisor publication matrix has thirteen scenarios. Its fast-exit case
waits for reverse publication and EOF, kills the original unreaped issuer,
observes its exit, and joins that worker before the fixture's Cargo-side readiness
receive. This does not force exit before the publisher returns; source inspection
separately establishes the absence of post-commit issuer-liveness checks. The
fixture injects a local test registration after an ordinary probe launch, not the
production fourteen-input application launch or actual Cargo readiness consumer.

Five campaigns ran in isolated root/UID1000 namespaces: application session,
registry transitions, registry transport, application observation and client
registration. The expanded root session campaign rejects empty, short, trailing,
wrong-binding, wrong-session, observation-domain, missing-EOF and timed-out reverse
publications. Observation alone cannot release Ready. Those campaigns qualify
the individual protocol transitions, not their complete deployed composition.

Seven-package all-target Clippy and feature-enabled Cargo/host all-target Clippy
passed with warnings denied. Scoped formatting and diff checks passed. All 5,095
source/config hashes remained unchanged through qualification; the signed source
audit also checked 23 selected test binaries and three static fixture binaries.

## Next Multi-GPU Gate

Run the complete positive startup with Cargo production context `3`, the strict
host consumer, actual static launcher/issuer, distinct-UID anchor and public
supervisor service/root registry. Include delayed registration, immediate exit
and failure cleanup. This CPU harness is separate from the independently measured
systemd deployment and genuine compiler receipt/current-record qualification.

Then deploy the fixed keyless proof custodian, retain one authenticated proof
owner across both devices, and consume it with each device's exact native launch
premises. Qualify admitted fill, staging, settled H2D, PUBLIC XGMI and full guarded
readback on two explicitly selected free MI300X devices. No new formal theorem,
GPU performance result, HIP/HSA parity or speedup is claimed. MI300X was not used.

## Integrity

Source patch SHA-256:
`0e475bd972e309396dcc688a330a2bcd179065a6bdc505b50648d7910619da08`

Archive SHA-256:
`2b79753b54d575441c97b3821e5285a05ed34adbe4a45c6ac4105877113b16aa`

The archive's MANIFEST.sha256 covers every bundled evidence file. The source audit
verifies the SSH signature, exact committed patch, source inventory and selected
binaries. Disposable build and qualification resources are removed after archiving.
