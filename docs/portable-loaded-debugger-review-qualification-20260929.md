# Portable historical debugger review — 2026-09-29

The repository now contains the pure historical loaded-file profile/planner
APIs and their controls under
[tools/debugger/loaded-profile](../tools/debugger/loaded-profile/README.md).
This is source/data review tooling, not a live debugger adapter.

Five runtime modules are exact copies of the previously qualified components;
the planner changes only two relative imports. All 173 historical profile and
77 planner control bodies remain byte-identical. The shared fixture reader
requires an explicit `FE2O3_LOADED_REVIEW_FIXTURES` manifest selecting all 76
complete, hash-pinned fixture contents. There is no host-path fallback, fixture
download, source evaluation or silent test skip. Raw host receipts are not
shipped in the repository.

The new default suite has 32 fixture-free API/admission controls. It does not
import the explicit filesystem reader or claim historical semantic coverage.
The explicit historical suites perform the retained 250 semantic controls.
The supplied manifest is bounded, read, parsed and identity-bracketed; the
reader checks the 76 fixture contents against source-owned hashes. The root
qualification gate separately pins the manifest itself.

## Qualification

Root executed the exact portable source package on mi350. All 32 default and
250 historical controls passed with zero failures/skips; missing fixture
configuration was separately verified to refuse. Receipt:
`e417637ee3a8f840110b9887ae711aee17d5d4e65296bba7e70db1abeed54ee7`.
Complete request, stream and selected-input pins were rechecked; source, inputs
and tool identities matched before/after.

The explicit 76-role archive contributes 13,854,356 payload bytes. Including the
19,910-byte supplied manifest and EOF reservations, each historical test process
admits 13,874,343 bytes and 349 content calls, within the fixed 32-MiB/1,024-call
limits. Each member is bounded by 8 MiB and the manifest by 64 KiB. Reads use
64-KiB exact-length-or-refuse chunks plus EOF, with named/descriptor identity
checks, final revalidation of all 77 names, and finally-based descriptor closure.
Counters do not cover metadata syscalls, Node loader IO, private API copies or RSS.

## What this does not establish

Historical input-byte qualification is not fresh observation of operational
binaries. These APIs do not establish a complete successor loaded-input graph,
expand the existing operational input cap, renew a native lease, launch GDB,
attach an inferior, dispatch a GPU kernel or capture physical registers/LDS.

The separate bounded loaded-input reader has passed 60 CPU controls, but is not
activated by this package. A supervised no-inferior GDB startup was previously
qualified; it remains startup evidence, not physical capture. The retained
failed startup attempt and all authority limitations remain visible.
V4 and the broader unfinished milestones remain open; accepted exits stay 6/18.
