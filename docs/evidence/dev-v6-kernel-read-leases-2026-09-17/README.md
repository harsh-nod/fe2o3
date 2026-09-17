# Initial V6 Kernel-Input Qualification

Initial frozen-source CPU/test cohort above
`13ef09c9ab79ecf2278c0fe99c8266279a0fda04`. This is not the final publication
candidate: independent review identified the callback-status oracle limitation
below. Its correction requires a separate source capture and qualification.
This complete initial cohort is preserved, not overwritten or relabeled.

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

- Source manifest: `4ac0c45a3f4b24d8c06456879dbb1f86e1595beb28d71152e1c98bf7c112a7da`.
- Base-relative patch: `75a342e193af3e9a9f9410a6c647f41fa0c53b9eae7c53acf882174c386924cc`.
- GNU/musl roster: `5ef139e49def5742109cfd32b6986fcd55c5d23d6411573c8bfb82143cfdfaf3`.

## Oracle Limitation

The callback-panic test asserts Succeeded inside the deliberately contained
callback panic. A wrong status would also panic and satisfy its expected panic
count, so this test does not independently establish the delivered status.
Its other checks still establish panic containment, no repeated backend wait,
and retention of a later reader batch. The planned correction records status
outside the panic boundary and asserts it after callback return and repetition.
No production failure was found by the independent source/oracle reviews.

Before this frozen campaign, exploratory runs corrected test-module routing and
the graph fixture's use of the ordinary rather than reservation-aware poll path.
The final pre-freeze Context run passed all 145 tests. These exploratory results
are not counted as archived full campaigns.

## Scope

These tests establish CPU Context/model custody and forwarding, not native kernel
arithmetic, machine-refinement authority, formal reader composition or performance.
Malformed duplicate-handle tests establish Context ownership, not two independently
retained native execution descriptors. Private stale-preparation injection is not
a public race; the post-acquisition panic window remains source-reviewed only.
No GPU campaign, new solver proof or matched HIP/HSA benchmark was run. Read-only
remote inventory created no files or jobs. Accepted checkpoints remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3; A1/A2 and #182 remain open.
