# MI300X direct-KFD opaque-checkpoint qualification, 2026-09-03

This is a bounded, caller-bound qualification of one exact active capture. The
producer source was commit `7c2db0c15664fcc2671796f6cc62219fc935cfa9`, tree `0b354b4ec534383eff9b1162c20c34392cbbacc9`. The host
kernel was `6.8.0-124-generic`.

The canonical redacted receipt is
`mi300x-direct-kfd-opaque-checkpoint-qualification-v1.json`. Its raw SHA-256
is `9e9e633b1a5f714662036317290338a86cacc27e5265704bd08b744d4b6ecdf1` over 3407 bytes. Its producer-manifest SHA-256
is `18fdfd09a075ea73d0e7f731954d0a0681172cff163082c369a3a3f509492258`.

The ignored MI300X gate ran the repository-owned finite Wave64 liveness fixture,
joined its target-declared publication to the exact KFD queue, suspended that
queue, captured every nonempty control-stack and wave-state range announced by
the eight public KFD headers, dropped the private zeroizing checkpoint, resumed
the queue, validated target output, observed runtime disable and terminal
telemetry, finished the debugger session, and reaped the successful child before
publishing this file.

The receipt contains relative range metadata and scoped native correlation
commitments. It contains no checkpoint bytes, stopped-state scope, raw address,
native process/GPU/queue/event ID, descriptor, handle, or live selector. Its
self-identity and Git-pinned raw digest detect substitution but are not a
signature and do not authenticate KFD, firmware, hardware, or physical artifact
execution. Capture was sequential and non-atomic; runtime and physical
suspension were not reobserved. Wave, lane, register, PC, source, and target
memory decoding remain unavailable.
