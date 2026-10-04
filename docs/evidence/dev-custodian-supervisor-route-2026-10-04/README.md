# Custodian Supervisor Route Qualification

Source: `967061565ee305d8bc843efe1c79c9ecd67ac085`, based on
`d1f1232d05e6223671e28c3f1b431133f8d07ff7`, on
`codex/r65-runtime-drain-versions`. The source commit's SSH signature was verified.

## Results

| Selection | Passed |
| --- | ---: |
| Runtime protocol unit tests | 49 |
| Compiler client unit tests | 33 |
| Supervisor unit tests | 66 |
| Cargo binary unit tests | 407 |
| Broker unit tests | 205 |
| Coordinator unit tests | 31 |
| Doctests across the five library packages | 120 |

The 791 unit passes exclude nested helper reruns. Default unit selections contain
33 ignored entries; this is not a claim that every ignored campaign was run.
Strict all-target Clippy passes with `--no-deps` for all six changed packages,
plus the feature-enabled Cargo vertical-test/binary selection. Formatting and
whitespace checks pass. An initial dependency-wide lint run found the unchanged
`PresentFieldV4` Default warning in `fe2o3-semantic-import`; it was not modified.

The isolated real-root composed campaign passed all six scenarios: descriptor,
roster, delayed transitions, clone3 fallback, early registration cancellation and
the new custodian-publication case. The latter exercises context 4, authenticated
cross-UID supervisor registration, original issuer binding, custodian-specific
Cargo readiness and reverse publication. Exactly one original custodian owner
must be extractable with its matching transcript. Deliberate owner drop contains
the original application before manager Ready; the app remains unadmitted, Cargo
fails without ACK, the issuer is reaped and the registry drains. Legacy cases
cannot produce a custodian extraction.

## Limits And Next Work

The composed campaign uses test keys and synthetic carriage. It does not establish
successful custodian startup, genuine compiler receipt acquisition, production
FD195 audit, completed proof, native execution, multi-GPU operation or performance.
No new formal theorem or full-workspace test result is claimed.

Next complete the installed proof manager/controller/analyzer/Verus deployment,
then qualify fresh compile-to-application admission and two-GPU execution. See the
[route design](../../runtime-custodian-supervisor-route-v1.md) and
[multi-GPU critical path](../../runtime-multi-gpu-critical-path.md).

The root campaign's private namespaces exited and no campaign descendants remained.
Owned static-build/evidence scratch is removed after publication. MI300X was not
used; unrelated worktree files and other builds were preserved.

## Artifacts

`evidence.tar.gz` contains logs, exact source patch/identity, selected binary hashes,
static ELF inspection, build metadata, commands, review notes and a SHA-256 manifest.
Commands were executed individually, not by invoking the reproduction script.
The archive excludes generated executables. Initial harness layout failure is
retained separately from the successful final root campaign.

Source patch SHA-256:
`a979f09e9da8f5cef612c59c8e885a249af5381d29c6ef3e0729c93d8c44dcd5`

Archive SHA-256:
`0fc8165b1710e7fdc4ffbf29dfa10d1d0dadc78139ef120600a8fcf167188df8`
