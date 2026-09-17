# Final V6 Kernel-Input Qualification

Final frozen-source CPU/test cohort above
`13ef09c9ab79ecf2278c0fe99c8266279a0fda04`, qualifying the
[typed-kernel input-lifetime contract](../../runtime-context-kernel-read-leases-v1.md).
This is a development packet, not V6 or A1/A2 acceptance. The complete
[initial cohort](../dev-v6-kernel-read-leases-2026-09-17/README.md) remains
immutable, including its callback-status oracle limitation.

## Results

GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 1043 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 788 | 2 |

Each target has 2,102 passes and 23 ignored, with zero failed, measured or
filtered tests. Complete matching rosters contain 2,125 entries, preserving
every copy-lease baseline entry and adding 20 passing kernel-reader tests.
Strict all-feature/all-target Clippy, formatting, no-default checks, unsafe-source
policy (5 passed, 1 ignored) and 87 doctests pass. All sixteen serial records exit
zero; source and all six unit-test binary hashes match at campaign completion.
The parser accepts both immutable baseline logs and rejects ten malformed-log
mutations. Musl uses `FE2O3_HIP_SYS_DISABLE=1`; unrestricted HIP linkage is not tested.

The source manifest covers 14 files: 10 Rust and 4 docs. SHA-256 identities:

- Source manifest: `72ca297250859c0164c40a461486f388fa5d6f3e48afb3a5786e95786103a372`.
- Base-relative patch: `75fb445ff62e26bc3dfc159a205392f2060233fdfa64040b8e5871a0a60aa50f`.
- GNU/musl roster: `5ef139e49def5742109cfd32b6986fcd55c5d23d6411573c8bfb82143cfdfaf3`.

## Coverage And Correction

The 20 kernel-reader tests cover pure-Read and mixed writable aliases, independent
capacity and partial headroom, shared/out-of-order readers, exact completion
routes, initial and observed failures, cancellation, token drop, malformed handles,
source and marker corruption, stale prepared identities, issue-time graph versions,
atomic/collective forwarding, provisional ownership and Unknown-writer disposal.
The deferred mock samples original backing bytes at modeled completion, preserving
ordered binding ranges and patch offsets; it does not perform kernel arithmetic.

Independent review found that the initial callback-panic test asserted its status
inside the deliberately contained panic. The corrected test records the delivered
status before panicking and asserts exactly one Succeeded outside containment,
both after completion and after repeated observation. Its mutex guard is dropped
before the deliberate panic. Independent correction review found no further
oracle blocker, and both final full-target runs pass the corrected test.

No production code changed between the initial and final cohorts. Only the
callback test and two documentation links changed in the captured source roster.
The initial archive was sealed before that correction; this archive has a fresh
source capture, full qualification and separate seal. Exploratory test-module and
graph-fixture corrections are described in the initial receipt and are not counted
as archived full campaigns.

## Scope

These tests establish CPU Context/model custody and forwarding, not native kernel
arithmetic, machine-refinement authority, formal reader composition or performance.
Malformed duplicate-handle tests establish Context ownership, not two independently
retained native execution descriptors. Private stale-preparation injection is not
a public race; the post-acquisition panic window remains source-reviewed only.
No GPU campaign, new solver proof or matched HIP/HSA benchmark was run. Read-only
remote inventory created no files or jobs and deleted nothing on the shared host.
Accepted checkpoints remain Native R125 CPU/test, Admission R118B C1-C3 and
Resources R116/V3; A1/A2 and #182 remain open.
