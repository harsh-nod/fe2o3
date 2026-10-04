# Disabled debugger type-anchor qualification — 2026-09-27

The disabled physical-v4 source package now retains debug information for its
complete loaded-maintenance scratch type through a 213-byte internal
const-pointer identity function marked used. It adds no scratch fields, owner
construction, callback, mutable API or runtime call. Public activation, selection,
capture and publication remain disabled.

The complete five-file patch preserves the prior 34 transformations and adds
one final anchor transformation. All eleven changed package files have complete
forward/inverse checks; sixteen other package files and the v3 package remain
unchanged. The selected external source remains 63 files, now 2,194,926 bytes
under its unchanged 2,195,456-byte cap (530 bytes of headroom).
The extracted CPU scratch definition matches the native header.

## Qualified source and CPU checks

The complete patch passed forward and reverse checks. Both selected source
stages and the installed amd-dbgapi header passed validation. All 45 Node source
controls passed, including four new anchor controls. Strict C++ builds passed
with warnings treated as errors. CPU execution passed 264 maintenance groups /
1,033 checks and six first-poison groups. The measured owner sizes remained
1,992 bytes before and after. No debugger or GPU target was executed by these
CPU checks.

Source/build receipt:
`2565c6deba181712471b9907d498ee38abd22173d53b3fcf63ce85224cddb230`.
CPU probe receipt:
`243c1cedbcccdbe40793609a1872e083b03abcd4a3af0120b3a0f9748e8edf73`.
Independent package source review:
`db768b43627b6162564da0dab193a28019940f73b238a3977b3eb9a3f14e453a`.

## Remaining runtime boundary

A separately bound private observable debugger completed all four build phases
and complete build-evidence custody. These facts do not establish nine-type
layout, startup, successful capture or native qualification. The static check
currently needs a metadata/artifact pin-domain correction before layout
inspection; no missing type size is inferred. The unchanged nine-type requirement
must still pass on actual built products.

This public package remains disabled and does not inherit private runtime
availability. Complete current startup closure, helpers, deployment, bounded
startup/postflight and native capture remain separate work.
Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**; no global compiler pin
change or public capture activation.
