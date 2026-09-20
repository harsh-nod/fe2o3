# Actual edited-source/debugger identity join

The private V17 observation path now reuses the compiler's exact checked
source-map projection through a closed borrowed owner. The ordinary source-map
document path keeps its original real binding and every provenance, operation
coverage, synthetic/semantic separation and captured-file check. No public V17
map transport, bundle subject, source-authority reconstruction or schema is added.

Run `phase11-source-machine-r7` passed on mi350-2 at HEAD
`f798cf93d0eff1c7e951488b489e959bae6c2e3e` plus the integrated source join and
breakpoint refusal-ownership changes. The stable Rust/Cargo/crate-README census
was `fafbed6b88ed1e1ced2e8f68ff9e8429b859ee10ebc05d4f789f4bd2e269362f`
(3,788 files, 73,827,205 bytes). Later inspector packaging is not part of this run.

Three genuine frontend callbacks ran: one ordinary baseline generating the
candidate, then two fresh V17 owners for default and edited register plans.
The two rustc sessions ran sequentially in one fixed-environment child; neither
reused a TyCtxt or executable owner. Only owned diagnostic values crossed them.

Each owner passed 30 independent full-output/initialization/canary simulation
cases and produced one complete debug transcript: 899 records and 833 steps.
Each same-owner source catalog had one file, eight mapped sites and nine
eliminated spans. Actual before/after checkpoints for the ordered operation
resolved to the compiler-produced source spans.

The existing debugger consumer, not a hash-comparison surrogate, rejected:

| Transcript | Admitted module | Catalog | Result |
| --- | --- | --- | --- |
| Current B | Current B | Unchanged old A | SourceIdentityMismatch |
| Unchanged old A | Current B | Unchanged old A | SourceIdentityMismatch |
| Unchanged old A | Current B | Current B | SourceIdentityMismatch |

Failed binding left the session unbound; a valid current binding then succeeded.
The old evidence was not modified to manufacture the failures. Two genuine
positive captures and 60 oracle simulations are 62 executions, not 62 captures.

The 33,342-byte `debug-join-observation.json` SHA-256 is
`bfd636007a7240a9376e4338eaf0b8e597378c0c871ac948dd305727d9a8b2b8`.
Ten source files occupied 7,543 bytes. The fixed cumulative logical envelope
prepaid 108,535,808 bytes under 128 MiB for both projections/captures and all
bounded clones; this is not allocator/RSS accounting. Existing source/import
and simulation bounds remain separately enforced.

The first unit qualification was deliberately not accepted: its test passed,
but overlapping formatting changed its before/after source census. That receipt
remains failed. The serial rerun `source-debugjoin-unit-r2` passed with identical
censuses; the subsequent actual ladder also retained exact before/after equality.

This qualifies a private in-memory source/catalog/capture join and stale-input
refusals. It does not create public map/capture interchange, establish source
authentication from a catalog, resume compilation, observe physical registers
or instruction microsteps, or close U2/V2. The existing fresh-source report's
`source_map_available: false` still means no public map was exported.
