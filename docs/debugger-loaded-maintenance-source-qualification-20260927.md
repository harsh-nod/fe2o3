# Disabled loaded-maintenance debugger source checkpoint — 2026-09-27

The new `tools/rocgdb-one-stop-native-adapters-v1/physical-v4` package
adds the first-loaded-ACK maintenance fix and first-failure diagnostics as a
separate, disabled source overlay. Existing physical-v3 remains unchanged.

Five source leaves match the CPU-qualified private implementation exactly.
Thirty-four reversible edits preserve the diagnostic and loaded-maintenance
steps. The same 63-file selected-source boundary totals 2,194,713 bytes, below
the unchanged 2,195,456-byte cap. All three source availability functions and
all four manifest authority flags remain false.

The narrowly owned maintenance epoch retains strong target references from the
first loaded-success acknowledgement until the first subsequent callback. It
preserves original acknowledgement/flush ordering and adds no resume credit,
acknowledgement, stop, capture or retry. Retained references can delay a final
target close during exceptional teardown; arbitrary teardown equivalence is
not claimed.

## Qualified relocated source

A fresh exact package/ancestor layout and disabled source projection passed:

- Forward and reverse patch checks; both exact 63-file source stages and the API
  header check.
- All 41 JavaScript source/placement/negative controls.
- Two strict C++ builds.
- Real-helper CPU mocks: 264 groups and 1,033 checks.
- First-failure CPU controls: six groups; old/new owner sizes both 1,992 bytes.

The source/build receipt is
`2d2bec8ca2543c98efaadb72e698e03c4fc27c29fa7198280c7e8c392f0c7070`;
the CPU-run receipt is
`a392dbf1a086c9bf4c64bcae909032c7b605b6ba80fabe865ee06317a4c02713`.
These are mocked CPU/source qualifications, not physical GPU capture evidence.

## Separate private full build

A fresh private debugger completed preparation, configuration, bootstrap and
full build. Every phase matched the exact independently checked read plan and
passed source/tool/input postflight. The final build used 4,129,554,506 charged
bytes and 545,734 reads, within the unchanged per-command 4-GiB / 600,000 limits.
This accounting does not include compiler-internal or module-loader I/O.

The debugger is 199,593,392 bytes, SHA-256
`d02e69f25ae1690bf424d3fa44a7c458a34c2d9445055bff093f071fcf717ed4`.
The adapter object is 8,887,312 bytes, SHA-256
`a91de2b2aea7dc89edfd0cabb051ce7a202c6f61e6d34308fa836b0ad8551caf`.
Full-build receipt:
`ae832714f1064e7898b3fdef17424bb05ab09672ac51b4d3bd2f900c145cc6ef`.

No debugger or target was executed by those compile phases. The private build
inherits active source flags; it must not be confused with the disabled public
projection or treated as a public capture capability.

## Remaining acceptance

Actual compiled adapter/scratch layout, complete build-custody handoff,
fresh loaded-runtime closure and startup, controller/family/replay checks,
and a separately bounded one-attempt native qualification remain required.
The previous commit-site-7 native failure remains a failure. No old lease is
reused, and no capture success is inferred from the new build.

No global compiler pin or public capture gate changes. Accepted broad exits
remain **M1/V1/V2/U1/U2/U3 (6/18)**.
