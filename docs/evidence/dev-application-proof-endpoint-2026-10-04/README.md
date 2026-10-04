# Four-Input Application Proof Endpoint

Source: `ec89634294eaf50cd4e128935ab21d79043f0f2a`.

This completes the inherited endpoint prerequisite on the
[multi-GPU critical path](../../runtime-multi-gpu-critical-path.md), not authenticated
application registration, remote proof custody, or ordinary two-GPU execution.

## Implementation

Cargo creates a nonblocking CLOEXEC Unix seqpacket pair before spawn, enables
PASSCRED, eagerly binds both abstract addresses, and commits the actual application
endpoint as occurrence slot 4. The non-owning pre-exec setup revalidates the original
object. Cargo closes its application-side alias immediately after spawn and retains
the move-only counterpart through ACK and failure cleanup in the existing prefork
allocation. FD195 remains exclusive to compiler auditing.

Both host admission paths claim all available inputs before propagating a claim
failure, close rejected aliases once, remove the proof environment value, and
independently reconstruct the fourth occurrence. Three slots without the proof
environment and four slots with it are distinct profiles; mixed profiles reject.
Successful admission retains the endpoint without exposing proof authority.

Root observation consumes original process owners and the complete registration
binding. It checks client/parent association, exact coordinates, source CLOEXEC and
status flags, Cargo creator credentials, reversed abstract addresses and distinct
socket objects. Only application-endpoint facts survive; temporary pidfd_getfd
aliases and ACK writers close before return. Post-ACK revalidation tolerates ACK
slot reuse but rejects changed proof inputs.

## Qualification

| Check | Result |
| --- | --- |
| Runtime protocol and client, including doc tests | 101 passed |
| Cargo main, rustc wrapper and linker proxy | 444 passed; 5 ignored |
| Host unit tests | 223 passed; 3 ignored |
| Broker unit tests | 207 passed; 12 ignored by default |
| Host and broker doc tests | 106 passed |
| Strict static application integration | 17 passed |
| Explicit isolated root observation campaign | 1 passed |
| Changed packages and feature-gated fixtures, strict Clippy | Passed |
| Scoped formatting, whitespace and frozen source checks | Passed |

Total: 1,099 passed, excluding nested helper reruns. The root campaign explicitly
runs one default-ignored broker test; 19 other deployment/hardware tests were not
run. CPU qualification used pinned nightly 2026-04-03 and the existing no-fork
application filter, with no sandbox widening and no GPU work.

The root campaign used a private PID/IPC/UTS/network namespace, read-only host mount,
private proc/dev/tmp, NoNewPrivs, and only SETUID, SETGID, SETPCAP, CHOWN, DAC_OVERRIDE,
SYS_PTRACE and KILL capabilities. It qualified cross-UID four-slot observation,
application message credentials distinct from Cargo creator credentials, continuity
after message traffic, ACK EOF/reuse, and proof HUP while observation remained alive.
Mutated flags, closed/replaced slots, application-created substitutes, foreign
bindings/counterparts and slot-4 substitution reject without leaking aliases.
A separate same-Cargo two-pair test isolates address/object association rejection.

Static single/roster consumers pass. Six new proof-profile mutations reject at the
expected gate, and existing process creation/exec restrictions remain enforced.
Early parse, alias, flags, missing-wire and mixed-schema failures close every named
input. Startup tests retain the proof peer through service/ACK/sandbox failure.
The legacy three-slot root path remains qualified; host legacy shape checks remain,
but this campaign does not add a full legacy three-slot static-host integration.

## Scope And Evidence

Creator credentials and endpoint possession are not authenticated registration.
The four-right supervisor transfer, independent bounded application session,
authenticated root challenge/Ready exchange, fixed keyless proof custodian, and
consuming native invocation join still remain. No new Verus proof or GPU performance
claim is made by this checkpoint.

`evidence.tar.gz` contains commands, final logs, the source patch, 5,083 source/config
hashes, test/static executable hashes, signed-source audit and an internal manifest.
Package Clippy uses `--no-deps -- -D warnings`; broker formatting is scoped to changed
modules to preserve inherited unrelated formatting. Temporary local build resources
were removed after archiving. No MI300X resources were created or changed.

Archive SHA256:
`a0908975f77976b11e72eb28558f31506228fdf06ef315e4f98d13fd27c5e898`.

Source patch SHA256:
`1731ff49f61acf0f7fd96e4a8dd4b9b675307614e42bb44abafc80771e25f7d9`.
