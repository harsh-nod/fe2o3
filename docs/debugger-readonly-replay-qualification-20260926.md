# Read-only debugger deployment replay — 2026-09-26

The staged debugger deployment passed an actual read-only replay. This checks
historical evidence and deployed bytes; it does not establish a fresh stopped
wave, live process identity, capture authority or successful native execution.

## What ran

Thirty replay controls passed, followed by the actual validator over 27 deployed
products, 38 runtime sources and 16 startup import modules. The validator accepted
93 benign-case records and 310 startup records. Source parity retained all 504
native admission rows. The launcher was hashed, not imported.

The explicit driver ledger charged 14,029,167 bytes and 526 read calls, including
EOF, under its unchanged 256 MiB / 8,192-call / 60-second limits. Startup,
artifact, candidate and observed snapshots retain their separate original
ledgers; these are not a combined 512 MiB allowance. Module-loader I/O is outside
the explicit driver ledger. Synchronous validator deadline checks are cooperative
and require the finite outer supervisor.

The wrapper calls the existing benign and execution-snapshot validators with the
same guard, preserving the startup validator's original two source sweeps.
The source packet and independent review were pinned before execution.

## Evidence

Controls receipt:
`03cd23c0457fbe1c6d6fec43b8a0aaf9a6817ffc1882a3bd55b431c4cecb604f`.

Actual replay receipt:
`83a84030a13676d2c36793c980325b25f4201d078aba492bf649c34ef0b7b70f`.

Actual replay output:
`60d3ff3cd3fc468b4ecaccd2c783b1b83726c4ac095d6d156ecfdbc0c6e99b44`.

Preceding staged build receipt:
`e0bbbeb63445bc1c5a43b73533a48a94273e3c9614e11677112ff2f035f8583f`.

## Still required

No scope owner, controller, debugger, target or GPU was invoked. No current
process census, native lease, physical capture or descendant-quiescence proof was
created. A fresh native run still needs a closed source/tool roster, bounded
root wrapper, fresh coordinated writer-quiet interval, exact process-generation
binding, cleanup acknowledgement and postflight acceptance. Historical startup
and replay records cannot substitute for any of those observations.

No public capture gate, global compiler pin or route-maturity change is made.
Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.
